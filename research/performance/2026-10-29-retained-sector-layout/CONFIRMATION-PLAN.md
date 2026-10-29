# Post-result maintainer decision: one confirmation series

This amendment is the maintaining agent's decision **after the first series
failed**. It is not an amendment to that series' own gate or verdict. Preserve
the original plan, all observations, rejected verdict and tables unchanged.
The maintaining agent declines a post-result override and authorizes exactly
one separately reported, pre-registered confirmation. Adoption rests on this
new series alone. Do not pool either series' observations or use the original
four-pair mature improvements as confirmation observations.

Freeze this document in Git before any new observations. The runtime baseline
is current main `b750004a`, containing post-#274 `9ad002c1`, the 0.8.2 version
metadata and research-only #275. Inspection finds no runtime source change
between those two revisions. The candidate applies the exact original
`rejected-layout.patch`, without edits, tuning or an added threshold. Verify
both changed runtime files byte-for-byte against original candidate `5edb5b2d`
and pin the new archive/build identities before timing. No other runtime change.

## Rows and execution

Use the same portable release/thin-LTO/one-codegen-unit build, complete-backend
fixture, grouped user instructions/cycles/L1d/dTLB counters and clock brackets
as the first series. Require at least 99% counter scheduling per interval;
retain any unsupported/failed acquisition as a failure, not a favorable retry.
Keep all wrapper overhead, without subtraction. Use no affinity, native CPU
code generation, SIMD, package installations or system/security/limit changes.

Four rows only: mature-500 stock and protocol 13, then young-500 stock and
protocol 13. Mature parents warm once at full speed through tick 193,000;
young parents start at zero. Each arm/protocol has its own complete backend.
Compare complete warm streams, fork the same frozen parent for every window,
and use 1,080 advances per child. Retain ordinary/maintenance splits from actual
food-epoch changes, endpoint equality, complete child and direct-parent stream
proofs. A physical checkpoint omits required presentation state and cannot
replace these parents. The previously passed 1/10/50-seat controls are not rerun.

Each row has one discarded AB, then **ten** alternating pairs:
AB/BA/AB/BA/AB/BA/AB/BA/AB/BA. Discarded AB remains retained and labeled.
Keep every reported pair, including CPU disturbances or unfavorable results.
No outcome-based exclusions, class retries, extra pairs or later series.
Do not inspect arm ratios to change execution. Reduce only after all counted
windows and direct-parent proofs finish. Report each protocol/row separately.

All builds, tests, traces, profiles and reductions run on the authorized SSH
guest. Local work is limited to editing, Git/gh, transfers, short integrity
checks and guest control. Use the sole task-owned target
`/tmp/slither-retained-layout-target`, verify copied executables and remove it
before warm proof streams. Do not alter other studies' retained import folders.
Keep full command-line inventories at measurement boundaries, redacting the
account in memory before writing. Public host description remains a 4-vCPU
Zen 5 EPYC KVM guest. No outside game server, cloud resource or local play-server
operation. The Battledome worker stays on AWS.

## Idle preflight and background CPU

Notify the maintaining agent at least 60 seconds before timing. Once all proofs
and builds finish, run an idle preflight with two process snapshots separated
by two seconds and retain redacted full process commands. If another active
CPU job is present, do not start; resolve that precondition without collecting
a partial outcome-driven series. No additional status/build/test/reduction
commands run during counted windows.

The existing request controller records background CPU at **every window's
boundaries**, including discarded ABs. These reads stay outside the measured
advance and do not change Rust instrumentation. Record aggregate `/proc/stat`
busy and steal deltas, the study process tree's CPU delta, and visible non-study
process CPU changes over the request's wall span. Busy excludes idle, iowait and
steal. Study CPU sums each live task-owned process's user/system CPU and waited
children, so an exited fork's CPU remains accounted for after reaping. Classify
the task's explicit process roots by task paths and include their descendants.
Also retain new/exited non-study process counts, because boundary snapshots
cannot fully attribute an unrelated process that begins and exits between them.

Report raw busy-minus-study residuals, even if negative due to counter timing or
quantization, alongside directly visible non-study CPU. Counters normally move
in 10 ms quanta; retain the actual quantum. These are descriptive background
diagnostics over the controller request span (including untimed policy/drains),
not exact tick-aligned attribution. They are **not an exclusion rule** and do
not enter the adoption gate. Keep every window irrespective of background CPU.

## Confirmation-only adoption gate

Apply all three conditions to the ten reported pairs, independently per
protocol. Use candidate/base paired ratios of integer CPU totals, not the ratio
of separate arm medians. The median of ten values is the mean of values 5 and 6
in sorted order; exact rational/integer comparisons decide boundary cases.

1. Mature whole-window median paired CPU ratio is strictly below one in both
   protocols, with **at least eight of ten** pairs strictly improving in each.
2. Mature maintenance median paired CPU ratio is strictly below one in both
   protocols.
3. Each young-500 row's median paired candidate-minus-base absolute CPU cost
   is at most **+10 microseconds/tick**, evaluated from window CPU totals and
   1,080 ticks. Equality at +10 passes; equality at ratio one does not improve.

All conditions must hold. Do not require every mature pair to improve under
this new rule, and do not silently apply this rule to the original five pairs.
If confirmation fails, the candidate stays rejected for good: no further series.
If it passes, include the exact runtime candidate in #277, then rerun the oracle,
four runtime mutations/restoration, all 27 traces and wire controls, full offline
Rust group, documentation checks and one complete mature-stream spot check on
the integrated head. Do not redo or tune the timing series for that integration.

## Reporting and delivery

Store this series' observations, background diagnostics, gate calculation and
tables separately from the first series. Keep the original result and rejection
as historical findings; label the confirmation authorization as post-result and
the final decision as based exclusively on the new pre-registered series.
Results remain descriptive no-socket fixture measurements, not live capacity.

Update #277 in place with the template's Change/Validation sections and one
accurate Agent-assistance footer last, without a top AI note. Retain lean selected
values and table calculators, linking earlier helpers instead of copying them;
no per-file source-hash maps. If rejected, keep the prototype patch. If adopted,
link its immutable original artifact from the first-series record rather than
duplicating landed runtime code as a new patch. Keep full raw evidence local-only,
verify transfers and archive members before deleting task-owned files, announce
guest release, and do not merge the PR.
