# Companion sampler for manual captures. It never starts or stops a compiler.
$ErrorActionPreference = 'Stop'
$iteration = 0
while ($true) {
    $compilers = @([System.Diagnostics.Process]::GetProcessesByName('cargo')) + @([System.Diagnostics.Process]::GetProcessesByName('rustc'))
    $cpu = $null
    if ($iteration % 5 -eq 0) {
        $cpu = (Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'").PercentProcessorTime
    }
    [ordered]@{
        unixMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
        compilers = @($compilers | ForEach-Object { $_.Id })
        cpuPercent = $cpu
    } | ConvertTo-Json -Compress
    foreach ($compiler in $compilers) { $compiler.Dispose() }
    $iteration++
    Start-Sleep -Milliseconds 200
}
