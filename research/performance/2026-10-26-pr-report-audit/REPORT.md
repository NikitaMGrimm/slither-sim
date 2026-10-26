# PR report row audit, 2026-10-26

## Evidence and selection

Read-only `gh api --paginate repos/NikitaMGrimm/slither-sim/issues/comments` and
`pulls?state=closed` yielded 93 merged-PR comparison comments. Of these, 74 have
the six legacy rows. The narrower table below uses PR numbers >=78 (59 reports
per long-lived row), separating earlier heuristic reports from the sequential
comparison period. These are latest retained comments, not every superseded
head/run. A flag means an entire reported interval beyond ±5%, not a point
estimate outside the band. These advisory labels do not prove real regressions.

Cost is actual child-process elapsed time, including warmup/setup/teardown,
from the [latest completed sample](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40365122719)
(PR #166). Shared builds, tests and traces cannot be allocated to one row;
their costs remain separate. A row's total includes every discarded warmup and
counted process in that run. Historical calibration workloads predate subsequent
runtime and workload changes.

| Previous timing row | Reports | Outside band | Sole flag PR | Median process, s | Total sampling, s | Decision |
| --- | ---: | ---: | --- | ---: | ---: | --- |
| step-default | 59 | 0 | — | 0.241 | 13.10 | Remove from PR timing; retain trace/local/release catalog. |
| step-food | 59 | 1 | [#126](https://github.com/NikitaMGrimm/slither-sim/pull/126), +6.59% | 0.550 | 9.91 | Keep food/step canary. |
| step-body | 59 | 1 | [#117](https://github.com/NikitaMGrimm/slither-sim/pull/117), −6.57% | 2.493 | 44.80 | Remove expensive timing; retain trace; official step covers physics at a useful population. |
| observe | 59 | 1 | [#116](https://github.com/NikitaMGrimm/slither-sim/pull/116), −9.04% | 0.485 | 8.77 | Keep cheap visibility/observation canary. |
| portable | 59 | 1 | [#122](https://github.com/NikitaMGrimm/slither-sim/pull/122), −33.58% | 1.074 | 28.11 | Replace host-path timing with socket recipients; retain trace. |
| protocol | 59 | 0 | — | 0.545 | 9.87 | Remove timing; decoder regression trace remains. |
| official-100-step | 59 | 0 | — | 0.672 | 17.52 | Keep realistic physics control. |
| official-100-bot | 59 | 0 | — | 0.714 | 12.88 | Keep step plus observation control. |
| battledome-32-step | 35 | 0 | — | 3.153 | 56.88 | Replace timing with 80 socket clients; preserve deterministic trace. |
| client-battledome-64 | 35 | 0 | — | 7.048 | 126.97 | Replace timing with real host admission/socket path at 80 clients; retain trace. |
| dome-proxy-32-step (retired) | 24 | 0 | — | — | — | Historical proxy already superseded; do not restore. |
| dome-proxy-32-bot (retired) | 24 | 0 | — | — | — | Historical proxy already superseded; do not restore. |

Across all 74 legacy reports, step-body also flagged #75 and protocol #48;
these earlier heuristic flags are not treated as sequential interval evidence.
No other legacy row flagged in that larger sample. In #164, both official-100
rows were Unavailable when their runtime-resolved definitions changed. The new
collector pins the head Config on both sides and retains head-only Redefined
values if a runtime still cannot execute that definition.

New `host-official-500` and `host-battledome-80` rows target per-recipient
encoding, visibility/food, physics and crowded admission through actual sockets.
They reuse `host_load` with a separate process for the synthetic client driver;
all host threads share one CPU and the driver uses a second. They use a short
one-second warmup and 286-tick measurement window. CI must establish feasibility;
500 is the target, not a local performance claim. They retain connected and
living diagnostics rather than asserting every socket is always alive.

Six timing rows replace ten. Ten deterministic state traces and the selected
regression tests remain. Dropping timing does not delete its workload definition
or regression coverage. The local/direct-release `ci` suite is unchanged.

## Historical A/A noise

Downloaded all eight successful `pr-report-calibration.yml` runs at commit
`86225c5f372e215477ce34c32fc97981c77cad23`: Ubuntu A
[40135517529](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40135517529),
[40135520936](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40135520936),
Ubuntu B
[40135544202](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40135544202),
[40135547378](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40135547378),
Windows A
[40135473823](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40135473823),
[40135476799](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40135476799),
Windows B
[40135505925](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40135505925),
[40135509157](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40135509157).

For m=1, each cell has six fresh A/A pairs. Fit log(candidate/base) = intercept
+ order separately in each job; pool residual sums of squares over 8 residual
degrees of freedom per OS/workload. Values below are 100 × residual log SD,
not confidence bounds. Linux had no process CPU readings in the retained pilot.

| Workload | Ubuntu operation wall SD, % | Windows operation wall SD, % | Windows whole-process CPU SD, % |
| --- | ---: | ---: | ---: |
| step-default | 4.390 | 7.521 | 8.662 |
| step-food | 1.968 | 2.887 | 3.514 |
| step-body | 0.974 | 5.701 | 4.189 |
| observe | 0.700 | 2.616 | 4.994 |
| portable | 4.641 | 0.936 | 1.456 |
| protocol | 3.166 | 11.281 | 7.503 |
| official-100-step | 2.415 | 3.394 | 2.101 |
| official-100-bot | 3.190 | 9.768 | 4.064 |
| dome-proxy-32-step | 1.019 | 1.377 | 1.302 |
| dome-proxy-32-bot | 1.700 | 3.555 | 6.177 |

Whole-process CPU is not uniformly quieter, and its scope includes setup and
warmup. Deterministic rows therefore retain operation wall. New PR pilots retain
Linux whole-case CPU diagnostics as well as socket-window CPU versus busy wall.
Instruction-count availability is tested explicitly on the runner; no claim is
made from an unavailable counter. Metric choice and pilot limits are documented
in [the protocol extension](../../../docs/maintainers/performance-statistics-v2.md#pr-metric-extension).

## Fixture-size investigation (no fixture changes)

At the audited tree, `tests/fixtures` has 42 files totaling 12,292,562 bytes.
Independent gzip level 9 compression of every file totals 1,394,021 bytes,
saving 10,898,541 bytes (88.66%). These are working-tree file sizes, not Git pack
or download-size estimates; Git already compresses objects.

`forager-conformance.json` is 8,352,478 bytes (67.95% of the fixture directory).
Lossless gzip level 9 with timestamp zero is 824,221 bytes: 7,528,257 bytes saved
(90.13%). Compact JSON is 8,352,477 bytes, saving only the final newline; it is
already compact. Gzipping compact JSON gives 824,218 bytes. Reproduce with Python
standard-library `gzip.compress(raw, compresslevel=9, mtime=0)`; decompression
returns every original byte and preserves f64 bit strings, record order and hashes.

All 140 full case records are distinct. Five frame payloads repeat, totaling
231,552 repeated serialized frame bytes (2.77% of the file). Pooling those frames
could retain each case and its metadata, but deleting the cases would change the
forward/reverse shared-instance sequence. `src/client/forager/conformance.rs`
checks all 140 cases, both orders, fresh/shared determinism and input immutability;
platform oracles also pin case IDs and every input hash. Repeated frames therefore
are not evidence of dispensable regression cases. `expected` fields are empty
in this input fixture; the separate platform oracles carry expected results.

Prefer byte-preserving compression over dropping cases if a later task changes
storage. The current tests use `include_str!` and package these fixtures, so gzip
requires an explicit test-time decompression path (and likely a dev dependency or
build preparation), package inclusion updates and exact-byte/hash verification.
No fixture, oracle, loader, dependency or coverage changes are made here.

## Hosted validation attempts

The [first PR run](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40376588026)
at head `9a64de2897b04b745aea922b895577e58b1b14ba` failed during the first
500-socket A/A warmup: the driver inspected raw frame byte 2 rather than splitting
bundled packets, so it did not count bundled own appearances and failed gameplay
validation. The collector also failed to classify early EOF as a failed probe
cell. No socket measurement from that attempt is accepted. The retained full
artifact holds stderr, partial A/A records and the failed attempt. Corrections
reuse the public protocol splitter, reconnect after death with a new socket
session, add a bundled-lifecycle regression and retain EOF probe failures.
