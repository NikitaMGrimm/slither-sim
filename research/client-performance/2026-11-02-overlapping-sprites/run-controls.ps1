param([string]$Root='D:/slither-scratch/client-lane-20261101/frame-prep/body-20261102',[ValidateSet('sparse','parity','frozen')][string]$Kind='parity')
$ErrorActionPreference='Stop'
function Snapshot { [ordered]@{utc=[DateTime]::UtcNow.ToString('o');load=@(Get-CimInstance Win32_Processor|ForEach-Object LoadPercentage);processes=@(Get-Process -ErrorAction SilentlyContinue|Select-Object Id,ProcessName,CPU,Path)} }
function Run($id,$exe,$arguments,$environment){
 if(Test-Path -LiteralPath "$Root/$id-receipt.json"){throw "Preserve receipt $id"}
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName="$Root/$exe.exe"
 foreach($a in $arguments){$info.ArgumentList.Add([string]$a)}
 foreach($key in @('GPU_SPRITE_FULL','GPU_SPRITE_PARITY','GPU_SPRITE_READBACK','BODY_ALL_RGBA','BODY_DIAGNOSTICS','GPU_SPRITE_PASSES')){$info.Environment.Remove($key)|Out-Null}
 foreach($key in $environment.Keys){$info.Environment[$key]=$environment[$key]}
 $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $observations=[Collections.Generic.List[object]]::new();$observations.Add((Snapshot))
 $p=[Diagnostics.Process]::Start($info);$stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync()
 while(-not $p.WaitForExit(500)){$observations.Add((Snapshot))}
 $observations.Add((Snapshot))
 [IO.File]::WriteAllText("$Root/$id.log",$stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult())
 $observations|ConvertTo-Json -Depth 8|Set-Content "$Root/$id-processes.json"
 [ordered]@{id=$id;exit=$p.ExitCode;binarySha256=(Get-FileHash -Algorithm SHA256 "$Root/$exe.exe").Hash;loadMin=($observations.load|Measure-Object -Minimum).Minimum;loadMax=($observations.load|Measure-Object -Maximum).Maximum}|ConvertTo-Json|Set-Content "$Root/$id-receipt.json"
 if($p.ExitCode -ne 0 -or (Get-Content -Raw "$Root/$id.log") -notmatch 'complete=true'){throw "failed $id"}
 Write-Output "completed $id"
}
if($Kind -eq 'frozen'){
 foreach($pair in 1..5){foreach($side in $(if($pair%2){@('before','after')}else{@('after','before')})){
  $id="frozen-$pair-$side"
  Run $id "verified-$side" @('synthetic','unused',"$Root/$id.csv",0,1920,1080,'native',4,'all',5,10) @{'GPU_SPRITE_PASSES'='1'}
 }}
}elseif($Kind -eq 'sparse'){
 foreach($pair in 1..5){foreach($side in $(if($pair%2){@('before','after')}else{@('after','before')})){
  $id="sparse-$pair-$side"
  Run $id "sparse-verified-$side" @('sparse',"$Root/$id.csv") @{}
 }}
}else{
 foreach($policy in @('native','original')){foreach($zoom in @(0,-120)){foreach($side in @('before','after')){
  $id="exact-$policy-$zoom-$side";$exe="verified-$side"
  Run $id $exe @('synthetic','unused',"$Root/$id.csv",$zoom,1920,1080,$policy,4,'all',0,1) @{'GPU_SPRITE_FULL'='1';'GPU_SPRITE_PARITY'='1';'BODY_ALL_RGBA'='1';'GPU_SPRITE_READBACK'='1'}
 }}}
}
