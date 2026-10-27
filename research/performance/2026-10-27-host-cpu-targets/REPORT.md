# Host CPU build targets, 2026-10-27

Keep portable defaults. Offer operators an opt-in `target-cpu=native` build on
their hosting machine, then measure there. Native has the strongest descriptive
local cycle result, but this shared WSL2 run does not qualify native EPYC CPU
savings, latency or capacity. There is no runtime source change.

## Frozen inputs and scope

Runtime `21d02811b52a80ec7a66066100dbb4c787de5cff` is reviewed #218. Merged main
`660e322eb8d369aaba4fbd4cfd101177c3d4a223` differs only in CHANGELOG wording;
the selector checks identical production, build, example and fixture sources.
[Selected identities and results](results.json) pin the archive, all four
executables per target, toolchain and fixture hashes. Rust 1.99.0 / LLVM 23.1.1,
thin LTO, one codegen unit, release debuginfo=1, default features throughout.
Only `RUSTFLAGS` changes: empty, `-C target-cpu=x86-64-v2`, `x86-64-v3`, or `native`.
The prebuilt standard library is not rebuilt. This compares build configurations,
not source implementations or release artifacts.

Local AMD Ryzen 7 9800X3D, Linux x86-64 under WSL2. Native includes AVX-512 features
on this CPU and is not an EPYC Zen 4 measurement. All builds, traces and counters
are pinned to one logical CPU, one Cargo/Rayon/test job. Other workers and the
Windows host remain active. User-space events only, `perf_event_paranoid=2`
unchanged; no administrator changes or outside game connections.

## Whole-process paired counters

Each target/protocol comparison has one untallied warmup pair, then five pairs
alternating default/target and target/default. Every event is **100% scheduled**;
no samples are excluded or replaced. This is the complete official-500 fixture,
1,080 ticks including admission and reconnect, not a steady-tick bracket.

Instructions and cycles below are billions, medians of the five runs per side.
Cycle ratios are paired medians and full paired ranges (target/default).
**Instruction counts across targets are not directly comparable measures of
work or speed:** instruction mix, SIMD width and individual costs change.
Cycles are descriptive here too; scheduling, cache and virtualized/shared-host
effects remain. No elapsed-time, CPU-time or capacity claim is made.

| Target | Clients | Instructions default / target | Cycles default / target | Paired cycle ratio [range] |
| --- | --- | ---: | ---: | ---: |
| v2 | stock | 12.087035 / 11.488987 | 5.740554 / 5.752015 | 1.00471 [0.79427, 1.03461] |
| v2 | half protocol-13 | 12.427763 / 11.825840 | 5.877986 / 5.797625 | 0.99462 [0.89081, 1.07523] |
| v2 | all protocol-13 | 12.776298 / 12.165773 | 5.938620 / 6.002248 | 1.01071 [0.97988, 1.02272] |
| v3 | stock | 12.086323 / 11.278889 | 5.531129 / 5.467852 | 0.98856 [0.96611, 1.06016] |
| v3 | half protocol-13 | 12.430291 / 11.612064 | 5.827078 / 5.620264 | 0.96730 [0.95151, 1.00606] |
| v3 | all protocol-13 | 12.775301 / 11.949398 | 5.874950 / 5.694971 | 0.97172 [0.96286, 0.97753] |
| native | stock | 12.086572 / 11.247142 | 5.726006 / 5.457922 | 0.95487 [0.94734, 0.95660] |
| native | half protocol-13 | 12.430314 / 11.579296 | 5.863875 / 5.562360 | 0.95475 [0.94310, 1.01709] |
| native | all protocol-13 | 12.775560 / 11.917011 | 6.172674 / 5.911964 | 0.96495 [0.94393, 0.97904] |

V2's lower instruction count does not yield a consistent cycle improvement.
V3 improves descriptive medians by 1.14–3.27%, with stock/mixed ranges crossing
parity. Native medians improve by 3.51–4.52%; the mixed-protocol range still
includes a regression. These support trying native locally, not changing the
minimum CPU for archives, the published crate or wheels. No deployment occurred.

## Exact output and floating math

All **36 full-trace comparisons** pass: the same twelve cases for each advanced
target against default. These include three official-500 protocol variants;
historical portable/protocol, official-500, Battledome-64 and both retained-client
interfaces; and separate Battledome 40/64/80. Full decompressed bytes, order,
timestamps, initial/final checkpoints and RNG endpoints match. Terminal records
and population/work summaries are checked. No tolerance or output normalization
is introduced.

Source call sites include `Point::distance`'s `hypot`, motion/boundary/spawn
trigonometry, pickup nutrition's `powf`, kiosk `exp`/`ln`, and wire rounding.
Disassembly and symbols show the same imported `asin`, `atan2`, `sin`, `cos`,
`sincos`, `hypot`, `pow`, `exp` and `log` routines in every target. Default links
scalar `floor`/`round`/`ceil` helpers and has no packed/scalar rounding-instruction
sites; advanced builds omit those helpers and have 170–172 rounding sites.
This is a code-generation observation, not attribution of a whole-process gain.
No fast-math or reassociation flags are added.

The supplemental [bit probe](math_probe.rs) compares **86,562 outputs per target**
for the crate's math operations, representative movement/angle expressions,
seeded finite inputs, signed zero, subnormals, ULP boundaries and huge values.
Every output bit matches default, including separately evaluated trigonometry
and `sin_cos`. This sampled oracle and the full traces establish their tested
scope, not universal bit equivalence for every input, compiler, libc or CPU.

## Reproduction and retained material

From Linux x86-64 with Rust 1.99.0, Python 3.12+, patch and user PMU access:

```sh
STUDY=research/performance/2026-10-27-host-cpu-targets
RUNTIME=21d02811b52a80ec7a66066100dbb4c787de5cff
OUT=/tmp/slither-cpu-targets-new
python3 "$STUDY/reproduce.py" build "$PWD" "$OUT" --runtime "$RUNTIME" --cpu 3
python3 "$STUDY/reproduce.py" audit "$PWD" "$OUT" --runtime "$RUNTIME" --cpu 3
python3 "$STUDY/reproduce.py" trace "$PWD" "$OUT" --runtime "$RUNTIME" --cpu 3
python3 "$STUDY/reproduce.py" count "$PWD" "$OUT" --runtime "$RUNTIME" --cpu 3 --perf /path/to/perf
python3 "$STUDY/select.py" "$PWD" "$OUT" 660e322eb8d369aaba4fbd4cfd101177c3d4a223
```

The pinned recipient-work reproducer supplies identical fixture overlays. All
Python entry points, including historical trace children, reject `-O`.
The seven documentation tests pass. A one-bit mutation in a scratch copy of
v2's supplemental math output makes the selector fail at its bit-equality gate;
the original raw output remains intact. This checks the study gate, not a
runtime mutation (there is no runtime change).
Raw arrays, executables, source archives, disassembly and traces are local-only
under the task's scratch output; only bounded selected summaries/hashes are
committed. A transient new-session WSL connection timeout occurred during traces;
the existing runner continued and access recovered without a reset. No failed
measurement was discarded or retried.
