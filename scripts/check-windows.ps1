param([string]$Python = './.venv/Scripts/python.exe')

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath (Split-Path -Parent $PSScriptRoot)
& ./scripts/ci.ps1 -Target x86_64-pc-windows-msvc -Platform windows-x64
if (-not $?) { throw 'Windows build pipeline failed' }
$env:PYTHONUTF8 = '1'
$env:AIHMC_TEST_BINARY = Join-Path (Get-Location) 'target/x86_64-pc-windows-msvc/release/aihmc-auth.exe'
& $Python -m unittest tests.test_python_baseline tests.test_mysql_baseline tests.test_rust_integration tests.test_rust_textures tests.test_differential tests.test_archive -v 2>&1 |
    Tee-Object -FilePath 'build.log'
$buildExitCode = $LASTEXITCODE
Copy-Item -LiteralPath 'build.log' -Destination 'test-output/logs/windows-integration.log'
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath 'build.log'
    exit $buildExitCode
}
