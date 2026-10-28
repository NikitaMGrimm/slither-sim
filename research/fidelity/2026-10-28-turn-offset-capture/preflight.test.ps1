$ErrorActionPreference = 'Stop'
(Get-Process -Id $PID).ProcessorAffinity = 1
. (Join-Path $PSScriptRoot 'preflight.ps1')

function Assert-Equal($Actual, $Expected) {
    if ($Actual -ne $Expected) { throw "Expected $Expected; got $Actual" }
}
function Invoke-AuthoredCase([int]$Failures) {
    $state = @{ calls = 0; now = [long]0; records = [Collections.Generic.List[object]]::new() }
    $check = {
        $state.calls++
        if ($state.calls -le $Failures) {
            $error = [InvalidOperationException]::new('authored competing connection')
            $error.Data['TurnExclusivityFailure'] = $true
            throw $error
        }
    }
    $failed = $false
    try {
        Wait-TurnExclusive -Check $check -Record { param($Row) $state.records.Add($Row) } `
            -Now { $state.now } -Delay { param($Milliseconds) $state.now += $Milliseconds }
    } catch { $failed = $true }
    return @{ state = $state; failed = $failed }
}

$clear = Invoke-AuthoredCase 0
Assert-Equal $clear.state.calls 1
Assert-Equal $clear.state.now 0
Assert-Equal $clear.failed $false
$eventual = Invoke-AuthoredCase 2
Assert-Equal $eventual.state.calls 3
Assert-Equal $eventual.state.now 1800000
Assert-Equal $eventual.failed $false
$exhausted = Invoke-AuthoredCase 99
Assert-Equal $exhausted.state.calls 5
Assert-Equal $exhausted.state.now 3600000
Assert-Equal $exhausted.failed $true
$generic = @{ calls = 0; delays = 0 }
try {
    Wait-TurnExclusive -Check { $generic.calls++; throw 'authored source failure' } `
        -Record { param($Row) } -Delay { param($Milliseconds) $generic.delays++ }
} catch { Assert-Equal $_.Exception.Message 'authored source failure' }
Assert-Equal $generic.calls 1
Assert-Equal $generic.delays 0
$edge = @{ calls = 0; clockCalls = 0; samples = @(0, 899999, 900001, 900002); delays = @() }
Wait-TurnExclusive -Check {
    $edge.calls++
    if ($edge.calls -eq 1) {
        $error = [InvalidOperationException]::new('authored edge failure')
        $error.Data['TurnExclusivityFailure'] = $true
        throw $error
    }
} -Record { param($Row) } -Now {
    $i = [Math]::Min($edge.clockCalls, $edge.samples.Length - 1)
    $edge.clockCalls++
    $edge.samples[$i]
} -Delay {
    param($Milliseconds)
    if ($Milliseconds -lt 0) { throw 'negative delay' }
    $edge.delays += $Milliseconds
}
Assert-Equal $edge.calls 2
Assert-Equal $edge.delays.Count 1
Assert-Equal $edge.delays[0] 1
Write-Output '5 authored preflight cases passed; no requests or sockets.'
