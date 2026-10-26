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
one-second warmup and 286-tick measurement window. Hosted validation below establishes the short-window feasibility at 500;
no local performance or sustained-capacity claim is made. They retain connected and
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

At commit `249cdb217d83a234895bc7eee4d98c3636c1cd76`, the `tests/fixtures`
Git tree is `9248b1d9afe335ae3f54d6a3f085818614980647` (unchanged in these PRs).
It has 42 tracked files totaling 12,292,562 bytes.
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

The [second PR run](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40377917826)
at head `920968ad60f2e4e7bd286c13da8bb1d8a762ad84` completed the report but accepted
neither socket row: child sockets closed during reconnect, leaving Unavailable
results. The driver must release the old TCP session before requesting a new
slot. A short one-tick functional smoke after that correction connected all 80
Battledome sockets and received gameplay; its debug timing is not performance
evidence. The full hosted workload remains the validation target.

That second run completed six A/A pairs for each deterministic row. Whole-case
CPU / operation-wall log residual SD percentages were: step-food 1.782 / 1.989,
observe 1.500 / 1.885, official-100-step 1.555 / 2.160 and official-100-bot
3.202 / 2.811. Different timing scopes prevent substituting whole-case CPU for
operation cost from those values alone. The runner's `perf stat -e instructions`
probe failed because hardware events were unavailable at its existing access
policy; Valgrind was not installed. No runner security settings were changed.

The [third PR run](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40379153116)
at head `027d6e5` completed with both socket rows Unavailable. Retained stderr
identified `Server is full` on replacement logins: dropping the old TCP session
alone does not wait for physics to consume its disconnect and free the seat.
The driver now retries deaths/capacity refusals after 100 ms, draining the other
clients meanwhile. The exact requested connected count is still required at
measurement boundaries, with a bounded untimed admission wait. This is a driver
lifecycle correction, not evidence that the runner cannot support 500 clients.

## Accepted host A/A samples and fixed metrics

The [manual Host calibration](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40380779843)
at `6ab29420e58276ee7a28360492bef82aba43c87e` and the
[PR-hosted pilot](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40381131394)
at `bef51d566657b795e11c2446591a01cdf4f70808` each completed six counted A/A
pairs plus one discarded warmup pair for both socket rows. All six report rows
in the latter completed and resolved Within ±5%, with no report errors. The
500-client row used eight comparison pairs; Battledome used twelve. Sampling
consumed 202.82 seconds, excluding builds/traces and the diagnostic pilot.

| Socket workload | Manual CPU / busy-wall log SD, % | PR pilot CPU / busy-wall log SD, % | Pooled CPU / busy-wall log SD, % | Fixed report metric |
| --- | ---: | ---: | ---: | --- |
| host-official-500 | 2.405 / 1.715 | 0.891 / 2.033 | 1.814 / 1.881 | Whole-host process CPU per physics tick. |
| host-battledome-80 | 1.185 / 0.635 | 0.921 / 0.690 | 1.062 / 0.663 | Simulation busy wall per physics tick. |

Pool within-job order-adjusted residual sums of squares over eight residual
degrees of freedom; do not pool job intercepts. For 500 clients the ranking
reverses between jobs, so there is no consistent quietest metric. CPU has a
slightly smaller pooled SD and covers gateway work as well as simulation; that
coverage motivates the choice. Battledome busy wall is quieter in both jobs.
CPU and busy wall remain in every raw result, and CPU rows expose their busy-wall
values in Details. No instruction-count metric was available.

In the manual calibration's twelve counted 500-client processes, every endpoint
had 500 connected sockets and 500 playing clients; the lowest observed playing
count was 497 during churn. In Battledome every endpoint had 80 connected
sockets, but only 15–16 playing clients (minimum 13), with the rest awaiting
admission. This is intentionally 80 socket clients stressing admission, not a
claim of 80 simultaneously living snakes. No client count reduction was needed.
Median process times were 5.146 seconds for 500 and 3.109 seconds for Battledome.

Both A/A jobs predate #168's host optimization; source revisions remain pinned
above. The final branch is rebased onto #168 without editing its implementation.
These short pilots establish tooling feasibility and describe their samples;
they do not qualify sustained 500-player capacity or validate the timing model.
Routine PRs use the fixed metric without a pilot. Recalibration remains a manual
`Host` workflow group, avoiding repeated calibration cost on every PR.

The [post-#168 PR run](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40382784368)
at `41fe897ef2232ad5cb49d4319eed0324b967df79` also completed without report errors:
all six timing rows resolved Within ±5%, ten of ten deterministic traces matched,
and sampling took 189.50 seconds without a per-PR pilot. This verifies the new
rows on the inherited host optimization; it is not a sustained-capacity claim.

A further [manual A/A job after #168](https://github.com/NikitaMGrimm/slither-sim/actions/runs/40383156762),
using the same `41fe897ef2232ad5cb49d4319eed0324b967df79` head, completed all six
pairs per row. The 500-client CPU / busy-wall log SD was 2.652% / 3.924%;
Battledome was 1.331% / 1.562%. This supports the 500-client CPU choice but is a
counterexample to uniformly quieter Battledome wall time. Keep the fixed
Battledome choice based on the two original jobs; a single new job does not
establish a replacement metric. Source changes and host variation mean these
three jobs are not a common-variance calibration or a proven noise winner.
All current raw results retain CPU and wall so later pilots can revisit this.
