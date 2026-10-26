# 500-bot host qualification on AWS — 2026-10-26

Can one host run 500 background bots with the default A4 Forager plus one
watching player in real time, with simulation and sockets sharing one core and
the bots on their own threads? Did #136 (private simulator state) slow it down?

## Findings

- **The host qualifies.** All six primary 300-second holds kept the full
  142.857 ticks/s with no deferred ticks. The simulation thread used 57.4–58.2%
  of its core, 58.3–59.1% with sockets, and the two bot workers 0.45–0.47 cores
  in total.
- Watcher frame excess p99 stayed at 0.075–0.081 ms, inside the 3 ms
  official-like grade; the largest single frame was 10.1 ms late.
- **#136 shows no regression.** Median simulation CPU rose 0.10 points against a
  0.71-point trial spread, and excess p99 rose 0.002 ms against a 0.004 ms spread.
- Four bot workers instead of two changed nothing material (56.9% simulation CPU,
  same delay).
- The [host capacity study](../2026-10-26-host-capacity/REPORT.md) measured 50.9%
  at 500 bots, but with older scripted bots, seven workers and an older world, so
  the two numbers are not a like-for-like comparison.
- Four earlier attempts ended in Spot interruptions or a harness fault; they are
  retained below and excluded from the primary results.

A is #136's parent on main and B its merge commit, so B includes the review
fixes. Both run the same explicit `Config::official(501)` world with
`dynamicRadius: false`. Later commits are not measured.

## Frozen method and sources

- A: `2cc67b78c968bfed36cc472010d8256947e56fbb` (#136's parent on main).
- B: `7d1d9bdcccd1dc575629f68823aae5ca05a64cc3` (#136's merge commit on main).
- Merge base: `2cc67b78c968bfed36cc472010d8256947e56fbb`.
- The [original plan](plan.json) was committed locally before the first launch; its published LF-byte SHA-256
  is `ef54c0e40a9e9dd18ae978961b742fad4d89951fba8e4326903100c402107e7d`. The original Windows CRLF-to-Git-LF normalization is
  recorded in [plan-byte-normalization.json](plan-byte-normalization.json), without
  changing any plan value. The completed attempt used the published LF bytes.
  [attempt5-amendment.json](attempt5-amendment.json) pins the completed attempt's
  merged-PR revisions, autonomous collection and revised Spot pool selection.
  Amendment SHA-256: `cd6c36cfead8c1a84dd77f24da13e1f197f39cbd2fbf0bc819117abc5927c884`. The workload, warm-up,
  hold, grades and spread rule stay fixed. The original B remains in the historical
  plan; the amended B above is the measured revision.
- One Ubuntu 24.04 x86-64 `c7a.4xlarge` Spot instance in `eu-central-1`, 16 vCPUs.
  Hardware readback: Architecture: x86_64; CPU(s): 16; On-line CPU(s) list: 0-15; Model name: AMD EPYC 9R14; Thread(s) per core: 1; Core(s) per socket: 16; Socket(s): 1.
- Both revisions built on that instance with Rust 1.99.0, `--release --locked`,
  the repository's thin LTO / one codegen-unit profile, no added compiler flags,
  and separate source/target trees. Four AWS build jobs; no local build or timing.
  Toolchain, binary, source-archive and executed-script digests are retained in
  [selected-results.json](selected-results.json) and [source-pins.json](source-pins.json).
- Interleaved A1 B1 A2 B2 A3 B3. Each is a fresh seed-7 world, 30 s fixed warm-up
  followed by 300 s measurement. The optional A4t is one later exploratory hold
  with four bot workers, excluded from the A/B comparison.

The [study harness](host_qualification.rs) extends
[`examples/host_load.rs`](../../../examples/host_load.rs). It uses the public Host
API, 500 bot seats, one socket player slot, `Config::official(501)`, default
responsive delivery, default A4 policy and a fixed 40 ms modeled round trip for
both players and bots. The watcher circles slowly without boosting and reconnects
250 ms after death/closure. This preserves the current example's policy and world
setup; the historical host-capacity harness used older scripted bots and seven
workers, so it was used as a method reference rather than as the workload.
No production runtime source was changed or instrumented. Per-decision bot timing
is unavailable from the production Host API; thread CPU covers decoding, policy,
event handling and channel work together.

Linux affinities, read back throughout every hold:

| Work | CPU assignment |
| --- | --- |
| Simulation and socket gateway | Both pinned to CPU 0, sharing one core |
| Watcher and wrapper/counter sampling | CPU 1 |
| Two bot workers | CPU 2 and CPU 3, one worker per core |
| Four-worker exploratory arm | CPU 2, 3, 4 and 5 |
| External Python `/proc` sampler | CPU 6 |

`RAYON_NUM_THREADS=1` bounds any Rayon pool to one worker; all observed thread
members and affinities are in the selected data. CPU percentages are Linux user
plus system CPU seconds divided by the measured wall interval, with 100% meaning
one core. Bot CPU is the sum across worker threads, reported in cores.
`simulation_seconds()` measures busy wall time, including preemption, and is
reported separately; it is not the CPU regression metric.

## Fixed grades and comparison rule

- **Real time:** zero newly deferred ticks during the hold **and** watcher frame
  excess p99 ≤ 10 ms.
- **Official-like:** zero newly deferred ticks during the hold **and** watcher
  frame excess p99 ≤ 3 ms. This is a delivery grade, not official-server fidelity.
- Each valid hold must complete 300 s, preserve the expected affinities and live
  thread membership, have no watcher read/send failures, and cover at least 95%
  of the hold with a playing watcher. Warm-up deferrals are reported separately.
- A/B difference = median(B) − median(A). Spread = the larger of the A and B
  within-revision ranges, separately for simulation CPU and excess p99. A
  regression requires a positive difference **strictly larger** than that spread.
  The paired B_i−A_i differences and their range are also reported. With three
  trials per revision these are descriptive comparisons, not significance tests.
  Decisions use unrounded values; displayed digits identify the arithmetic and
  do not remove timer/header resolution limits.

Frame excess is receipt milliseconds minus the server's cumulative elapsed-ms
headers, then minus that connection's minimum **over the entire hold**. Every
post-setup binary frame in the hold is included, with exact nearest-rank p50/p99
and max. There is no 1,200-frame baseline reset or timer-stall masking. A new
connection gets a new baseline. Headers have 1 ms resolution; this metric removes
fixed path offsets and measures scheduling/clock variation, not absolute latency.

## Results

| Trial | Bot threads | Ticks/s | Deferred | Simulation CPU | Simulation + sockets CPU | Bot CPU, cores | Watcher excess p50 / p99 / max, ms | Grade |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| A1 | 2 | 142.857 | 0 | 57.59% | 58.52% | 0.464 | 0.062 / 0.078 / 10.076 | official-like |
| B1 | 2 | 142.857 | 0 | 58.18% | 59.10% | 0.470 | 0.064 / 0.081 / 4.782 | official-like |
| A2 | 2 | 142.857 | 0 | 57.40% | 58.35% | 0.461 | 0.061 / 0.076 / 7.355 | official-like |
| B2 | 2 | 142.857 | 0 | 57.50% | 58.45% | 0.471 | 0.062 / 0.078 / 0.523 | official-like |
| A3 | 2 | 142.857 | 0 | 57.40% | 58.31% | 0.460 | 0.059 / 0.075 / 2.078 | official-like |
| B3 | 2 | 142.857 | 0 | 57.47% | 58.36% | 0.453 | 0.059 / 0.077 / 8.997 | official-like |
| A4t | 4 | 142.857 | 0 | 56.86% | 57.77% | 0.464 | 0.062 / 0.079 / 5.875 | official-like |

Each primary hold is independently graded. Selected data retain exact hold and
CPU windows, frame denominators, all thread CPU shares, busy-wall load and raw
member hashes. The target rate is 1 / 0.007 ≈ 142.857 ticks/s; boundary sampling
can change the count by one tick. A1–A3 and B1–B3 label revision repetitions;
every trial uses the default A4 policy.

Received leaderboard population includes the watcher and counts living snakes,
not configured bot seats. It is sampled when reports arrive, not a time-weighted
internal population trace. Normal deaths and respawns remain part of this workload.

| Trial | Population samples | Population p50 / p99 / max | Population mean | Watcher connections | Playing coverage | Warm-up deferrals | System steal |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| A1 | 100 | 501 / 501 / 501 | 500.99 | 9 | 99.235% | 0 | 0.0002% |
| B1 | 100 | 501 / 501 / 501 | 501.00 | 5 | 99.617% | 0 | 0.0002% |
| A2 | 100 | 501 / 501 / 501 | 501.00 | 8 | 99.330% | 0 | 0.0002% |
| B2 | 100 | 501 / 501 / 501 | 500.99 | 7 | 99.426% | 0 | 0.0002% |
| A3 | 99 | 501 / 501 / 501 | 500.98 | 8 | 99.426% | 0 | 0.0002% |
| B3 | 99 | 501 / 501 / 501 | 500.99 | 9 | 99.235% | 0 | 0.0002% |
| A4t | 100 | 501 / 501 / 501 | 500.99 | 5 | 99.617% | 0 | 0.0002% |

## A/B verdict for PR #136

| Metric | A median | B median | B − A | A / B within-revision range | Spread threshold | Regression beyond spread? |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Simulation CPU, percentage points of one core | 57.400 | 57.500 | +0.100 | 0.194 / 0.710 | 0.710 | no |
| Watcher excess p99, ms | 0.075764 | 0.077889 | +0.002125 | 0.003099 / 0.003825 | 0.003825 | no |

Neither measured B-minus-A median increase exceeds its predeclared trial-to-trial spread; the frozen rule flags no regression. This applies to the amended, pinned heads with matching world settings.
The exploratory four-worker hold does not contribute to that verdict.

Paired differences, in launch order:

- Simulation CPU percentage points: +0.590, +0.100, +0.073; paired difference range 0.517.
- Excess p99 ms: +0.003, +0.002, +0.002; paired difference range 0.001.

## Attempts, spend and cleanup

The first attempt compiled both revisions, then failed its native 3+8 s smoke:
the initial watcher incorrectly treated bundled WebSocket payloads as individual
protocol packets and miscounted playing time. **No graded hold was collected in
that attempt.** Its raw archive and failed validity outcome are retained locally.
[harness-amendment.json](harness-amendment.json) records the correction before
graded collection: use public `protocol::split_frame`, recognize full appearance
even in a setup bundle, handle bundled game-over, and retain received population.
Both timing thresholds, order, primary duration and regression rule remained fixed.

Attempt 2 passed its corrected smoke and retained completed A1 and B1 summaries,
then lost its SSH connection during A2. Initial diagnosis blamed the desktop link;
later AWS state records identify `Server.SpotInstanceTermination`. The instance
ended while A2 was incomplete, and no full raw archive was recovered. This entire
attempt is excluded from the primary comparison.

Attempt 3 re-pinned #136 after it had rebased onto main.
SSH setup is retried at most five times, with 2/4/8/16 s backoff. Builds and holds
run under a detached remote supervisor with an exclusive job claim and persistent
status. Reconnecting observes the same job and cannot start a duplicate hold.
An actually interrupted hold is retained as invalid and may rerun once in its
original ABAB slot, only after live work is ruled out. Native smoke also tests
duplicate-launch deduplication. It passed before attempt 3's A1.

Attempt 3 then exhausted bounded SSH retries after A1/B1. AWS also records a Spot
termination for this attempt; its completed holds are supplementary only and no
full raw archive was recovered. These failures do not demonstrate workload overload.

Attempt 4 follows the plan's merged-PR rule and runs the entire build/ABAB sequence
on the instance without a desktop SSH dependency. It publishes progress and the
final archive through two keys in a temporary private S3 bucket. A temporary
instance role grants only `s3:PutObject` on those keys, with no embedded credential;
public access is blocked. The desktop polls S3 every 90 s and verifies the final
archive's SHA-256 before cleanup. The boot hard shutdown remains independent of
the runner and desktop. The handoff uses `uv` with `boto3==1.43.118`.

AWS terminated attempt 4 during A1, with Spot status
`instance-terminated-no-capacity`. Native smoke and progress were retrieved through
S3, but no primary hold completed. The instance, storage, groups, key and private
handoff resources were cleaned up. This failure established the interruption
cause independently of the desktop SSH symptoms.

Attempt 5 keeps the merged-PR revisions and autonomous execution, and changes the
hardware/pool arm before collection: a `c7a.4xlarge` with 16 physical cores,
selected from an untried zone by advisory Spot placement score before price.
The mixed-type placement scores are not a guarantee for the chosen individual
type. Work remains pinned to the same host, watcher and bot CPUs; extra cores
are idle apart from management work. The workload, primary order, durations,
grades and regression criterion remain unchanged. All primary statistics here
come solely from this completed attempt.

Post-collection source review found that the outer supervisor still carried a
1,600-second build-era timeout, shorter than the autonomous sequence. Collection
nevertheless continued through the independent Python worker, as the S3 progress
and complete raw holds demonstrate. The published supervisor guard is corrected
to 90 minutes, below the 100-minute boot shutdown, with a focused regression test.
This management correction changes no measured host, sampler, grades or results;
executed-script hashes retain the collected version.

Supplementary completed holds from failed attempts, **excluded from all primary
grades and A/B statistics**. Source revisions and summary hashes are retained in
the selected JSON. Their full raw data could not be retrieved before cleanup,
so only the returned summaries are available.

| Attempt | Hold | Revision | Simulation CPU | Bot CPU, cores | Deferred | Excess p50 / p99 / max, ms |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| 2 | A1 | `cbe8bd9` | 57.91% | 0.461 | 0 | 0.060 / 0.079 / 4.751 |
| 2 | B1 | `c274de6` | 57.17% | 0.459 | 0 | 0.053 / 0.071 / 7.956 |
| 3 | A1 | `cbe8bd9` | 57.24% | 0.463 | 0 | 0.062 / 0.079 / 9.032 |
| 3 | B1 | `7689285` | 57.54% | 0.459 | 0 | 0.062 / 0.079 / 11.216 |

All five instances were one-time Spot, capped at $0.50/hour for attempts 1–4 and
$0.80/hour for attempt 5, configured with
instance-initiated shutdown → terminate, a verified `shutdown -h +100` timer and
delete-on-termination 24 GiB gp3 storage. Only SSH from the operator's current
address was admitted; game traffic stayed on loopback. No outside game servers
were contacted. Build/download/bootstrap traffic is not game traffic.

Estimated total for all five attempts: **$0.553**,
using each launch's Spot quote plus IPv4 and gp3, conservatively through cleanup
completion, plus a $0.01 allowance for S3 requests, handoff storage and transfer.
This is an estimate, not an invoice. The combined compute estimate at each attempt's price cap is
$1.040; the $10 spend cap
was preserved with ample reserve. Public IPv4 and storage rates are estimation
inputs, not a billed-cost claim.

Cleanup was verified for each attempt, most recently **2026-10-26T10:30:00.179317+00:00**. Across
**eu-central-1, us-east-1, us-west-2 and eu-west-1**, zero study-created
nonterminated instances, open/active Spot requests, volumes, security groups or
key pairs remained. Historical terminated-instance and cancelled-Spot records
can remain visible in AWS without being live resources. Checks are scoped to
the study's saved ownership records and tags; unrelated resources were preserved.
Temporary S3 buckets, IAM instance profiles and roles were also deleted and
verified absent. Private AWS inventories retain identifiers; none are included here.

## Reproduction, checks and limits

On an equivalent native Linux AWS machine with at least seven available cores,
archive each pinned Git revision into its own directory, copy
`host_qualification.rs` into each `examples/` directory, and build with the
recorded toolchain and release profile. Then, from this study directory:

```sh
# Repeat separately for A and B; targets must remain isolated.
CARGO_BUILD_JOBS=4 cargo build --release --locked --example host_qualification
python3 run_trial.py /absolute/path/to/A/target/release/examples/host_qualification /scratch/A1
# Run B1 A2 B2 A3 B3 in the frozen order. Never time on a shared desktop.
python3 select_results.py /scratch/first-attempt /scratch/attempt2 /scratch/attempt3 /scratch/attempt4 /scratch/completed-attempt
python3 write_report.py
python3 -B -m unittest discover -s . -p 'test_*.py'
```

The cargo command runs inside each archived source tree; the Python commands run
inside this study directory. `aws_run.py` supplies the bounded provision/build/
trial/collect/finally-cleanup pattern and verifies launch revisions against the
frozen plan. Historical plan reuse requires the original source pins; do not
silently retarget it to newer main. Raw archives must be restored into the recorded
private scratch layout for `select_results.py`; they are **local-only**, without
a public retrieval URL. [source-pins.json](source-pins.json) and `files.sha256`
provide integrity identifiers, not independent verification of scientific claims.
The earlier [host-capacity study](../2026-10-26-host-capacity/REPORT.md) and all its
pinned scripts were left unchanged.

Validation: both AWS release builds, native sampler smoke, all 7 completed
holds with boundary/affinity validation, four synthetic analysis tests (nearest
rank, strict spread rule, grades/counter deltas, incorrect pin rejection), five
transport/job-ownership tests, Rust format check, and repository documentation
tests. No local timing or full
Rust test suite was run for this study-only addition.

Limits: one AWS instance type/CPU, one seed, three primary holds per head and one
exploratory hold; fresh worlds followed for five minutes, asynchronous bot
trajectories, fixed RTT with no jitter, a loopback watcher rather than a real WAN
player, correlated frame samples, limited tail duration, and no per-decision bot
cost/cadence counter. Aggregate bot CPU is not a per-decision tail bound.
This does not establish 500 socket-player capacity, long-lived
mature-world capacity, gameplay strength, mechanics or official-server fidelity.
No fitted model or mechanics rule was proposed; held-out fit validation is
inapplicable. This comparison is matched by source ancestry and initial world
settings, rather than by identical asynchronous gameplay trajectories.
