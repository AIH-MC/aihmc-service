param(
    [Parameter(Mandatory = $true)]
    [string]$Target,
    [Parameter(Mandatory = $true)]
    [ValidateSet('windows-x64', 'linux-x64', 'linux-arm64')]
    [string]$Platform
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root
$logs = Join-Path $root 'test-output/logs'
$artifacts = Join-Path $root 'test-output/artifacts'
New-Item -ItemType Directory -Force -Path $logs, $artifacts | Out-Null

function Invoke-CargoStep {
    param([string]$Name, [string[]]$CargoArguments)
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

$staging = Join-Path $root ('test-output/package-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $staging | Out-Null
New-Item -ItemType Directory -Path (Join-Path $staging 'keys'), (Join-Path $staging 'textures') | Out-Null
$binaryName = if ($Platform -eq 'windows-x64') { 'aihmc-auth.exe' } else { 'aihmc-auth' }
Copy-Item -LiteralPath (Join-Path $root "target/$Target/release/$binaryName") -Destination $staging
Copy-Item -LiteralPath (Join-Path $root 'config.example.toml') -Destination $staging
Copy-Item -LiteralPath (Join-Path $root 'assets') -Destination $staging -Recurse
Copy-Item -LiteralPath (Join-Path $root 'docs/DEPLOYMENT.md') -Destination $staging

# Package only the allowlisted files before smoke logs/configuration are created.
if ($Platform -eq 'windows-x64') {
    $archive = Join-Path $artifacts "aihmc-auth-$Platform.zip"
    Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $archive -Force 2>&1 |
        Tee-Object -FilePath 'build.log'
    $unpacked = Join-Path $root ('test-output/unpacked-' + [guid]::NewGuid().ToString('N'))
    Expand-Archive -LiteralPath $archive -DestinationPath $unpacked
}
else {
    $archive = Join-Path $artifacts "aihmc-auth-$Platform.tar.gz"
    & tar -czf $archive -C $staging . 2>&1 | Tee-Object -FilePath 'build.log'
    $buildExitCode = $LASTEXITCODE
    if ($buildExitCode -ne 0) { Get-Content 'build.log'; exit $buildExitCode }
    $unpacked = Join-Path $root ('test-output/unpacked-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $unpacked | Out-Null
    & tar -xzf $archive -C $unpacked 2>&1 | Tee-Object -FilePath 'build.log'
    $buildExitCode = $LASTEXITCODE
    if ($buildExitCode -ne 0) { Get-Content 'build.log'; exit $buildExitCode }
}
$hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  $([System.IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath "$archive.sha256"
& (Join-Path $PSScriptRoot 'smoke.ps1') -Executable (Join-Path $unpacked $binaryName) -Directory $unpacked 2>&1 |
    Tee-Object -FilePath 'build.log'
Copy-Item -LiteralPath 'build.log' -Destination (Join-Path $logs 'smoke.log')
Write-Output "Packaged $archive"
