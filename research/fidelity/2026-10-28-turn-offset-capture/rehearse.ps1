param(
    [Parameter(Mandatory)][string]$ArchiveRoot,
    [Parameter(Mandatory)][string]$RemoteTarget,
    [Parameter(Mandatory)][string]$RemoteScratch,
    [Parameter(Mandatory)][string]$NodeImageId,
    [Parameter(Mandatory)][string]$RustImageId,
    [string]$SshExecutable = 'ssh',
    [string]$ScpExecutable = 'scp'
)
$ErrorActionPreference = 'Stop'
(Get-Process -Id $PID).ProcessorAffinity = 1
if ($RemoteScratch -notmatch '^/tmp/turn-capture\.[A-Za-z0-9]+$' -or
    $NodeImageId -notmatch '^sha256:[a-f0-9]{64}$' -or $RustImageId -notmatch '^sha256:[a-f0-9]{64}$') { throw 'Invalid private rehearsal configuration.' }
$study = 'research/fidelity/2026-10-28-turn-offset-capture'
$nonce = [guid]::NewGuid().ToString('N').Substring(0,12)
$server = 'turn-rehearsal-server-' + $nonce
$started = [Collections.Generic.List[object]]::new()
$legs = [Collections.Generic.List[object]]::new()
$utf8 = [Text.UTF8Encoding]::new($false)
function Remote([string]$Command) {
    $value = & $SshExecutable $RemoteTarget $Command
    if ($LASTEXITCODE -ne 0) { throw 'Rehearsal remote operation failed; private logs retained.' }
    return ($value -join "`n")
}
function Stop-Task([string]$Name) {
    $id = Remote "docker ps -q --filter 'name=^/$Name$' --filter 'label=research.turn-capture-instance=$Name'"
    if ($id.Trim()) { Remote "docker stop --time 8 '$id'" | Out-Null }
}
function Start-Docker([string]$Name, [string]$Command) {
    if ((Remote "docker ps -a -q --filter 'name=^/$Name$'").Trim()) { throw 'Fresh rehearsal name already occupied.' }
    [IO.File]::WriteAllText((Join-Path $ArchiveRoot ($Name + '-command-private.txt')), $Command, $utf8)
    $p = Start-Process -FilePath $SshExecutable -ArgumentList @($RemoteTarget, ('"' + $Command + '"')) -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $ArchiveRoot ($Name + '-stdout-private.txt')) -RedirectStandardError (Join-Path $ArchiveRoot ($Name + '-stderr-private.txt'))
    $p.ProcessorAffinity = 1
    $started.Add(@{ name = $Name; process = $p })
    return $p
}
function Prepare-Leg([string]$Name, [string]$Ip) {
    $local = Join-Path $ArchiveRoot $Name
    New-Item -ItemType Directory -Path $local | Out-Null
    @{ ip = $Ip; port = 8766 } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $local 'endpoint-private.json') -Encoding utf8NoBOM
    @{ dataOrigin = 'local-simulator'; question = 'authored offline guard rehearsal' } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $local 'plan-private.json') -Encoding utf8NoBOM
    @{ origin = 'authored-local-rehearsal'; legacyInnerOriginLabelIsNotAuthoritative = $true } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $local 'rehearsal-marker.json') -Encoding utf8NoBOM
    Remote "mkdir '$RemoteScratch/$Name'" | Out-Null
    & $ScpExecutable -q (Join-Path $local 'endpoint-private.json') (Join-Path $local 'plan-private.json') (Join-Path $local 'rehearsal-marker.json') "${RemoteTarget}:${RemoteScratch}/$Name/"
    if ($LASTEXITCODE -ne 0) { throw 'Rehearsal input transfer failed.' }
    return $local
}
function Start-Capture([string]$Name, [string]$Instance, [string]$Local) {
    $argsList = @('-NoProfile','-File',('"'+(Join-Path $PSScriptRoot 'connection-monitor.ps1')+'"'),
        '-ArchiveRoot',('"'+$Local+'"'),'-RemoteTarget',$RemoteTarget,'-RemoteScratch',$RemoteScratch,
        '-ImageId',$NodeImageId,'-Instance',$Instance,'-ArchiveName',$Name,'-SshExecutable',('"'+$SshExecutable+'"'))
    $p = Start-Process -FilePath (Get-Command pwsh).Source -ArgumentList $argsList -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $Local 'coordinator-stdout-private.txt') -RedirectStandardError (Join-Path $Local 'coordinator-stderr-private.txt')
    $p.ProcessorAffinity = 1
    $started.Add(@{ name = $Instance; process = $p })
    return $p
}
function Audit-Leg([string]$Name, [string]$Instance, [string]$Mode) {
    $audit = 'turn-rehearsal-audit-' + [guid]::NewGuid().ToString('N').Substring(0,8)
    $command = "docker run --rm --pull never --name '$audit' --network none --cpus 1 --memory 512m --memory-swap 512m --read-only --mount type=bind,src=$RemoteScratch/source,dst=/source,readonly --mount type=bind,src=$RemoteScratch/$Name,dst=/archive '$NodeImageId' node '/source/$study/rehearsal-audit.mjs' /archive '$Instance' '$Mode'"
    return (Remote $command | ConvertFrom-Json)
}
try {
    Write-Output 'Building the local rehearsal server, offline and capped.'
    $build = 'turn-rehearsal-build-' + $nonce
    $buildCommand = @'
docker run --rm --pull never --name '__NAME__' --label research.turn-capture-instance=__NAME__ --network none --cpus 1 --memory 512m --memory-swap 512m --env CARGO_BUILD_JOBS=1 --env CARGO_PROFILE_RELEASE_LTO=false --env CARGO_PROFILE_RELEASE_CODEGEN_UNITS=256 --mount type=bind,src=__ROOT__,dst=/work '__IMAGE__' sh -c 'mkdir -p /work/source/.cargo; printf "[source.crates-io]\nreplace-with = \"vendored-sources\"\n[source.vendored-sources]\ndirectory = \"/work/vendor\"\n" > /work/source/.cargo/config.toml; cd /work/source; CARGO_TARGET_DIR=/work/build-target cargo build --offline --locked --release --no-default-features --features networking --bin slither-sim -j 1'
'@
    $buildCommand = $buildCommand.Replace('__NAME__',$build).Replace('__ROOT__',$RemoteScratch).Replace('__IMAGE__',$RustImageId)
    $p = Start-Docker $build $buildCommand
    $p.WaitForExit()
    if ($p.ExitCode -ne 0) { throw 'Capped offline release build failed.' }
    $config = @{ seed = 7; players = @{ max = 8 }; bots = @{ count = 0; threads = 1 };
        bind = ((@(0,0,0,0) -join '.') + ':8766'); simulation = @{ world = @{ radius = 1500; dynamicRadius = $false;
            foodCount = 550; ambientReplenishPerSecond = 8.04; dt = 0.007; prey = @{ count = 0 } } } }
    $config | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $ArchiveRoot 'local-server-config-private.json') -Encoding utf8NoBOM
    & $ScpExecutable -q (Join-Path $ArchiveRoot 'local-server-config-private.json') "${RemoteTarget}:${RemoteScratch}/local-server-config.json"
    if ($LASTEXITCODE -ne 0) { throw 'Local server configuration transfer failed.' }
    $serverCommand = "docker run --rm --pull never --name '$server' --label research.turn-capture-instance=$server --network bridge --cpus 1 --memory 512m --memory-swap 512m --read-only --mount type=bind,src=$RemoteScratch/build-target/release/slither-sim,dst=/app/slither-sim,readonly --mount type=bind,src=$RemoteScratch/local-server-config.json,dst=/app/config.json,readonly '$NodeImageId' /app/slither-sim host --config /app/config.json"
    $serverProcess = Start-Docker $server $serverCommand
    Start-Sleep -Seconds 2
    if ($serverProcess.HasExited) { throw 'Local server failed to start.' }
    $ip = (Remote "docker inspect --format '{{.NetworkSettings.Networks.bridge.IPAddress}}' '$server'").Trim()
    if ($ip -notmatch '^(\d{1,3}\.){3}\d{1,3}$') { throw 'Local server bridge address unavailable.' }
    foreach ($mode in @('baseline','conflict')) {
        $name = 'rehearsal-' + $mode
        $local = Prepare-Leg $name $ip
        $instance = 'turn-rehearsal-' + $mode + '-' + $nonce
        $capture = Start-Capture $name $instance $local
        $clock = [Diagnostics.Stopwatch]::StartNew()
        Write-Output ('Running authored ' + $mode + ' rehearsal.')
        while (-not $capture.HasExited) {
            $statusText = Remote "if test -f '$RemoteScratch/$name/capture-1-stdout.txt'; then tail -n 1 '$RemoteScratch/$name/capture-1-stdout.txt'; fi"
            $eventText = Remote "if test -f '$RemoteScratch/$name/guard-$instance-private.jsonl'; then tail -n 1 '$RemoteScratch/$name/guard-$instance-private.jsonl'; fi"
            $status = $null; $event = $null
            try { $status = $statusText | ConvertFrom-Json } catch { <# child may be exiting #> }
            try { $event = $eventText | ConvertFrom-Json } catch { <# waiting for first sample #> }
            @{ mode = $mode; elapsed = $status.elapsedSeconds; attributedConnections = $event.ours } | ConvertTo-Json -Compress | Set-Content -LiteralPath (Join-Path $ArchiveRoot 'rehearsal-progress-private.json') -Encoding utf8NoBOM
            if ($mode -eq 'baseline' -and $status.elapsedSeconds -ge 310) { Stop-Task $instance; break }
            if ($mode -eq 'conflict' -and $event.ours -eq 2 -and $status.elapsedSeconds -ge 5) {
                $inject = 'turn-rehearsal-inject-' + $nonce
                $injection = @'
docker run --rm --pull never --name '__NAME__' --label research.turn-capture-instance=__NAME__ --network host --cpus 1 --memory 128m --memory-swap 128m --read-only '__IMAGE__' node -e 'const ws=new WebSocket(process.argv[1]);ws.onopen=()=>setTimeout(()=>ws.close(),12000);setTimeout(()=>process.exit(0),15000)' 'ws://__IP__:8766/slither'
'@
                Start-Docker $inject ($injection.Replace('__NAME__',$inject).Replace('__IMAGE__',$NodeImageId).Replace('__IP__',$ip)) | Out-Null
                break
            }
            if ($clock.Elapsed.TotalSeconds -gt 360) { throw 'Rehearsal did not reach its required condition.' }
            Start-Sleep -Seconds 5
            $capture.Refresh()
        }
        if (-not $capture.WaitForExit(30000)) { Stop-Task $instance; throw 'Rehearsal interruption failed.' }
        $summary = Audit-Leg $name $instance $mode
        $legs.Add($summary)
        $summary | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $local 'rehearsal-leg-summary.json') -Encoding utf8NoBOM
    }
    $summary = @{ origin = 'authored-local-rehearsal'; passed = $true; officialRequests = 0;
        legs = @($legs); windowsBranch = 'unit-tested; local server was not exposed to controller';
        innerLegacyOriginLabel = 'unchanged driver header; outer rehearsal marker identifies synthetic local data' }
    $summary | ConvertTo-Json -Depth 15 | Set-Content -LiteralPath (Join-Path $ArchiveRoot 'rehearsal-summary-private.json') -Encoding utf8NoBOM
    Write-Output 'Offline namespace, host-conflict, interruption and sealing rehearsal passed.'
} finally {
    foreach ($entry in $started) {
        try { Stop-Task $entry.name } catch { <# preserve original failure; record cleanup below #> }
        if (-not $entry.process.HasExited -and -not $entry.process.WaitForExit(10000)) { $entry.process.Kill() }
    }
}
