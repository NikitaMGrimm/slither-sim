param(
    [Parameter(Mandatory)][string]$ArchiveRoot,
    [Parameter(Mandatory)][string]$RemoteTarget,
    [Parameter(Mandatory)][string]$RemoteScratch,
    [Parameter(Mandatory)][string]$ImageId,
    [Parameter(Mandatory)][string]$Instance,
    [string]$SshExecutable = 'ssh'
)
$ErrorActionPreference = 'Stop'
(Get-Process -Id $PID).ProcessorAffinity = 1
if ($RemoteScratch -notmatch '^/tmp/turn-capture\.[A-Za-z0-9]+$' -or
    $Instance -notmatch '^[a-zA-Z0-9_.-]+$' -or $ImageId -notmatch '^sha256:[a-f0-9]{64}$') { throw 'Invalid private launch configuration.' }
$endpoint = Get-Content -Raw -LiteralPath (Join-Path $ArchiveRoot 'endpoint-private.json') | ConvertFrom-Json
if ($endpoint.ip -notmatch '^(\d{1,3}\.){3}\d{1,3}$' -or $endpoint.port -lt 1 -or $endpoint.port -gt 65535) { throw 'Invalid endpoint.' }
$peer = $endpoint.ip + ':' + $endpoint.port
$study = 'research/fidelity/2026-10-28-turn-offset-capture'
$remote = $null
$sequence = 0
$seenValid = $false
$monitorClock = [Diagnostics.Stopwatch]::StartNew()
$failure = $null
$utf8 = [Text.UTF8Encoding]::new($false)
function Remote-Text([string]$Command) {
    $output = & $SshExecutable $RemoteTarget $Command
    if ($LASTEXITCODE -ne 0) { throw 'Remote census or scratch operation failed.' }
    return ($output -join "`n")
}
function Write-Snapshot($Snapshot) {
    $text = ($Snapshot | ConvertTo-Json -Depth 15 -Compress) + "`n"
    [IO.File]::AppendAllText((Join-Path $ArchiveRoot 'connection-census-private.jsonl'), $text, $utf8)
    $bytes = $utf8.GetBytes($text)
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $SshExecutable
    $info.ArgumentList.Add($RemoteTarget)
    $info.ArgumentList.Add("cat > '$RemoteScratch/archive/connection-snapshot.tmp' && mv '$RemoteScratch/archive/connection-snapshot.tmp' '$RemoteScratch/archive/connection-snapshot-private.json'")
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $writer = [Diagnostics.Process]::Start($info)
    $writer.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
    $writer.StandardInput.Close()
    if (-not $writer.WaitForExit(10000)) { $writer.Kill(); throw 'Census delivery timeout.' }
    if ($writer.ExitCode -ne 0) { throw 'Census delivery failed.' }
}
try {
    $existing = Remote-Text "docker ps -a --filter 'name=^/$Instance$' --format '{{.ID}}'"
    if ($existing.Trim()) { throw 'Task container name already exists; nothing altered.' }
    $command = "docker run --rm --pull never --name '$Instance' --network host --cpus 1 --memory 512m --memory-swap 512m --pids-limit 64 --read-only --tmpfs /tmp:rw,nosuid,noexec,size=16m --env UV_THREADPOOL_SIZE=1 --env NODE_OPTIONS=--max-old-space-size=320 --mount type=bind,src=$RemoteScratch/source,dst=/source,readonly --mount type=bind,src=$RemoteScratch/archive,dst=/archive '$ImageId' node '/source/$study/container-guard.mjs' /archive '$Instance'"
    $stdout = Join-Path $ArchiveRoot ($Instance + '-stdout-private.txt')
    $stderr = Join-Path $ArchiveRoot ($Instance + '-stderr-private.txt')
    $remote = Start-Process -FilePath $SshExecutable -ArgumentList @($RemoteTarget, ('"' + $command + '"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $remote.ProcessorAffinity = 1
    while (-not $remote.HasExited) {
        $sequence++
        $connections = @(Get-NetTCPConnection -ErrorAction Stop |
            Where-Object { $_.RemoteAddress -eq $endpoint.ip -and $_.RemotePort -eq $endpoint.port -and $_.State -notin @('Closed','TimeWait','Listen','Bound') } | ForEach-Object {
                $identity = Get-CimInstance Win32_Process -Filter ('ProcessId=' + $_.OwningProcess)
                @{ state = $_.State.ToString(); pid = $_.OwningProcess; commandLine = $identity.CommandLine }
            })
        $snapshot = @{ instance = $Instance; sequence = $sequence; wall = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();
            windows = @{ ok = $true; connections = $connections }; remote = @{ ok = $false } }
        try {
            $encoded = (Remote-Text "sh '$RemoteScratch/source/$study/connection-probe.sh' '$RemoteScratch' '$Instance' '$peer'").Split("`n")
            if ($encoded.Length -ne 3) { throw 'Invalid census framing.' }
            $decoded = @($encoded | ForEach-Object { $utf8.GetString([Convert]::FromBase64String($_)) })
            $owned = @($decoded[0].Split("`n") | ForEach-Object { if ($_ -match '^\s*(\d+)\s') { $Matches[1] } })
            if (-not $owned.Count) { throw 'Container process census unavailable.' }
            $commands = @{}
            foreach ($line in $decoded[2].Split("`n")) { if ($line -match '^\s*(\d+)\s+(.*)$') { $commands[$Matches[1]] = $Matches[2] } }
            $snapshot.remote = @{ ok = $true; ownedPids = $owned; ss = $decoded[1]; commandLines = $commands }
            $seenValid = $true
        } catch {
            $remote.Refresh()
            if ($remote.HasExited) { break }
            if (-not $seenValid -and $monitorClock.Elapsed.TotalSeconds -lt 15) { Start-Sleep -Milliseconds 500; continue }
            $snapshot.remote.error = $_.Exception.Message
        }
        Write-Snapshot $snapshot
        Start-Sleep -Seconds 3
        $remote.Refresh()
    }
    $remote.WaitForExit()
    if ($remote.ExitCode -ne 0) { throw 'Container guard ended with failure; private records retained.' }
} catch {
    $failure = $_.Exception.Message
    throw
} finally {
    if ($remote -and -not $remote.HasExited) {
        & $SshExecutable $RemoteTarget "docker stop --time 5 '$Instance'" | Out-Null
        if (-not $remote.WaitForExit(10000)) { $remote.Kill() }
    }
    [IO.File]::WriteAllText((Join-Path $ArchiveRoot ($Instance + '-coordinator-result-private.json')),
        (@{ instance = $Instance; failure = $failure; sequences = $sequence; exitCode = $(if ($remote -and $remote.HasExited) { $remote.ExitCode } else { $null }) } | ConvertTo-Json), $utf8)
}
