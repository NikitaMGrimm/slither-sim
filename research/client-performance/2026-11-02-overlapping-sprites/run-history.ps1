param([string]$Label='main-msaa',[string]$Before='profile-main',[string]$After='profile-main',[int]$AfterSamples=4,[string]$Policy='native',[string]$Root='D:/slither-scratch/client-lane-20261101/frame-prep/body-20261102')
$ErrorActionPreference='Stop'

function Snapshot { [ordered]@{utc=[DateTime]::UtcNow.ToString('o');load=@(Get-CimInstance Win32_Processor|ForEach-Object LoadPercentage);processes=@(Get-Process -ErrorAction SilentlyContinue|Select-Object Id,ProcessName,CPU,Path)} }
foreach($pair in 1..5){
 foreach($zoom in @(0,-120)){
  foreach($side in $(if($pair%2 -eq 1){@('before','after')}else{@('after','before')})){
   $id="$Label-$pair-$zoom-$side"
   if(Test-Path -LiteralPath "$root/$id-receipt.json"){throw 'Preserve existing receipt'}
   $binary=if($side -eq 'before'){$Before}else{$After};$samples=if($side -eq 'before'){4}else{$AfterSamples}
   $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName="$root/$binary.exe"
   foreach($arg in @('synthetic','unused',"$root/$id.csv",$zoom,1920,1080,$Policy,$samples,'all',0,1)){$info.ArgumentList.Add([string]$arg)}
   foreach($key in @('GPU_SPRITE_FULL','GPU_SPRITE_PARITY','GPU_SPRITE_READBACK','BODY_ALL_RGBA','BODY_DIAGNOSTICS','GPU_SPRITE_PASSES')){$info.Environment.Remove($key)|Out-Null}
   $info.Environment['GPU_SPRITE_FULL']='1'
   $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
   $observations=[Collections.Generic.List[object]]::new();$observations.Add((Snapshot));$start=[DateTime]::UtcNow
   $p=[Diagnostics.Process]::Start($info);$stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync()
   while(-not $p.WaitForExit(500)){$observations.Add((Snapshot))}
   $observations.Add((Snapshot))
   [IO.File]::WriteAllText("$root/$id.log",$stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult())
   $observations|ConvertTo-Json -Depth 8|Set-Content "$root/$id-processes.json"
   [ordered]@{id=$id;pair=$pair;zoom=$zoom;side=$side;samples=$samples;exit=$p.ExitCode;binarySha256=(Get-FileHash -Algorithm SHA256 "$root/$binary.exe").Hash;wallSeconds=([DateTime]::UtcNow-$start).TotalSeconds;cpuSeconds=$p.TotalProcessorTime.TotalSeconds;compileOverlap=@($observations.processes|Where-Object ProcessName -match '^(cargo|rustc|link)$').Count -gt 0;loadMin=($observations.load|Measure-Object -Minimum).Minimum;loadMax=($observations.load|Measure-Object -Maximum).Maximum}|ConvertTo-Json|Set-Content "$root/$id-receipt.json"
   if($p.ExitCode -ne 0 -or (Get-Content -Raw "$root/$id.log") -notmatch 'complete=true'){throw "failed $id"}
   Write-Output "completed $id"
  }
 }
}
