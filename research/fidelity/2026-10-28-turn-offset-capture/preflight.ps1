# Only a zero-socket exclusivity failure is retryable under the owner amendment.
function Wait-TurnExclusive {
    param(
        [Parameter(Mandatory)][scriptblock]$Check,
        [Parameter(Mandatory)][scriptblock]$Record,
        [scriptblock]$Now = { [Environment]::TickCount64 },
        [scriptblock]$Delay = { param($Milliseconds) Start-Sleep -Milliseconds $Milliseconds }
    )
    for ($checkIndex = 0; $checkIndex -le 4; $checkIndex++) {
        try {
            & $Check
            & $Record @{ checkIndex = $checkIndex; status = 'passed'; checkedAtTick = (& $Now) }
            return
        } catch {
            if (-not $_.Exception.Data['TurnExclusivityFailure']) { throw }
            $failedAt = & $Now
            & $Record @{ checkIndex = $checkIndex; status = 'failed'; checkedAtTick = $failedAt;
                reason = $_.Exception.Message; final = ($checkIndex -eq 4) }
            if ($checkIndex -eq 4) { throw 'Exclusivity preflight exhausted four re-checks; permanent study stop.' }
            $nextCheckAt = $failedAt + 900000
            & $Record @{ checkIndex = $checkIndex; status = 'waiting'; nextCheckTick = $nextCheckAt }
            while ($true) {
                $remainingMs = $nextCheckAt - (& $Now)
                if ($remainingMs -le 0) { break }
                & $Delay ([int][Math]::Min(30000, $remainingMs))
            }
        }
    }
}
