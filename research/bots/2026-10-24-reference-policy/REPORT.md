# Portable reference policy, 2026-10-24

This is a synthetic, offline engineering study of the crate's shared reference
policy. It uses protocol-19 received-state `slither-client-v2` frames, not debug
snapshots or geometric `serve` frames. No outside game server was contacted.

## Question and implementation

Can the revised geometric design become a simple, allocation-free portable
reference that separates useful play from circling, idle and suicidal controls?

The kernel lives in `src/reference.rs`, outside the networking feature, so the
host, future load generator and examples can share one observation-only function.
`client::ReferenceBot` is a zero-sized, stateless adapter; no new trait, tuning
surface or policy ABI is needed. It checks the frame contract/lifecycle, then
passes only the observation to the kernel. The host's existing seekers are not
replaced in this change.

Food is reconsidered every decision, minimizing distance plus twice the turning
radius times heading change. Thirteen headings are scored using four 90 ms
turn-limited motion samples. Boundary, opponent-head and body-edge clearance
penalties use the design prototype's policy margins. The head-to-first-anchor
edge is included and the existing `physics::segment_distance` helper is reused.
Food cost is computed once per pellet, preserving first-item ties. Boost is off.
No simulator, debug or other slot's observation is available to the kernel.

The earlier geometric pilot at `f36df18` remains exploratory evidence, local-only
under `C:/Users/Nikita/.t3/scratch/bot-capacity-design-20261024`. Its thresholds
and strength results are not relabelled as portable results. Main already contains
serve v2 via #61 (`2e7e387`); this implementation starts from `a019c2b`, with no
serve reconciliation.

## Frozen method

[`plan.json`](plan.json) is the predeclared plan. All controls see their own
decoded `ClientFrame` through `SimulatorBackend`, with fixed zero processing
time, no added link latency, six 7 ms physics ticks per decision/broadcast,
`Config::official(16)`, 180 simulated seconds and no respawn. The same decoder,
command quantization and receipt scheduling serve native and network clients.
Wall-clock decision measurements never feed back into skill trajectories.

Each world has eight references and two each of greedy food seeker, circle,
idle (no steering commands) and outward-suicide controls. Policy assignments
rotate by zero and five slots. Calibration uses seeds 31001–31024; validation
uses fresh seeds 41001–41024. The policy and threshold rule are fixed before
fresh validation. Reference self-comparison splits the eight references into
two groups of four. Other controls compare their two-slot mean with the eight
reference seats in the same world. These are cohort comparisons, not individual
entrant qualification or a balanced tournament.

Metrics are alive-tick fraction and summed picked-up `Food.value` (stored
newborn-equivalent nutrition, excluding prey), measured by the referee every
physics tick. This is not size-dependent actual mass growth. Dead remainder
counts in the survival/food denominator. Score AUC and kills are retained as
diagnostics. The policy sees none of the referee's world/pickup data.

Average both rotated-world ratios within each seed. Thresholds are reference
self-comparison calibration fifth percentiles rounded downward to 0.05. Require
separation from idle/suicide survival and circle food calibration p95 before
validation; otherwise the calibration is inconclusive. Qualification requires
both lower endpoints of 10,000-resample seed-block bootstrap 95% intervals for
mean paired ratios to clear their frozen thresholds. No per-snake/decision
pseudoreplication, validation retuning or epsilon for zero reference metrics.

## Results

**The reference passes this portable cohort qualification; all four negative
controls fail.** The 48 calibration worlds and 48 fresh validation worlds are
all retained. Their nominal 180 s horizon is 25,714 ticks (179.998 s).

Calibration reference/reference p05 ratios were **0.82648 survival** and
**0.58874 food**, giving frozen gates **0.80 / 0.55**. Calibration p95 survival
ratios were 0.10737 for idle and 0.06245 for suicide; circle's food p95 was
0.02837. The gate was written before fresh validation was collected.

| Fresh validation cohort | Survival ratio, mean [bootstrap 95% interval] | Food ratio, mean [bootstrap 95% interval] | Qualified |
| --- | --- | --- | --- |
| Reference/reference | 1.007 [0.943, 1.076] | 1.053 [0.944, 1.169] | Yes |
| Greedy | 0.675 [0.599, 0.753] | 0.239 [0.196, 0.282] | No |
| Circle | 0.997 [0.924, 1.070] | 0.022 [0.019, 0.025] | No |
| Idle | 0.076 [0.070, 0.082] | 0.029 [0.027, 0.031] | No |
| Suicide | 0.053 [0.051, 0.054] | 0.014 [0.013, 0.015] | No |

The 384 reference lives in validation had mean absolute alive fraction 0.851
and normalized food intake 74.68. Survival alone admits the circler. Requiring
every reference life to survive the whole match would reject this reference.
The two seed-41001 worlds were rerun and matched validation JSONL bytes exactly,
including decision checksums and referee outcomes. No policy/threshold change
was made after inspecting either pack.

### Decision cost

Windows 11 Pro 10.0.26200, Ryzen 7 9800X3D (8 cores/16 logical processors),
rustc 1.99.0 MSVC/LLVM 23.1.1, default features, release/thin LTO/one codegen unit.
The Windows `start /b /wait /affinity 4` launcher pins the process to one logical
CPU. This is a shared desktop; no quiet-host claim. Rust/tools checks and the
competence runs finished before timing, and no repetitions were discarded.

The corpus comes from 240 decisions of all-reference `Config::official(n)`
worlds, seed 7, capturing every fifteenth decision's playing **received** frames.
The initial waiting sample is excluded. Dead slots contribute no frozen frames;
this is decision cost, not occupancy or capacity. One pass warms the corpus;
three batches of 200,000 adapter calls cycle through it with `black_box`.

| Initial snakes | Frozen frames | CPU µs/decision, median [min, max] | Elapsed µs/decision, median |
| ---: | ---: | ---: | ---: |
| 4 | 60 | 5.94 [5.55, 8.13] | 5.91 |
| 16 | 240 | 9.84 [9.77, 10.16] | 10.46 |
| 64 | 951 | 10.70 [10.55, 11.25] | 11.84 |
| 128 | 1,917 | 12.11 [12.03, 12.89] | 13.46 |

CPU is `GetProcessTimes` user+kernel delta around the call loop, including the
adapter and corpus lookup, excluding construction, decoding, simulation, JSON
and output. Its counter resolution is visible in the raw values. The elapsed
and CPU medians can differ in either direction because of scheduling/counter
resolution; do not interpret their difference as separately measured overhead.
Other platforms report elapsed cost and an unavailable CPU field in this probe;
Linux scored cgroup accounting remains step 3. Frames are from short, initially
small-snake runs, not a mature dense workload. This is not a speedup comparison
with the differently sourced/informed geometric pilot.

The adapter has **zero bytes of instance state**, and the decision-allocation
test observes zero allocations for repeated food/body-heavy inputs. This does
not mean zero decoder, observation, frame-corpus or server memory.

Selected values, environment, exact source hashes and raw-file hashes are in
[`results.json`](results.json). Measured executable SHA-256:
`5d5ca8daf68ecacd45e1ebab12f337e12050342d2116bb951f32083f30cab7c0`.
Base `a019c2b3f63f2e555d2a71984392dfc9b222e2b6`, plus the exact kernel/adapter/
harness/plan/analyzer hashes recorded there, identifies the measured source
while this PR's documentation/results were still being written.

## Verification and limits

Focused deterministic tests cover food reselection/turn cost/ties, wall/body/head
avoidance, translated arena centers, repeated anchors, large snakes, unchanged
inputs, zero decision allocations, adapter lifecycle and stateless reset. A
loopback WebSocket transports the local backend's exact packet tape into a
persistent client decoder at the retained virtual receipt times; full frames and
reference decisions must match, including reconnect. This proves matching-input
parity, not identical trajectories under arbitrary real network delays.

Passed locally: `uv run --locked --only-group tools python -B tools/check.py rust`,
the `tools` group, the ten focused reference/adapter tests and
`cargo package --locked --allow-dirty`. The first adapter test assumed the native
backend immediately played; it correctly returned `waiting`. The fixture was
fixed to advance the login/setup ticks, with no policy change. Both attempts'
logs are retained. Linux/macOS/MSRV execution is left to PR CI.

This study does not establish combat strength, human-level play, hidden-seed
robustness against every adversary, enforced decision deadlines or server capacity.
Steps 3–5 still need isolated entrant workers, deadline/fallback rules, balanced
matches/replay, sustained respawn qualification, host/generator tail instrumentation
and audited population/protocol bounds. Battledome scoring waits for #75 to merge;
its settled 400-point arc spawns, 400 mass floor with no free-boost pellets,
3584/1784 radii, 2684 spawn ring, 900-second kill window and no prey are not tested
by this official-map study. The first capacity composition remains 1:1 remote
players/server bots; no AWS resources were used.

## Reproduction and evidence

```sh
cargo test --locked --test reference --test client_reference
cargo build --release --locked --example reference_study
reference_study matches 31001 24 180 > calibration.jsonl
uv run --locked --only-group tools python research/bots/2026-10-24-reference-policy/analyze.py freeze calibration.jsonl frozen.json
reference_study matches 41001 24 180 > validation.jsonl
uv run --locked --only-group tools python research/bots/2026-10-24-reference-policy/analyze.py validate validation.jsonl results.json --frozen frozen.json
reference_study matches 41001 1 180 > replay.jsonl
reference_study cost 16 200000
```

Use the built `target/release/examples/reference_study` executable (`.exe` on
Windows); pin to one logical CPU with the OS launcher. Fresh output paths are
required by the analyzer. Selected summaries/hashes accompany this report;
full JSONL, logs and binaries stay local-only outside Git under
`C:/Users/Nikita/.t3/scratch/bot-capacity-design-20261024/reference-policy`.
A local path/hash identifies evidence without making it publicly retrievable.
