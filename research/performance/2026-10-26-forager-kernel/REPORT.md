# Exact Forager kernel shortcuts

## Question and scope

Reduce the CPU cost of the portable Forager policy for hosts with many bots,
preserving every decision bit. This is an offline replay study of committed
synthetic/local-simulator inputs, not playing-strength or outside-server evidence.
The original owner-provided AWS figures motivate the work but are not used as
the baseline: their workload differs from this bounded fixture replay.

The baseline is `04ede6605b14a35706182359a329869caeff69cc`, with only the replay
harness from `6dc0a95` added. The measured candidate is `6db923d`, containing two
separately verified changes. `658e7dc` subsequently formats one closure without
changing operations. Parameters, timing derivation, intent derivation, math
functions and floating-point expression order are unchanged.

## Profile and exactness arguments

First, sample the baseline with Linux `perf record -e cpu-clock -F 997 -g
--call-graph dwarf`, for both A1 and A4-mean on the full and dense suites.
[Selected samples](profile.json) identify gathering and distance/clearance work
as the main targets. For the full suite, gathering accounts for 21.25% (A1) and
13.82% (A4) of process samples; `hypot` accounts for 14.89% and 22.68%.
These process profiles include decoding, warm-up and CSV serialization, so the
shares are diagnostic rather than isolated kernel percentages. Profiled timings
are excluded from the speed comparison.

1. **Skip irrelevant occupancy marking** (`5cb33c7`). Occupancy is read only
   when valuing food. If its multiplier is exactly 1, multiplying a positive
   cell value cannot change its bits. Boost regathering discards its temporary
   food scene entirely. Both cases can skip head-path occupancy sampling and
   body-to-cell lookup. Food accumulation and obstacle gathering retain their
   original order and arithmetic.
2. **Stop losing clearance scans** (`6db923d`). Clearance is a running minimum.
   With the policies' nonnegative danger weights, the existing cost expression
   can only increase as clearance decreases. Once that exact expression is
   greater than or equal to the incumbent cost, the arc cannot replace the
   incumbent: strict `<` keeps ties with the earlier arc. The incumbent retains
   its complete clearance. The two boolean safety probes likewise stop once
   clearance is strictly below the safety threshold. No approximate bound,
   reassociation, replacement math function or changed tie order is used.

The new tests cover a cost tie before a later, closer obstacle, and compare all
four returned search values bit for bit against exhaustive fan scoring on 384
seeded scenes (historical defaults, A1 and A4). They include cruise/boost arcs,
cut-off head paths, rim cases and restricted boost fans.

## Measurement method

- One AMD EPYC 9R14 `c7a.2xlarge` Spot instance in `eu-central-1`; every timed
  process pinned to logical CPU 0. No local desktop timing measurements.
- Rust 1.99.0, x86-64 GNU, locked dependencies, release thin LTO and one codegen
  unit. Debug line information enabled equally for both builds; no custom
  `RUSTFLAGS`, native CPU tuning or fast-math. Build logs and binary hashes
  retained. Both binaries are library test executables, allowing the private A4
  parameters to be exercised without adding a public benchmark API.
- The identical opt-in `client::forager::conformance::replay_benchmark` harness
  decodes once, creates one reused bot, reserves the sample buffer and warms up
  for 200 complete passes. Each measured interval wraps one full adapter
  `decide`, including timing and intent derivation. It includes `Instant` clock
  overhead; decoding, sample storage, output conversion and file I/O are outside
  the measured interval. Every sample retains its case ID and action bits.
- The full suite has 128 actionable frames from the 140-case conformance
  fixture. Invalid/lifecycle frames remain in correctness tests, but are not
  timed as decisions. The dense subset has eight actionable frames with at
  least 512 opponent body anchors. It is a subset, not extra independent data.
- Twelve adjacent baseline/candidate pairs per policy and suite, with order
  reversed on odd pairs. Each pass has 4,000 fixture cycles: 512,000 decisions
  for the full suite, 32,000 for dense. That is 6,144,000 and 384,000 samples
  respectively per policy and binary. [measure.py](measure.py) is the exact
  standard-library driver used. No samples are removed.
- Quantiles use nearest rank. Report the median of the 12 pass statistics and
  the range of paired candidate/baseline ratios. The largest observed sample
  across all passes is also reported separately: maxima can be dominated by
  scheduling interruptions. This is a descriptive comparison on a fixed corpus,
  without a production-distribution or familywise statistical claim.
- Every pass must have the same complete action-stream SHA-256 as its matched
  baseline, across all repeats. Platform conformance separately checks every
  timing, intent, heading and boost bit against the frozen oracle.

## Results

Times in microseconds. Columns through p99.9 are medians of the 12 pass
statistics; **max** is the largest observed sample over all 12 passes.

| Policy / suite | Build | Mean | p50 | p90 | p99 | p99.9 | Max |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A1 / all | baseline | 7.146 | 4.780 | 15.975 | 32.380 | 55.190 | 217.878 |
| A1 / all | candidate | 6.483 | 4.560 | 13.745 | 27.240 | 46.844 | 197.069 |
| A1 / dense | baseline | 14.866 | 10.415 | 48.579 | 49.684 | 60.245 | 222.198 |
| A1 / dense | candidate | 12.923 | 9.205 | 41.330 | 42.025 | 52.264 | 198.719 |
| A4-mean / all | baseline | 14.815 | 6.740 | 42.294 | 92.140 | 108.699 | 262.848 |
| A4-mean / all | candidate | 13.044 | 6.735 | 34.489 | 72.394 | 92.830 | 255.558 |
| A4-mean / dense | baseline | 41.840 | 39.815 | 72.279 | 80.650 | 85.070 | 220.309 |
| A4-mean / dense | candidate | 41.312 | 39.870 | 72.264 | 80.559 | 84.939 | 223.309 |

Paired ratios (candidate/baseline; below 1 is faster). The range contains
all 12 pairs, not a confidence interval. Max here uses each pass maximum.

| Policy / suite | Mean | p50 | p90 | p99 | p99.9 | Pass max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A1 / all | 0.908 (0.898–0.915) | 0.954 (0.932–0.962) | 0.861 (0.851–0.873) | 0.842 (0.821–0.852) | 0.848 (0.838–0.858) | 1.034 (0.746–1.188) |
| A1 / dense | 0.868 (0.825–0.925) | 0.885 (0.808–0.962) | 0.850 (0.826–0.879) | 0.843 (0.820–0.883) | 0.866 (0.846–0.886) | 0.826 (0.370–1.363) |
| A4-mean / all | 0.881 (0.866–0.883) | 0.998 (0.971–1.013) | 0.815 (0.809–0.822) | 0.787 (0.768–0.799) | 0.855 (0.842–0.868) | 0.881 (0.749–1.077) |
| A4-mean / dense | 0.987 (0.980–1.004) | 1.002 (0.997–1.004) | 0.999 (0.989–1.017) | 0.996 (0.983–1.064) | 0.999 (0.989–1.020) | 1.018 (0.772–1.054) |

All 96 action-stream hashes agree within their policy/suite, including every
baseline/candidate and repeated pass. [passes.json](passes.json) retains all
pass statistics and CSV hashes; [summary.json](summary.json) retains the
aggregate statistics.

The full-suite mean improves by 9.2% for A1 and 11.9% for A4, using median
paired ratios; p99 improves by 15.8% and 21.3%. A1 also improves throughout the
dense-suite quantiles. A4's full-suite median is essentially unchanged. A4's
dense-suite mean changes by only 1.3%, and its quantiles are essentially flat:
their paired ranges include regressions. Treat that dense result as within
noise, not a demonstrated stress-tail improvement. All pass-maximum ratio
ranges cross 1; no reliable maximum-latency gain is claimed, despite several
lower worst observed samples. The A4 dense worst sample increases from
220.309 to 223.309 microseconds.

Adopt the two small exact shortcuts for the clear full-suite mean and tail
improvements, retaining the dense A4 limitation. No parameter/default change
or performance threshold relaxation is part of this work.

## Verification and limits

Before each optimization commit, Windows x86-64 MSVC and WSL Linux x86-64 GNU
passed both A1/A4 platform-oracle equality and all existing kernel regressions.
After the second change, 51 focused tests pass on each platform; the timing
test is ignored during correctness runs. Fresh/shared interleaving and
steady-allocation regressions pass. AWS release-profile baseline and candidate
conformance and kernel tests also pass before timing.

The full `python tools/check.py rust` group passes on Windows (format, all
Clippy feature groups, default/core/bot/networking tests, doctests, C ABI,
Python-crate check and basic example). Windows release-profile focused checks
also pass all 51 tests. The seven documentation tests pass. The tools,
packaging and WASM groups were not rerun locally; this change does not alter
their interfaces or targets. The selected corpus and seeded search regression are finite
coverage; the source monotonicity argument is required in addition to those
tests. No macOS bit-equality oracle exists. No claim is made about server
capacity, 500-bot scheduling, playing strength or every possible hardware target.

## Reproduction and evidence

Build the baseline and candidate in separate output directories with the same
harness and toolchain:

```sh
CARGO_BUILD_JOBS=2 CARGO_PROFILE_RELEASE_DEBUG=1 \
  cargo +1.99.0 test --release --locked --lib --no-run --message-format=json
```

Take the executable path from Cargo's `compiler-artifact` JSON, run its
`forager` and `bot::` tests, then on an isolated Linux measurement host:

```sh
RUST_TEST_THREADS=2 RAYON_NUM_THREADS=2 python3 measure.py \
  --baseline /absolute/path/to/baseline-test-binary \
  --candidate /absolute/path/to/candidate-test-binary \
  --output /fresh/results --cpu 0 --pairs 12 --rounds 4000
```

[evidence.json](evidence.json) records fixture, harness, driver and raw archive
hashes. Raw CSV samples, full profiles, build logs, private launch/cleanup
records and failed attempts remain **local-only** under the task's
`bot-kernel-speedup` scratch directory. Hashes identify these retained files;
they do not make them downloadable. AWS identifiers, account details, addresses
and credentials are excluded from this report and the repository.

The first AWS attempt was retained as failed: Ubuntu's
`perf_event_paranoid=4` denied the sampling event. It produced no accepted timing
data and was terminated. The second instance enabled process profiling before
the run. The conversation interruption stopped the local controller, but the
remote job and verified two-hour shutdown timer continued; management resumed
against that same instance. A Spot cancellation briefly remained visible as
active after termination; the subsequent read confirmed it cancelled.

Both instances had one-time Spot requests with a $0.25/hour price cap, a
16-GiB root volume deleted on termination, and a two-hour automatic shutdown
configured to terminate the instance. Even both complete leases would cap
compute at $1 total, below the $3 task budget with storage/IP charges.

Cleanup was verified by a fresh regional audit after termination: zero
nonterminated instances, volumes, addresses, owned snapshots and open/active
Spot requests. Both task key pairs and security groups were deleted. The two
leases actually lasted about 3 and 20 minutes; their combined compute cost at
the price cap is below $0.10. This is a price-cap calculation, not an invoice.
