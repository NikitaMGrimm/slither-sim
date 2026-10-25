$ErrorActionPreference = 'Stop'
(Get-Process -Id $PID).ProcessorAffinity = 48
$taskRoot = 'C:\Users\Nikita\.t3\scratch\official-capture-20261025'
$taskDeadline = (Get-Date).AddMinutes(80)
$PID | Set-Content -LiteralPath "$taskRoot\process-monitor.pid"
do {
    $taskRows = @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,ExecutablePath,CreationDate,KernelModeTime,UserModeTime,WorkingSetSize)
    $taskRecord = [pscustomobject]@{utc=(Get-Date).ToUniversalTime().ToString('o'); processes=$taskRows}
    $taskRecord | ConvertTo-Json -Depth 4 -Compress | Add-Content -LiteralPath "$taskRoot\process-inventory.jsonl" -Encoding utf8
    Start-Sleep -Seconds 30
} while ((Get-Date) -lt $taskDeadline -and (Test-Path -LiteralPath "$taskRoot\session.lock"))
