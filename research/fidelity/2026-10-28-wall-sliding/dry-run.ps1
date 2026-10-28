param(
    [Parameter(Mandatory)][string]$OutputRoot,
    [Parameter(Mandatory)][string]$ServerBinary,
    [ValidateRange(60, 240)][int]$Seconds = 240
)
$ErrorActionPreference = 'Stop'
(Get-Process -Id $PID).ProcessorAffinity = 1
$env:RAYON_NUM_THREADS = '1'
$env:UV_THREADPOOL_SIZE = '1'
$env:RUST_TEST_THREADS = '1'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../../..')).Path
$root = (New-Item -ItemType Directory -Path $OutputRoot -ErrorAction Stop).FullName
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = $listener.LocalEndpoint.Port
$listener.Stop()
$config = @{ seed = 7; bind = ('127.0.0.1:' + $port); players = @{ max = 2 };
    bots = @{ count = 0; threads = 1 }; simulation = @{ delivery = 'official'; recordWire = $false;
        world = @{ radius = 1500; dynamicRadius = $false; dt = .007; foodCount = 0;
            ambientReplenishPerSecond = 0; prey = @{ count = 0 } } } }
$configPath = Join-Path $root 'server-config-private.json'
$config | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $configPath -Encoding utf8NoBOM
$planPath = Join-Path $root 'local-plan-private.json'
@{ dataOrigin = 'local-simulator'; purpose = 'frozen full-selector wall rehearsal'; seconds = $Seconds } |
    ConvertTo-Json | Set-Content -LiteralPath $planPath -Encoding utf8NoBOM
$server = $null
$collector = $null
$success = $false
try {
    $server = Start-Process -FilePath $ServerBinary -ArgumentList @('host','--config',('"'+$configPath+'"')) `
        -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'server-stdout-private.txt') `
        -RedirectStandardError (Join-Path $root 'server-stderr-private.txt')
    $server.ProcessorAffinity = 1
    Start-Sleep -Seconds 2
    if ($server.HasExited) { throw 'Local server failed before collection.' }
    if (-not (Get-NetTCPConnection -LocalAddress '127.0.0.1' -LocalPort $port -State Listen -ErrorAction Stop)) {
        throw 'Local listener unavailable.'
    }
    $capture = Join-Path $root 'capture'
    $collector = Start-Process -FilePath (Get-Command node).Source -ArgumentList @(
        ('"'+(Join-Path $repo 'tools/fidelity/slither-headless.mjs')+'"'),
        '--server',('127.0.0.1:'+$port),'--out',('"'+$capture+'"'),
        '--seconds',$Seconds,'--bots','1','--scenario','boundaryprobe',
        '--nick','"Wall study"','--plan',('"'+$planPath+'"')) `
        -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'collector-stdout-private.txt') `
        -RedirectStandardError (Join-Path $root 'collector-stderr-private.txt')
    $collector.ProcessorAffinity = 1
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while (-not $collector.HasExited -and $clock.Elapsed.TotalSeconds -le $Seconds+15) {
        Start-Sleep -Seconds 1
        $collector.Refresh()
    }
    if (-not $collector.HasExited -or $collector.ExitCode -ne 0) { throw 'Local collector failed or exceeded rehearsal bound.' }
    $success = $true
} finally {
    foreach ($owned in @($collector, $server)) {
        if ($owned -and -not $owned.HasExited) {
            Stop-Process -Id $owned.Id -ErrorAction Stop
            $owned.WaitForExit()
        }
    }
    @{ origin = 'local-simulator'; officialConnections = 0; collectorCompleted = $success;
        configuredSeconds = $Seconds; processesStopped = $true } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'process-result.json') -Encoding utf8NoBOM
}
if (-not $success) { throw 'Local rehearsal failed; retained files must not be replaced.' }
Write-Output 'Local-only recording completed; frozen selector still must be run.'
