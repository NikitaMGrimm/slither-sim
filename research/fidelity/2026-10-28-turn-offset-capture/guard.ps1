param(
    [Parameter(Mandatory)][string]$ArchiveRoot,
    [Parameter(Mandatory)][ValidateSet('Discover', 'Capture')][string]$Action
)
$ErrorActionPreference = 'Stop'
(Get-Process -Id $PID).ProcessorAffinity = 1
$env:CARGO_BUILD_JOBS = '1'
$env:RAYON_NUM_THREADS = '1'
$env:RUST_TEST_THREADS = '1'
$ArchiveRoot = (Resolve-Path -LiteralPath $ArchiveRoot).Path
$lockPath = Join-Path (Split-Path -Parent $ArchiveRoot) '.slither-sim-official-capture.lock'
$activePath = Join-Path $ArchiveRoot 'guard-active.json'
$process = $null
$lock = $null
$guardCreated = $false
$ordinal = $null
$guardFailure = $null
$processResultPath = $null
. (Join-Path $PSScriptRoot 'preflight.ps1')

function Throw-Exclusivity([string]$Message) {
    $failure = [InvalidOperationException]::new($Message)
    $failure.Data['TurnExclusivityFailure'] = $true
    throw $failure
}

function Assert-Exclusive([int]$AllowedPid) {
    $others = @(Get-CimInstance Win32_Process | Where-Object {
        $_.ProcessId -ne $AllowedPid -and $_.Name -match '^(node|python|python3|uv)(\.exe)?$' -and
        $_.CommandLine -match '(slither|official|collect\.py|collector|fidelity|turn-offset)'
    })
    if ($others.Count -gt 0) { Throw-Exclusivity 'Another possible official collector is running; capture stopped.' }
    $endpointPath = Join-Path $ArchiveRoot 'endpoint-private.json'
    if (Test-Path -LiteralPath $endpointPath) {
        $endpoint = Get-Content -Raw -LiteralPath $endpointPath | ConvertFrom-Json
        $connections = @(Get-NetTCPConnection -RemoteAddress $endpoint.ip -RemotePort $endpoint.port -ErrorAction SilentlyContinue |
            Where-Object { $_.State -notin @('Closed', 'TimeWait', 'Listen', 'Bound') -and $_.OwningProcess -ne $AllowedPid })
        if ($connections.Count -gt 0) { Throw-Exclusivity 'Another connection to the bound server exists; capture stopped.' }
    }
}

try {
    $lock = [IO.File]::Open($lockPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    if (Test-Path -LiteralPath (Join-Path $ArchiveRoot 'STUDY-STOP.json')) { throw 'Study stop is permanent.' }
    if (Test-Path -LiteralPath (Join-Path $ArchiveRoot 'STOP')) { throw 'Integrity stop is permanent.' }
    [IO.File]::WriteAllText($activePath, (@{ pid = $PID; action = $Action; started = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() } | ConvertTo-Json))
    $guardCreated = $true
    $scriptName = if ($Action -eq 'Discover') { 'discover.mjs' } else { 'capture.mjs' }
    $prefix = if ($Action -eq 'Discover') { 'discovery' } else { 'capture-' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }
    $stdout = Join-Path $ArchiveRoot ($prefix + '-stdout.txt')
    $stderr = Join-Path $ArchiveRoot ($prefix + '-stderr.txt')
    $processResultPath = Join-Path $ArchiveRoot ($prefix + '-process-result-private.json')
    $scriptPath = Join-Path $PSScriptRoot $scriptName
    $argsList = @('"' + $scriptPath + '"', '"' + $ArchiveRoot + '"')
    if ($Action -eq 'Capture') {
        $ledgerPath = Join-Path $ArchiveRoot 'invocations.jsonl'
        $previous = @()
        if (Test-Path -LiteralPath $ledgerPath) {
            $previous = @(Get-Content -LiteralPath $ledgerPath | ForEach-Object { $_ | ConvertFrom-Json })
        }
        for ($i = 0; $i -lt $previous.Count; $i++) {
            if ($previous[$i].ordinal -ne $i + 1) { throw 'Invalid attempt ledger.' }
        }
        $ordinal = $previous.Count + 1
        if ($ordinal -gt 4) { throw 'Invocation budget exhausted.' }
        if ($ordinal -ge 3) {
            $gate = Get-Content -Raw -LiteralPath (Join-Path $ArchiveRoot 'heldout-gate.json') | ConvertFrom-Json
            if (-not $gate.coveredAllPrimary -or -not $gate.freezeCommit) { throw 'Held-out gate closed.' }
        }
    }
    # This gate precedes reservation/launch, so no game socket can yet exist.
    Wait-TurnExclusive -Check { Assert-Exclusive 0 } -Record {
        param($Row)
        $Row.action = $Action
        $Row.invocation = $ordinal
        $Row.wall = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
        $text = ($Row | ConvertTo-Json -Compress) + "`n"
        [IO.File]::AppendAllText((Join-Path $ArchiveRoot 'preflight-checks-private.jsonl'), $text)
        Write-Output ($Row | ConvertTo-Json -Compress)
    }
    if ($Action -eq 'Capture') {
        $reservation = @{ ordinal = $ordinal; reservedWall = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() } | ConvertTo-Json -Compress
        $ledger = [IO.File]::Open($ledgerPath, [IO.FileMode]::Append, [IO.FileAccess]::Write, [IO.FileShare]::Read)
        try {
            $bytes = [Text.Encoding]::UTF8.GetBytes($reservation + "`n")
            $ledger.Write($bytes, 0, $bytes.Length)
            $ledger.Flush($true)
        } finally { $ledger.Dispose() }
    }
    $start = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath (Get-Command node).Source -ArgumentList $argsList -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $process.ProcessorAffinity = 1
    while (-not $process.HasExited) {
        $remainingMs = 1200000 - $start.Elapsed.TotalMilliseconds
        if ($remainingMs -le 0) {
            Stop-Process -Id $process.Id -Force
            throw 'Hard invocation deadline reached.'
        }
        Start-Sleep -Milliseconds ([int][Math]::Min(3000, $remainingMs))
        if ($start.Elapsed.TotalMilliseconds -ge 1200000 -and -not $process.HasExited) {
            Stop-Process -Id $process.Id -Force
            throw 'Hard invocation deadline reached.'
        }
        Assert-Exclusive $process.Id
        $process.Refresh()
    }
    $process.WaitForExit()
    Assert-Exclusive 0
    Write-Output ('guard complete; exit=' + $process.ExitCode)
    if ($process.ExitCode -ne 0) { throw 'Acquisition process failed; retained logs identify the failure.' }
} catch {
    $guardFailure = $_.Exception.Message
    if ($guardCreated -and -not (Test-Path -LiteralPath (Join-Path $ArchiveRoot 'STUDY-STOP.json'))) {
        [IO.File]::WriteAllText((Join-Path $ArchiveRoot 'STUDY-STOP.json'),
            (@{ reason = 'exclusive-guard-or-child-failure'; ordinal = $ordinal; failure = $guardFailure } | ConvertTo-Json))
    }
    if ($process -and -not $process.HasExited) {
        [IO.File]::WriteAllText((Join-Path $ArchiveRoot 'STOP'), 'exclusive guard interrupted capture')
        $graceMs = [int][Math]::Max(0, [Math]::Min(5000, 1200000 - $start.Elapsed.TotalMilliseconds))
        if (-not $process.WaitForExit($graceMs)) { Stop-Process -Id $process.Id -Force }
    }
    throw
} finally {
    if ($processResultPath) {
        $outcome = @{ action = $Action; ordinal = $ordinal; failure = $guardFailure;
            exitCode = $(if ($process -and $process.HasExited) { $process.ExitCode } else { $null });
            finishedWall = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() }
        [IO.File]::WriteAllText($processResultPath, ($outcome | ConvertTo-Json))
    }
    if ($guardCreated) { Remove-Item -LiteralPath $activePath }
    if ($lock) { $lock.Dispose(); Remove-Item -LiteralPath $lockPath }
}
