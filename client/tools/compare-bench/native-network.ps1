param([Parameter(Mandatory=$true)][string]$Config)
$ErrorActionPreference='Stop'
$c=Get-Content -LiteralPath $Config -Raw | ConvertFrom-Json
$known=@{}
$rootCreation=$null
$deadline=[DateTime]::UtcNow.AddSeconds($c.maxSeconds)
$stopReason='deadline'
do {
  $errors=@();$tcp=@();$udp=@();$tcpCollected=$false;$udpCollected=$false
  $all=@(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CreationDate,ExecutablePath)
  $live=@{};foreach($p in $all){$live[[int]$p.ProcessId]=$p}
  $root=$live[[int]$c.pid]
  if(!$rootCreation -and $root -and $root.ExecutablePath -eq $c.executable){$rootCreation=$root.CreationDate.Ticks;$known[[int]$c.pid]=$rootCreation}
  $owned=New-Object 'Collections.Generic.HashSet[int]'
  foreach($id in @($known.Keys)){if($live[$id] -and $live[$id].CreationDate.Ticks -eq $known[$id]){[void]$owned.Add($id)}}
  do {
    $added=$false
    foreach($p in $all){
      $parent=[int]$p.ParentProcessId;$id=[int]$p.ProcessId
      if(!$owned.Contains($id) -and $owned.Contains($parent) -and $p.CreationDate.Ticks -ge $known[$parent]){
        $known[$id]=$p.CreationDate.Ticks;[void]$owned.Add($id);$added=$true
      }
    }
  } while($added)
  try {
    $tcp=@(Get-NetTCPConnection -ErrorAction Stop | Where-Object {$owned.Contains([int]$_.OwningProcess)} | ForEach-Object {
      @{pid=$_.OwningProcess;localAddress=$_.LocalAddress;localPort=$_.LocalPort;remoteAddress=$_.RemoteAddress;remotePort=$_.RemotePort;state=[string]$_.State}
    });$tcpCollected=$true
  }catch{$errors+=('TCP: '+$_.Exception.Message)}
  try {
    $udp=@(Get-NetUDPEndpoint -ErrorAction Stop | Where-Object {$owned.Contains([int]$_.OwningProcess)} | ForEach-Object {
      @{pid=$_.OwningProcess;localAddress=$_.LocalAddress;localPort=$_.LocalPort;remoteAddress=$null;remotePort=$null}
    });$udpCollected=$true
  }catch{$errors+=('UDP: '+$_.Exception.Message)}
  @{atUnixMs=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();pids=@($owned);rootAlive=$owned.Contains([int]$c.pid);
    tcp=$tcp;udp=$udp;tcpCollected=$tcpCollected;udpCollected=$udpCollected;errors=$errors} | ConvertTo-Json -Depth 5 -Compress
  if(Test-Path -LiteralPath $c.stopFile){$stopReason='requested';break}
  if($owned.Count -eq 0){$stopReason='process-tree-ended';break}
  Start-Sleep -Milliseconds $c.intervalMs
} while([DateTime]::UtcNow -lt $deadline)
if($stopReason -eq 'deadline'){Write-Error 'Network sampler reached its lifetime bound';exit 2}
