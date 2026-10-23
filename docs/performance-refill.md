# Ambient refill investigation, October 23, 2026

This is an **unpromoted experiment**, based on public runtime `20dd3aff` after
the previously merged correctness/collision/observation changes. It does not
reuse an older training-world bottleneck assumption. The
[selected evidence](performance-refill-20261023.json) retains every complete and
partial measurement, source/build/binary pins, populations, latency, throughput,
peak working sets and trace proof. Raw files remain in owned ignored outputs.

A separate current-tree `StepProfile` screen used seed 7, fixed 7 ms ticks,
10,000 ticks per process, one warmup and three samples. The dense fixture has
32 ring-start snakes, radius 6,000 and 12,000 food placement attempts. It retained
32 living snakes, at least 11,213 food, zero deaths and nine 1,000-tick episode
resets per process. Median measured phase costs per 1,000 ticks were 63.63 ms
for pickup and 16.29 ms for ambient refill. Default-world medians were 1.36 and
0.43 ms. These include profiling overhead and use a source-equivalent executable
separate from the comparison; they justify investigation, not a speed claim.

The candidate scans only the generated candidate's cell when exactly one refill
interval is due. Multiple attempts retain the existing shared occupancy map.
The branch condition uses the loop's exact next subtraction/epsilon comparison;
both paths include every origin and current public food edits. Candidate creation,
RNG consumption, clock subtraction, rejected intervals, target count and ordered
birth/admission remain unchanged. Defaults and the physics tick are unchanged.

The first inline form is `642a989b`; the revised form `02ed4b22` moves the same
scan into a private non-inlined helper. This layout experiment is a hypothesis,
not an established explanation of the first form's observation regression.

## Complete first comparison

One untallied warmup and five adjacent alternating pairs used the unchanged six
baseline workloads. The artifact interval was approximately 17:00:40–17:02:02 UTC,
81.2 seconds; that interval excludes initial metadata acquisition. The coordinator
and health peer agreed to hold compute. This shared desktop was not otherwise
proven idle. The machine was a Ryzen 7 9800X3D / Windows 11, using rustc/Cargo
1.99.0, LLVM 23.1.1, the locked thin-LTO release profile, one codegen unit and
jobs=2. No platform tuning or additional runtime threads were used.

| Case | Iterations/sample | Median paired CPU ratio | Median paired inner-wall ratio |
| --- | ---: | ---: | ---: |
| step-default | 100,000 | 0.857 | 0.866 |
| step-food | 10,000 | 0.765 | 0.815 |
| step-body | 50,000 | 0.955 | 0.988 |
| observe | 1,000 | 1.042 | 1.083 |
| portable | 5,000 | 0.975 | 1.010 |
| protocol | 5,000 | 1.000 | 0.990 |

Lower ratios mean less measured cost. Each ratio is candidate/baseline within
one matched pair; the table takes their medians and removes no outliers. Default
and dense-food CPU improved in all five pairs, while observation CPU worsened in
all five (range 1.011–1.150). Body, portable and protocol results were mixed. No
general speedup, performance-equivalence bound or production incidence estimate
is established. The candidate remains unpromoted.

Native inner intervals exclude setup, controls, resets and returned-output
destruction; process CPU/wall include them. CPU is Windows kernel plus user time,
with quantization. Peak working set measures resident pages rather than private
allocation. Exact per-sample tails, throughput, CPU and memory are in the evidence
file; they describe these different workloads rather than equivalent work units.

## Excluded and incomplete attempts

Before timing, artifact checks caught an unchanged candidate executable after
two source manifests reused one Cargo target. That attempt is retained and
excluded. Fresh candidate-only target storage produced distinct source-pinned
executables. No stale candidate measurements are included.

The helper comparison began at 17:17:25.065 UTC. Observed costs rose markedly;
the parent failed to enforce its agreed 120-second window and stopped only its
owned runners/children at 17:20:35.628, after 190.557 seconds. At 17:20:55.503 a
process inspection verified zero remaining owned measurement processes. All
partial rows remain explicitly **incomplete**, without a comparison summary,
completed post-run hash check, selected helper medians or performance conclusion.
The cause of the cost increase is not established. No retry is included.

Future comparisons need an enforced shared deadline and cleanup reserve, rather
than an expected-duration estimate. The separate runner correction introduces
that control; it does not qualify this incomplete experiment.

## Output checks and remaining work

Both forms passed the full Rust group: default/core-only all-target tests,
doctests, Clippy with warnings denied, formatting and the basic example. Nineteen
focused ambient/pickup/core tests passed. Independent source review found no
issues in admission or helper extraction; the reviewer did not execute checks.

`examples/perf_ambient_parity.rs` is an untimed synthetic trace harness with eight
bounded fixtures and seeds 7/19: 16 runs and 2,968 ticks. It exercises default
single attempts, epsilon/fractional credit, multiple attempts, disabled caps,
every food origin/public edits, target changes and rate setters. Complete bytes
match the unchanged runtime, including serialized private RNG/clocks, ordered
food and events. Five small fixtures also match with full state on every tick.
It contains no replacement refill model and does not cover snakes/prey itself.

Both forms additionally match full normal traces for 1,000 default ticks,
128 dense-food ticks, 32 body ticks, two observation batches, 32 portable decisions
and two protocol replays. These include state, events, observations, frames and
wire. Only object-key order and the unordered `prey_outward` set are canonicalized.
The identical harness bytes and exact digests are retained in the evidence file.

Remaining priorities, ranked as investigations:

1. Complete the helper comparison under a shared deadline, retaining observation
   and portable controls. Its performance is still unknown.
2. Attribute the larger pickup phase before changing its candidate-mask traversal
   or removal flags. These are source hypotheses, not demonstrated bottlenecks.
3. Run the validated isolated observation diagnostics to split filtering, ordering
   and copying; preserve the wide-world counter-case.
4. Expand the validated retained-client harness to its full 384 decisions and
   inspect actual map/reconnect/sector coverage before optimizing client state.

No hosted, Arena, policy/training or official-server gain is claimed. No defaults,
visibility policy, protocol semantics or scientific approximations are changed.
