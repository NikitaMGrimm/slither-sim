# Conditional follow-up if the 10k wall guard fires

**Superseded before execution by [AMENDMENT.md](AMENDMENT.md).** No extra
warmup is run. This proposed protocol remains as history; `count_censored.py`
instead measures the final guarded snapshot and retains its unsettled label.

The original two-wall-hour limit and its censored verdict remain unchanged.
The 5k warmup takes tens of wall minutes to advance the required virtual time;
10k may hit that guard before the ecology screen can pass. No 10k timing has
been selected when this conditional follow-up is declared.

If and only if the original 10k attempt is censored by wall time before 1,800
virtual seconds, retain it in full. Continue its complete world checkpoint,
admission RNG, held actions, retry clocks and tick in a separate attempt, with
the same nominal two-wall-hour guard and absolute 1,800-virtual-second
ceiling. No stability threshold, policy, density, decision cadence or physics
changes. Preserve the original warm census and append continuation census to
the screen; select the first newly qualifying state by the original criterion.

The native guard is checked at 1,000-tick census boundaries; report actual wall
duration, including any boundary overshoot, rather than claiming an exact timer.

Use a generated study-only resume entry point over the identical frozen helpers.
Before resuming, require its 24-tick parallel action tape and final state to
match the original counted executable from the censored checkpoint. Scratch
and derived caches are recreated, with no policy memory or changed world state.
Pin the helper, generated source and binary against the original master pins.

Only if this follow-up qualifies, run the unchanged original counted binary's
verification, five paired windows and phase diagnostics from its new checkpoint.
Record the follow-up as the selected 10k attempt, keeping the original censored
receipt/census visible. Do not replace any measured window or count from an
unsettled checkpoint as a primary. If it remains censored, report that limit;
the separate diagnostic may describe its last state, explicitly unqualified.
