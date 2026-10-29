# Reproduce gateway attribution and readiness confirmation

Use Python 3.11.4+ (the extraction filter requires that patch release) and existing
locked/offline Rust dependencies. Reuse the
capacity-checkpoint and food-diff fixed-backend harnesses by reference; do not
copy their implementations into this study. External execution root and one
bounded Cargo target are task-owned. No package/system/limit/security change.

Attribution runtime is `cc1bb1e9935094a00cf513413d7922d858dd5c1e`; primary base
`539b37f3be969827b5eac67c2978e604f9e814e8` and candidate
`da7e65c26d0236156c24e23f43f9bd6dfcabca34`. `afd7e03a` adds tests only.
Reconstruct the [capacity-checkpoint harness](../2026-10-28-capacity-checkpoint/REPRODUCE.md)
by applying its `harness.patch` in a fresh external `$OUT` directory. Its linked
#248 public study supplies `shared248`; do not copy those sources into this study.
Then apply this study's [harness.patch](harness.patch) in the same directory.
This is a delta against that earlier patch: unchanged build/client/run/select
and timing helpers are linked, not duplicated. The published delta was reapplied
locally to the earlier helpers and every resulting helper matched its as-run bytes.
Reuse the earlier pinned common-client binary
or build its unchanged client once and supply that same binary to both arms.
For a fresh build, first use the original #256 base archive `d127acd1` with its
`client_build.py`, then replace `base.tar` for the new hosts. Redirect that
builder's target and all target references to the sole bounded target below;
remove it before later host builds. This run reused the existing client binary.
Put base/candidate Git archives in the external directory and apply only this
study's measurement overlay. The archived as-run gateway_costs.rs retains its
seconds-scale mistake; analyze.py performs the documented exact inverse and
validates every fractional nanosecond remainder and clock-coordinate coverage.
Do not edit a hash-pinned historical module to make its identity look repaired.

The attribution arm's archive is named `attribution-v2.tar`. Archive its runtime
with `git archive --format=tar -o "$OUT/attribution-v2.tar" cc1bb1e9`. Run:

```sh
python3 overlay.py attribution-v2
python3 cleanup_target.py
python3 run_attribution.py attribution-v2 smoke-8-3 8 --pilot
python3 analyze.py smoke-8-3
python3 check_math.py math-controls
```

The actual record contains older failed/preparation attempts, each preserved.
`launch_v2.py` then runs one
mature 500-socket attribution cohort with the existing warmup/class controls.
It reuses `samplerPid` from `attribution-controller.json`: in this run the sampler
was already running from the first, stopped warmup. In a fresh reproduction,
start `core_cpu.py` detached as in #256's reproduction commands and write that
sampler PID to `attribution-controller.json` before invoking `launch_v2.py`.
Run `python3 analyze.py mature-attribution-2` after the cohort completes.
Create `CORE_STOP` and wait for the sampler to exit before later builds.
Its core sampler and all processes stop before later builds. analyze.py reuses
the original selector's per-window quality predicates; it never substitutes an
invalid fine window for coarse. check_math.py writes an exact command/exit
receipt for six unit tests and four deliberate analytical faults. Their detached
copies cannot mutate the retained analysis or measured sources.

Archive the integrated runtime pins as `base.tar` and `candidate.tar`. For
production correctness, `python3 check_integrated.py` extracts the candidate,
runs cargo test --locked -j1 --lib client::server::, then separately disables
compaction MOD, event delivery, and query fallback. Each must fail at runtime.
Finally restore the original bytes, rerun the focused suite and tools/check.py
rust. It saves each command/expected failure/exit under checks-integrated and
writes required-checks-pass.json only after all checks pass. The earlier
preintegration receipt is retained separately, never substituted for this one.
This is the exact receipt producer used for this run; it is not handwritten.
Its `run()` records each command, expected-failure flag and exit, requires runtime
test failures rather than build errors for mutations, and restores source bytes
in `finally` after each fault. It writes exactly
`{'passed': True, 'checks': rows}` as indented JSON with a final newline. The
selected receipt is retained under `checks` in `fixed-results.json`.

For the later transport-only tests, archive `afd7e03a` as
`verification-current.tar`, then run `python3 check_final_tests.py`. It first
compares the production server prefix and complete readiness module against
`candidate.tar`. It runs all four new tests, changes the new Open's
`read_blocked = false` to `true`, requires exit 101 with runtime failures,
restores the original bytes, reruns those four tests, then executes
`python3 tools/check.py rust`. It writes `final-tests-pass.json` directly from
those command/exit rows; [final-tests.json](final-tests.json) retains that receipt.
Neither script needs the unrelated food-diff oracle to create a pass receipt.

primary_build.py base and candidate reuse the capacity overlay symmetrically,
omitting its old optional poll-syscall timer while retaining aligned task CPU,
census and queue telemetry. Actual food-epoch events are recorded only during
measurement. prepare_fixed.py builds the unchanged short/mature full-backend
fixture and 27-case trace matrix, including both payload-only mutation controls.
Use the same sole Cargo target and remove it after all builds/parity finish.
run_fixed_all.py runs short, stock193000/360 and legacy193000/360 rows, comparing
every warm byte and all repeated/direct-parent proof continuations. This fixture
has no sockets and supplies no gateway speed claim.
`prepare_fixed.py` names the earlier execution roots. Reconstruct the
[food-diff full-backend harness](../2026-10-29-food-diffs/REPRODUCE.md) and the
[food-packet payload control](../2026-10-29-food-packet-bytes/REPRODUCE.md), then
bind those constants to the reconstructed external roots. The only fixed-builder
change redirects its target to `/tmp/slither-gateway-round3-target`.
It copies the just-produced `required-checks-pass.json` into `fixed/` before the
inherited selector reads it. The inherited selector's food-diff naming is not a
claim that its old cache mutation ran for this readiness candidate.

After creating the integrated archives, the fresh successful path is:

```sh
python3 check_integrated.py
python3 cleanup_target.py
python3 resume_builds.py
python3 run_fixed_all.py
python3 check_final_tests.py
python3 cleanup_target.py
```

`resume_builds.py` builds both final primary hosts and runs `prepare_fixed.py`,
then writes `builds-complete.json` from successful subprocess returns. The fixed
controller requires that real receipt. Provide `verification-current.tar` for
the final test script as described above; keep the runtime `candidate.tar` pin.

This session originally built both primary hosts, then added the symmetric
post-hold readiness timer with `rebuild_primary.py base` and `candidate`, before
any primary timing. The retained `primary_build.py` includes the same final
timer placement for fresh builds. It wraps the whole readiness function,
including array and query/registration; the inherited field `readinessPoll`
therefore means readiness total in this study. It is disabled in selected holds.

Only after all those gates, `python3 start_series.py` detaches `live_series.py`. The exact
order is discardedAB, then AB/BA/AB/BA/AB. Each arm is a fresh full real-socket
world with tick-zero warmup and the same common client. It uses the unchanged
run.py --short-mature and select.py predicates, with one class-only retry and
no retry for timing, cadence, queues, CPU reconciliation or isolation failures.
The budget is12CPUh, including host/client warmups; the controller reads existing
telemetry to stop a cohort before exceeding it. Keep full redacted command
inventories at boundaries; no manual status commands during counted windows.
reduce_live.py verifies actual food epochs for every selected tick and recomputes
all/M/other gateway and aligned world CPU means,medians,p99s and paired ranges.
No component p99 is added. The independent worlds make results descriptive.
After the controller writes `status: complete`, run
`python3 reduce_live.py`; copy its bounded selected result to `live-results.json`
beside the report. `python3 tables.py` recomputes all report tables from selected
values without the raw traces, guest or extra Python dependencies.

This run writes `status: failed`: the discarded candidate's first attempt fails
the class gate (p90 reaches 197), its allowed class retry passes, then the first
primary base fails the competing-process CPU screen. Do not invoke the complete
five-pair reducer on this partial series or substitute discarded controls.
There is no non-class retry. The unchanged threshold is
`3 * selected_wall_seconds / 300`; sampled other-process CPU is 0.300 seconds
against 0.294052 seconds allowed. Its 10 ms quantization is a recorded limit,
not a post hoc reason to mark the original window accepted.

For the retained incomplete archive, run the post-measurement analysis locally:

```sh
python3 reduce_incomplete.py /path/to/2026-10-28-official-density-capacity/select_results.py
```

It reads `final-complete.tar.gz` beside itself, extracts only known analysis
inputs and imports the earlier selector by reference. It exposes the failed
isolation row without writing an accepted receipt, preserves all exclusions,
checks every actual food flag and reproduces the original all-tick quantiles.
Its selected public `live-incomplete.json` omits the unrelated process's name;
the CPU values and failed status are unchanged. `tables.py` prints these excluded
windows and the attribution/fixed tables, with zero completed primary pairs.

The readiness runtime and its tests are not proposed for landing. Apply
`rejected-readiness.patch` to base `539b37f3`; the patch reconstructs all three
candidate source/test files exactly, as recorded in `rejected-readiness.json`.
The measured candidate refs are local source pins; this patch allows source
reconstruction without requiring those unpushed commit objects. The source tree
is restored byte for byte to the integrated baseline after preserving the patch.

Archive complete sources/binaries/raw traces/command inventories and failures
with anonymous tar ownership. Copy outside the guest, verify SHA256, compare
every prospective deleted file with its archive, then prune task-owned duplicates.
Raw archives are local-only. Selected aggregates and computation/reproduction
deltas are the only public report inputs. Preserve all rejected code as a patch,
no patch for a runtime change that lands. No merge is authorized.

## Fresh series amendment and final rejection

Amendment `9ef4d813` is committed before fresh collection. The original failed
series and `live-incomplete.json` remain unchanged. Main is checked again and
still matches `539b37f3`, so this run reuses the already-verified base/candidate
binaries, common client and correctness receipts. No new build is needed.

Apply `fresh-harness.patch` after the #256 harness patch and this study's original
`harness.patch`, in a separate execution directory. The delta changes only the
isolation allowance plus diagnostic fields and fresh-series bookkeeping; runtime
binary code, client, windows and other gates remain pinned. The guest replay
applies those three patches in order and compares every resulting helper with its
execution source. Public evidence uses whole-artifact pins, not source-file maps.

`setup_fresh.py` verifies the retained `prior-complete.tar.gz` (the original final
archive identified in the report), extracts only immutable host/client inputs and
unchanged helpers, then adds `.010` to `3*(end-start)/300`. It does not import old
runs as fresh rows. Use the public #248 helpers as `shared248`. Without access to
the retained local-only archive, reconstruct and build the pinned hosts/client
using the earlier commands, then apply the same selector delta. The full source
and archived-code patch remain sufficient for rebuilding; raw evidence is not
published as a download.

After `preflight.py`, `start_series.py` launches one fresh discarded AB then five
AB/BA/AB/BA/AB pairs under the unchanged 12-CPU-hour bound and retry policy. The
controller records the frozen plan commit and new series identity. `adoption.py`
implements the predeclared all-tick gateway-mean range separation and per-pair
world-p99 guard; the complete-series reducer requires all five pairs.

This series stops at pair 2's candidate isolation failure: 0.330 CPU-seconds
against 0.304059 allowed. No retry is launched. After the controller and sampler
stop, run on the guest:

```sh
python3 reduce_partial.py
python3 publish_fresh.py
python3 check_adoption.py
```

The partial reducer reuses the complete reducer's math and actual food-flag
checks, exposes the failed screen only in memory, and writes no accepted receipt.
It pairs only accepted primary arms and records that the five-pair adoption test
is unavailable. `publish_fresh.py` removes duplicated case bodies and uncited
submetrics without changing values or acceptance. Copy `fresh-results.json`
beside the report. `tables.py` recomputes its percentages from case values and
requires exact agreement with the selected paired changes. The policy-test
receipt is retained as `fresh-method-checks.json`.

All fresh collection is on the guest; after the new local-CPU restriction, all
remaining tests and reductions also run there outside timed windows. Final
report/navigation checks precede verified archival cleanup and guest release.
No runtime code is proposed, no Performance changelog entry is added, and one
research PR records both attempts. The PR body contains only the assistance
footer, as requested by the maintaining agent. No further epoll trial is planned.
