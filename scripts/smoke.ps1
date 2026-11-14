param(
    [Parameter(Mandatory = $true)]
    [string]$Executable,
    [Parameter(Mandatory = $true)]
    [string]$Directory
)

$ErrorActionPreference = 'Stop'
$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = $listener.LocalEndpoint.Port
$listener.Stop()
$configuration = Get-Content -LiteralPath (Join-Path $Directory 'config.example.toml') -Raw
$configuration = $configuration.Replace('host = "0.0.0.0"', 'host = "127.0.0.1"')
$configuration = $configuration.Replace('port = 8002', "port = $port")
$configuration = $configuration.Replace('access_token = ""', 'access_token = "smoke-test-token"')
$configPath = Join-Path $Directory 'smoke-config.toml'
[System.IO.File]::WriteAllText($configPath, $configuration)
$startOptions = @{
    FilePath = (Resolve-Path -LiteralPath $Executable).Path
    ArgumentList = @('--config', ('"{0}"' -f $configPath))
    WorkingDirectory = $Directory
    RedirectStandardOutput = (Join-Path $Directory 'server.stdout.log')
    RedirectStandardError = (Join-Path $Directory 'server.stderr.log')
    PassThru = $true
}
if ($IsWindows) {
    $startOptions.WindowStyle = 'Hidden'
}
$process = Start-Process @startOptions
try {
    $ready = $false
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        if ($process.HasExited) {
            throw 'Service exited before becoming ready'
        }
        try {
            $response = Invoke-RestMethod "http://127.0.0.1:$port/" -TimeoutSec 1
            $ready = $true
            break
        }
        catch {
            Start-Sleep -Milliseconds 100
        }
    }
    if (-not $ready -or $response.message -ne 'Hello World') {
        throw 'Root HTTP smoke failed'
    }
    $response = Invoke-RestMethod "http://127.0.0.1:$port/check_token" -Headers @{
        Authorization = 'Bearer smoke-test-token'
    }
    if ($response.message -ne '验证通过') {
        throw 'Bearer HTTP smoke failed'
    }
    Write-Output "PASS: real executable HTTP smoke on port $port"
}
finally {
    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id
        $process.WaitForExit()
    }
    Get-Content -LiteralPath (Join-Path $Directory 'server.stderr.log')
    Remove-Item -LiteralPath $configPath
}
