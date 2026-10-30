# Reproduce the lag attribution study

Read [PLAN.md](PLAN.md) and [ATTEMPTS.md](ATTEMPTS.md), including the prospective
scheduler-only follow-up. Raw recordings and native archives are **local-only**,
outside Git; there is no public download URL. Published digests identify retained
material but do not grant access. All native builds, tests, trace decoding and
reductions run on AWS. The local PC performs editing, metadata checks and remote
control only. No shared SSH guest is used.

## Native observation build

The exact runtime revision and archive/binary pins are in `selected-results.json`.
Create `main.tar` with `git archive` at that revision and extract it into a fresh
remote `source/`. Use Rust 1.99 and the default portable release profile:

```sh
python3 tools/prepare.py source tools
export CARGO_TARGET_DIR=/srv/slither-study/target
export CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_DEBUG=0 RUSTFLAGS=''
unset CARGO_ENCODED_RUSTFLAGS
cd source
cargo build --release --locked --bin slither-sim --example lag_micro
```

`tools/` contains this study's [prepare.py](prepare.py) and
[study_lag.rs](study_lag.rs). The prepare script instruments only the disposable
copy's `src/client/server.rs`, writes `instrumentation.diff` and one full Rust
source manifest, and stages the observer outside `src/`. Keep the generated diff
and manifest in the native archive. This checkout has no production source edit.
The build manifest pins the source by one digest rather than repeating file maps.

Reuse #286's constant-boost client and independent probe unchanged. The retained
#286 native archive has SHA256
`407b60d66b5f39299cf45a0402451d97c21650c0922920766b64757e1542d851`.
Its `bin/clients` and `bin/probe` must match the inherited binary hashes in its
`pins.json`; this study checks those matches. Their sources, old pins and native
proof log are included once in this study's native archive. The client source is
`3e3e702536243790ee43047dfbebe3dc7cf7eac20fb31498e0bb0d6e4ce45ca5`.
Rebuilding on another toolchain/environment need not reproduce binary hashes;
label a fresh run with its own artifacts rather than pretending it is this run.

Stage these unchanged #286 helpers beside the new helpers, without copying them
into this public study:

```sh
git show c3cb55d7fa255a3686954c85e96f0b936cc1f25f:research/performance/2026-10-30-battledome-input-latency/run.py > tools/prior_run.py
git show c3cb55d7fa255a3686954c85e96f0b936cc1f25f:research/performance/2026-10-30-battledome-input-latency/checkpoint.py > tools/checkpoint.py
git show c3cb55d7fa255a3686954c85e96f0b936cc1f25f:research/performance/2026-10-30-battledome-input-latency/reduce.py > tools/prior_reduce.py
```

The inherited capture code and decoder are used directly, not thin replacements.
The native client/probe link to their pinned #286 client source; current-main
changes since that pin are reviewed separately and are not silently rebuilt into
them. Runtime, observer and all binary hashes remain explicit.
The current-main diff from the inherited client pin is limited to
`perf-internals`-gated benchmark additions in the server simulation modules;
the default build and received-client/policy paths have no corresponding change.

## Target and observer

On the disposable one-core Linux target, install the matching `linux-tools`
package, enable `kernel.sched_schedstats=1`, mount tracefs if needed, and test
the requested tracepoints with a short non-primary `perf record`. Preserve
capability checks and loss reports. Use only the authorized project resources.
Verify terminate-on-shutdown, the in-guest deadline and DeleteOnTermination before
any counting. Keep the AWS profile explicit and never put endpoints/IDs in Git.

Copy the observation host to `ROOT/bin/host` and the micro example to
`ROOT/bin/lag_micro`. Run the native microcalibration on the target:

```sh
mkdir -p ROOT/micro
touch ROOT/micro/RECORD
STUDY_LOG=ROOT/micro/ticks.bin ROOT/bin/lag_micro > ROOT/micro/results.jsonl
python3 server.py ROOT 18769
```

Use absolute paths for ROOT. The microcalibration's socket counter is unattached;
it is a component cost, not the full live overhead estimate. OFF/LIGHT/TRACE live
calibration includes the attached socket descriptor and pacing records. OFF still
has the mode check, enabled schedstats and external telemetry, so its comparison
does not establish an entirely unobserved pristine-host baseline.

The controller binds only on the task's private network and starts the host
under `taskset -c 0` with the shipped Battledome preset, 641 possible seats,
zero internal bots and seed 7. `[Simulation,Gateway]::run` only gain observation
calls. No poses, RNG, physics, player latency, priorities or tick rules change.

`STUDY_LOG` enables the file observer. A sibling `RECORD` sentinel selects LIGHT
recording; it is polled every 200 ms, outside the counted transition settles.
Fixed binary records are sixteen little-endian `u64` words (128 bytes):

| Word | Meaning |
| ---: | --- |
| 0 | Kind: 0 tick start, 1 tick end, 2 pacing sleep entry, 3 return |
| 1 | Monotonic record sequence; disabled intervals leave tick gaps |
| 2 | CLOCK_MONOTONIC timestamp, ns |
| 3 | Scheduled tick deadline or requested pacing wake, ns |
| 4–5 | Tick number and cumulative deferred ticks |
| 6 | Simulation CLOCK_THREAD_CPUTIME_ID, ns |
| 7–8 | Simulation schedstat CPU and runnable wait, ns |
| 9–10 | Socket schedstat CPU and runnable wait, ns |
| 11–12 | Simulation/socket schedstat timeslice counts |
| 13–14 | Raw thread IDs, retained privately only |
| 15 | Requested pacing sleep length, ns (zero on tick records) |

`ticks.meta.json` supplies the host epoch/monotonic/Unix anchor. Requested sleep
wake timestamps precede the observer's own reads/write, so microsecond observer
cost is part of the wake bound; do not call it a pure kernel timer measurement.
The target also records 10-Hz thread scheduling and guest CPU/IRQ/softirq/steal
counters plus the unchanged CLI stats. Guest counters have jiffy/time resolution.

## Collect the two separate series

On the same-AZ generator, stage `bin/{clients,probe}`, `pins.json`, helpers and a
private job JSON containing `serverPrivate`. Pin the controller to CPU 7; load
workers use CPUs 0–5, probe CPU 6, capture CPU 8, and analysis/SSH transfer work
uses spare cores. Install tcpdump and iputils-ping. Then:

```sh
python3 tools/collect.py ROOT ROOT/job.json
```

This grows the world with the frozen 300/60/60-second prehistory, runs the six
60-second OFF/LIGHT/TRACE/TRACE/LIGHT/OFF windows and chooses the primary mode
only from the frozen overhead guard. Then it runs the three 300-second primary
windows. `calibration.json` records host process CPU/tick and the mode decision.
The denominator for OFF comes from existing CLI tick rates; this is a live
descriptive comparison with ecology drift, not deterministic fixed work.

The all-IRQ command uses scheduler switch/wakeup/new-wakeup, hard IRQ and softirq
entry/exit tracepoints, `--clockid mono` and a 512-page buffer. Trace processes
are closed before checkpointing. Record collector CPU and any lost events.

After the original series completes, preserve all its files and use a fresh
target session directory/controller with the **same native host**, plus a fresh
generator session directory. Reuse the native binaries and master pins. Run:

```sh
python3 tools/followup.py FOLLOWUP FOLLOWUP/job.json
```

The target controller source adds the `sched` observation mode: identical perf
options but only the three scheduler tracepoints. Four 60-second LIGHT/SCHED/
SCHED/LIGHT windows gate three 300-second diagnostic windows. This has a separate
prospective plan and does not replace the original LIGHT primaries. The original
controller is retained in the native archive; the follow-up controller/helper
version has a separate closed archive pinned once in the selected results.
IRQ duration is not reconstructed from scheduler-only captures.
Scheduler running intervals are guest-scheduler observations; they cannot by
themselves disprove unreported hypervisor preemption. Proc CPU bounds and the
coarse steal counters remain separate. Trace labels must not turn invisible
hypervisor time into proven socket CPU.

`collect.py` atomically replaces target-count/RESET files and inherits #286's
capture lifecycle. Every finished window closes its probe, capture and target
trace before checkpointing. `server.py` produces the target component archive
and verifies its members; the generator verifies that download before packaging
the full row. [bundle.py](bundle.py) produces each outer archive, manifest and
atomic `*-ready.json`. A progress marker alone never authorizes download or
source deletion. Secure and hash-check each completed row and the native archive
before teardown; preserve any failed row as failed evidence.

## Reduce and verify remotely

For each series ROOT, retain `pins.json`, the one native archive/ready receipt,
all `LABEL.tar.gz`/`LABEL-ready.json` pairs and analysis helpers. Native archives
can be referenced by symlink from the second session. Run on a spare generator
CPU after a closed row, or after counting:

```sh
taskset -c 9 python3 -B -m unittest test_reduce
taskset -c 9 python3 reduce.py ROOT ROOT/selected-series.json
```

The reducer checks each outer archive against its own ready receipt, each member,
the matching native pins, the nested target receipt and matching window. It
decodes `perf.data` with `perf script --ns`, rejecting recorded loss, and checks
trace runnable-wait time against proc counters. Event timestamps use the same
target clock as tick records. Start lag and completion progress are distinct:
completion includes that tick's work. The comparable #281 diagnostic samples
every 100th completed tick, subtracts virtual tick time and normalizes by the
window minimum. Fresh timestamps are monotonic; #281's census used Unix time.
Interval intersection assigns observed runnable
wait to the socket, other host work, observation/controller work, other guest
tasks and idle/unattributed time. Full traces subtract observed IRQ intervals;
scheduler-only traces cannot. The raw perf stream is required to recompute these
tables; bounded selected summaries alone cannot recreate unrecorded events.

Generator coverage/headroom gates are unchanged; their report-count minimum is
adapted to the actual 60/300-second windows instead of inheriting #286's fixed
300-report minimum for 360 seconds. Preserve every pass/fail and error. Capture
loss invalidates kernel diagnostics for that connection, not independent
monotonic client echoes; report the verified subset and its possible bias.

The frozen >20-ms episode definition, five-tick recovery, censored boundaries and
cause bounds remain fixed in both series. A zero episode count makes causal
episode shares **not estimable**, not proof of zero socket contention or steal.
Do not turn routine runnable wait within a comfortably met 7-ms budget into an
explanation for an unobserved 80-ms stall. Follow the report's distinction between
direct fresh evidence, source behavior and historical inference.

Run repository documentation checks on a separate pristine validation tree with
the study/index overlaid, leaving the measured source manifest intact. Close and
verify the final analysis/extras archive before retrieving it. Only after all
required sources are verified may task resources be terminated. Finish the
all-enabled-region audit, append cleanup/cost receipts and open the research PR;
do not merge and do not change the previously accepted #286 branch.

`assemble.py ROOT OUTPUT` binds both series to the native build and each row
archive. `tables.py OUTPUT` regenerates the marked table block in REPORT.
`augment.py` appends the exploratory no-pacing adjacency without decoding the
unchanged scheduler events again; a full reduction of the final follow-up row
was compared with that path and is equal. Both call the same tested function.

## Post-collection controller correction

CodeQL found assertion-based RPC path guards, which disappear under `python -O`.
The current `server.py` explicitly rejects invalid labels/modes before mutation.
The as-measured controller versions remain in the pinned native/follow-up tool
archives and the pre-correction published revision
`f1333bbec1babe14500c3cafbafb720d3b857251`. Original source pins and results are
unchanged. Current helper hashes therefore differ for this controller; use the
as-run archive when checking the original source manifest. Reassembling with the
corrected helper updates that helper-manifest digest, not quantitative rows.

Two short offline boundary checks pass both normally and under optimization:

```sh
python -B -m unittest discover -s research/performance/2026-10-30-battledome-lag-attribution -p test_server.py
python -O -B -m unittest discover -s research/performance/2026-10-30-battledome-lag-attribution -p test_server.py
```

These sub-second post-collection controller checks ran locally. Native work,
trace decoding, scientific reductions and documentation checks ran on AWS.
