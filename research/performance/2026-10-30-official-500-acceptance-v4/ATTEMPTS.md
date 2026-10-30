# V4 attempts

The plan and three host-side scripts were initially committed in `f1f3ccf5`
before any provisioning. Fresh official on-demand rates passed the frozen caps;
all 17 enabled regions were inventoried, preserving unrelated resources. The
first tagged on-demand pair launched with +90-minute shutdown deadlines.

Before any world, the first two readiness checks reached bootstrap before the
shutdown-file check succeeded; no timing started. Both deadlines were then
verified against launch (<=92 minutes), alongside same-zone placement, one/eight
vCPUs and terminate-on-shutdown. No timer was extended or instance replaced.

AWS preflight verifies the unchanged v3 host/client binary hashes and the three
committed v4 source files. Syntax, targeted/protected service classification,
fail-closed quiet-state checks, live all-process/stat samples, summed headroom,
class/gate and nearest-rank controls passed. The documentation check found one
link to v3's report, not yet on main. Before tick zero it was changed to the
pinned v3 commit URL and rechecked; no method, script, workload or gate changed.

Final plan/source freeze is `89d74121a9eecdd7b73d10b16efeb710de7a892f`.
The [preflight receipt](preflight.json) binds it to the unchanged v3 binaries
and local-only proof archive. All seven AWS documentation checks pass after
the link correction. No new Rust build/test or Cargo target is used.

## Pre-start quiet-policy verification correction

No server world existed when initial quiet verification stopped. Actual timer
failure states, cached load states and snap SSM's higher-priority unit file
exposed verifier/setup errors. Preserve the first preparation source/log/states
and the subsequent reset-of-unloaded-service failure in separate local-only
archives. Correct before any world: stop timers before masking, reset only
failed units, verify UnitFileState masks, and record snap SSM as stop-only while
snapd and its socket remain stopped/masked. The final actual-state preparation
passes on the same instances; no deadline extension, replacement, acceptance
window, result or gate change. Final corrected freeze/proofs supersede the
initial pre-start receipt below, preserving the setup failures.

Corrected final plan/source freeze: `2abc874e275c7e9ed62d7cb358fc9cc8efe1d6d2`. Actual quiet-state
verification, corrected AWS module controls and all seven documentation checks
pass before tick zero. The preflight receipt binds all earlier setup evidence
and the final proof archive; initial masks/failed states remain retained.

## Run 1: complete, all gates pass

The full thirty-minute window completed with zero deferrals. Both online/offline
full results match, including every tail and maximum; all four tail/max mutation
controls are detected. The closed raw archive was retrieved/hash-verified before
teardown. World CPU p50/p99/p99.9/max is 4.750660/7.018760/8.238520/11.906610 ms;
maintenance/ordinary means are 7.150233/4.789356 ms. World p99 <=7 ms is false
and report-only. All client/queue/generator and class gates pass; worst one-second
input-lateness p99 is 1.579167 ms. Final living/mean/p90/p99 is
500/101.434/234/289. No exclusion, tuning or rescue.

All-process/stat data, pre-window/after unit/timer inventories and a timestamped
journal are retained. The descriptive reducer uses direct matched-process CPU
for other work; its signed global/task user-counter residual does not reconcile
exactly and is explicitly not other-process CPU. No gate uses that residual.
First-pair instances/volumes were verified gone before the second launch.

## Run 2: fresh independent pair

The second tagged on-demand pair uses the same image selection, collection
source, verified v3 binaries and frozen gates. Same-zone placement, one/eight
vCPUs, shutdown deadlines <=92 minutes and terminate-on-shutdown were verified.
The same module/headroom/gate/quantile controls and seven documentation checks
passed before its world; the separate run-2 proof archive is bound in preflight.
No new Rust build, target, runtime/generator change or timing retry.

The second full thirty-minute window completes and passes every gate, with zero
deferrals. Full result replay is exact, including tails/maxima and all four
mutation controls. World CPU p50/p99/p99.9/max is
4.631940/7.077700/8.246330/12.019360 ms; maintenance/ordinary means
7.086837/4.707322 ms (full selected precision is authoritative). World p99 <=7 ms
is false and report-only. All other class/client/queue/generator gates pass.
Both closed archives, journals, all-process/full-proc-stat and unit/timer states
are retrieved/hash-verified. The study passes its two-run frozen gates without
replacement, exclusion, retiming, runtime change or rescue of failed v3.

Seven final documentation checks passed on AWS after both windows. Every raw
archive, replay/integrity receipt and final check proof was retrieved before
teardown. Final audit covers all 17 enabled regions with none unavailable;
task instances, requests, volumes, key and group are gone, unrelated resources
preserved. Conservative compute plus full reserve is $1.598168 < $3.
Cost/cleanup numbers and this prose were filled after teardown without changing
link targets, gates or measured results. No further acceptance attempt was run.
