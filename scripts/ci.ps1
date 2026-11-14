param(
    [Parameter(Mandatory = $true)]
    [string]$Target,
    [Parameter(Mandatory = $true)]
    [ValidateSet('windows-x64', 'linux-x64', 'linux-arm64')]
    [string]$Platform,
    [string]$ProjectDirectory = 'rust'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$project = Join-Path $root $ProjectDirectory
$output = Join-Path $root "test-output/$Platform"
$logs = Join-Path $output 'logs'
$artifacts = Join-Path $output 'artifacts'
New-Item -ItemType Directory -Force -Path $logs, $artifacts | Out-Null
Set-Location -LiteralPath $project

function Invoke-CargoStep {
    param([string]$Name, [string[]]$CargoArguments)

    Set-Content -LiteralPath 'build.log' -Value ''
    & cargo @CargoArguments 2>&1 | Tee-Object -FilePath 'build.log'
    $buildExitCode = $LASTEXITCODE
    Copy-Item -LiteralPath 'build.log' -Destination (Join-Path $logs "$Name.log")
    if ($buildExitCode -ne 0) {
        Get-Content -LiteralPath 'build.log'
        exit $buildExitCode
    }
}

Invoke-CargoStep 'format' @('fmt', '--all', '--', '--check')
Invoke-CargoStep 'clippy' @('clippy', '--locked', '--all-targets', '--target', $Target, '--', '-D', 'warnings')
Invoke-CargoStep 'test' @('test', '--locked', '--target', $Target)
Invoke-CargoStep 'release' @('build', '--locked', '--release', '--target', $Target)

$binaryName = if ($Platform -eq 'windows-x64') { 'aihmc-api.exe' } else { 'aihmc-api' }
$binary = Join-Path $project "target/$Target/release/$binaryName"
if ($Platform -ne 'windows-x64') {
    & readelf -h -l -d $binary 2>&1 | Tee-Object -FilePath 'build.log'
    $buildExitCode = $LASTEXITCODE
    if ($buildExitCode -ne 0) {
        Get-Content -LiteralPath 'build.log'
        exit $buildExitCode
    }
    Copy-Item -LiteralPath 'build.log' -Destination (Join-Path $logs 'elf.log')
    $elf = Get-Content -LiteralPath 'build.log' -Raw
    $machine = if ($Platform -eq 'linux-x64') { 'Advanced Micro Devices X86-64' } else { 'AArch64' }
    if ($elf -notmatch "Machine:\s+$machine" -or $elf -match 'INTERP|\(NEEDED\)') {
        throw 'Linux release must have the requested architecture and no dynamic runtime dependencies'
    }
}

$staging = Join-Path $output 'package'
$unpacked = Join-Path $output 'unpacked'
# These paths are fixed children of this job's output directory, never caller supplied.
foreach ($directory in @($staging, $unpacked)) {
    $resolved = [IO.Path]::GetFullPath($directory)
    $allowed = [IO.Path]::GetFullPath($output) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Package cleanup path is outside the job output directory'
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
    New-Item -ItemType Directory -Path $resolved | Out-Null
}
Copy-Item -LiteralPath $binary -Destination $staging
Copy-Item -LiteralPath (Join-Path $root 'config.example.toml'), (Join-Path $project 'README.md') -Destination $staging
Copy-Item -LiteralPath (Join-Path $root 'docs/DEPLOYMENT.md') -Destination (Join-Path $staging 'DEPLOYMENT.md')

Set-Content -LiteralPath 'build.log' -Value ''
if ($Platform -eq 'windows-x64') {
    $archive = Join-Path $artifacts "aihmc-service-$Platform.zip"
    Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $archive -Force 2>&1 |
        Tee-Object -FilePath 'build.log'
    Expand-Archive -LiteralPath $archive -DestinationPath $unpacked
}
else {
    $archive = Join-Path $artifacts "aihmc-service-$Platform.tar.gz"
    & tar -czf $archive -C $staging . 2>&1 | Tee-Object -FilePath 'build.log'
    $buildExitCode = $LASTEXITCODE
    if ($buildExitCode -ne 0) {
        Get-Content -LiteralPath 'build.log'
        exit $buildExitCode
    }
    & tar -xzf $archive -C $unpacked 2>&1 | Tee-Object -FilePath 'build.log' -Append
    $buildExitCode = $LASTEXITCODE
    if ($buildExitCode -ne 0) {
        Get-Content -LiteralPath 'build.log'
        exit $buildExitCode
    }
}
Copy-Item -LiteralPath 'build.log' -Destination (Join-Path $logs 'package.log')
$hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath "$archive.sha256" -Value "$hash  $([IO.Path]::GetFileName($archive))"

# Verify the distributed executable from the extracted package, without real secrets.
Push-Location -LiteralPath $unpacked
try {
    & (Join-Path $unpacked $binaryName) --help 2>&1 | Tee-Object -FilePath 'build.log'
    $buildExitCode = $LASTEXITCODE
    if ($buildExitCode -ne 0) {
        Get-Content -LiteralPath 'build.log'
        exit $buildExitCode
    }
    & (Join-Path $unpacked $binaryName) --config 'missing-runtime.toml' 2>&1 |
        Tee-Object -FilePath 'build.log' -Append
    $buildExitCode = $LASTEXITCODE
    if ($buildExitCode -ne 1) {
        Get-Content -LiteralPath 'build.log'
        throw "Expected missing config exit 1, received $buildExitCode"
    }
    Copy-Item -LiteralPath 'build.log' -Destination (Join-Path $logs 'smoke.log')
}
finally {
    Pop-Location
}
Write-Output "Verified package: $archive (SHA256 $hash)"
if ($env:GITHUB_STEP_SUMMARY) {
    "Platform: $Platform; target: $Target; commit: $env:GITHUB_SHA; SHA256: $hash" |
        Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY
}
exit 0
