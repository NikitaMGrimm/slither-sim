# Broadcast visibility distances, 2026-10-28

Keep: exact guarded snake/prey visibility reduces plain host-tick user
instructions by **3.29% at stock-500**, **3.09% with protocol 13**, and **3.02%
at 2,281 seats**. Every one of 23 catalog traces and four host/wire traces is
byte-identical. [Selected counts, source/binary pins and trace hashes](results.json).

These are short-body synthetic fixtures, not the mature worlds in PR #248.
They establish an instruction saving on the stated work, not elapsed-time
speed, long-running capacity or recovery of the mature world's budget gap.

## Attribution before implementation

The initial phase/caller census preceded production edits. Supplemental
calibration uses the same immutable baseline. Shared-libm instructions account
for **9.81% of stock-500** and **8.79% of 2,281-seat** plain host-tick
instructions here. They do not reproduce #248's 45.1% mature-world sample.

Corrected callee instructions from the tagged diagnostic, divided by the
separate plain host-tick instruction denominator:

| Call site | 500 | 2,281 |
| --- | ---: | ---: |
| `covered`: head `hypot` | 2.096% | 1.893% |
| `covered`: segment `hypot` | 1.050% | 0.979% |
| Recipient prey visibility `hypot` | 1.280% | 1.176% |
| Incremental anchor `atan2`/`sincos`/distance | 0.128% | 0.113% |
| Full snake appearance math | 0.009% | 0.008% |
| Food-interest heading `sincos` | 1.002% | 0.881% |
| Physics `advance_directed` math | 2.743% | 2.413% |
| Physics head-tip `sincos` | 1.002% | 0.881% |
| Physics prey math | 0.360% | 0.314% |
| Other tick libm callers | 0.141% | 0.125% |
| **All tick libm** | **9.813%** | **8.785%** |

The covered predicates make 4,587,706 calls and prey visibility 1,861,705 calls
in stock-500, against 32,093 incremental-anchor `atan2` calls. Frequency rejects
anchor trigonometry as the first target in this fixture. Angle normalization's
`fmod` is statically linked from Rust compiler builtins, so it does not explain
the shared-libm weight. The implementation targets visibility, leaving physics,
angle encoders and the existing delta sharing unchanged.

Separate phase brackets, million instructions per tick, include instrumentation
overhead. The final four rows nest inside broadcast; they must not be added to
it. The residual includes shared preparation, candidate/history work, HUD,
framing/conversion and instrument boundaries.

| Phase | Stock-500 | Protocol-13 500 | Stock-2,281 |
| --- | ---: | ---: | ---: |
| Complete instrumented tick | 8.053 | 8.505 | 41.063 |
| World physics | 2.010 | 2.029 | 11.168 |
| Food-interest advance | 0.294 | 0.294 | 1.332 |
| Broadcast inclusive | 5.597 | 5.996 | 27.991 |
| Snake coverage | 1.090 | 1.082 | 5.146 |
| Snake encoding/history | 1.265 | 1.248 | 5.909 |
| Recipient food | 0.653 | 0.654 | 3.888 |
| Recipient prey | 0.667 | 0.667 | 4.026 |
| Broadcast residual | 1.922 | 2.345 | 9.022 |

## Plain instruction comparison and parity

One discarded warmup pair, then five alternating pairs. No pair is excluded.
All counter intervals meet the 99% scheduling requirement. Reductions below
use each pair's ratio; Base/Candidate are medians. Host rows measure all
1,080 advance calls, excluding outer setup, controls and drains.

| Fixed work | Base / candidate | Median instruction reduction (five-pair range) |
| --- | ---: | ---: |
| Stock-500 host tick | 6.949 / 6.721 M/tick | 3.2899% (3.2889–3.2910%) |
| Protocol-13 500 host tick | 7.426 / 7.196 M/tick | 3.0907% (3.0902–3.0911%) |
| Stock-2,281 host tick | 36.063 / 34.973 M/tick | 3.0228% (3.0227–3.0245%) |
| `client-official-500 24`, outer fixture | 5.275 / 5.140 B/call | 2.5573% (2.5567–2.5575%) |

Outer host-fixture reductions are separately 3.0661%, 2.8469% and 2.8141%;
their setup/control/drain work is not silently counted as world work. These
fixed-repeat ranges describe one deterministic input, not confidence intervals
over different seeds or server populations.

All registered non-socket catalog trace windows and all four raw host variants
match complete decompressed bytes and validated terminal records. Host traces
retain frame order, timestamps, complete initial/final world checkpoints and
spawn/latency RNG endpoints. Stock/mixed/legacy minima are 499 living, ending
at 500, with three reconnects and no refusals. The larger row stays at 2,281
living, with the forced reconnect and no refusals. Maps and every actual frame
count remain in the selected summaries. These are not real-socket tests.

Multiplying the squared-radius comparator by 0.98 changes both stock and legacy
wire traces, and the same byte comparer rejects them. Changing the guard's
boundary sign from minus to plus fails the independent projection oracle;
restoring exact candidate bytes passes. The complete offline Rust group passes,
including all Clippy feature configurations, all-target feature tests,
doctests, C ABI, Python crate check and basic example. Debug/release geometry
oracles and all seven final documentation tests pass. The final reproduction
recipe regenerates every Rust source hash of all four measured build arms.

## Runtime change

Reuse `Point::distance_le` for head and prey visibility. For edges, share the
original scalar segment projection with a private guarded comparison. Clear
cases use the existing squared-distance guard; ambiguous and extreme cases
still call the original `hypot` predicate. The guard is not a new visibility
tolerance. Physics, recipients, packet ordering and report gates are unchanged.

The independent regression freezes the original projection expression and
compares coordinate-result bits and predicate results at the computed radius,
its adjacent floats, random radii, zero-length segments, signed zero, overflow,
underflow and nonfinite inputs. The existing point comparison oracle separately
checks the guarded circle predicate.

## Method

Baseline runtime `d127acd1f0e9db2100c8a1dd5c2fa43e2deff506`; behavior candidate
`9c3b29da` (full identities accompany selected results). Main's intervening
PR #248 changes research/docs only, with identical runtime sources and lockfile.
Read its [capacity report](../2026-10-28-official-density-capacity/REPORT.md)
before collection. These short fixtures do not recreate its mature worlds.

A 4-vCPU Zen 5 EPYC KVM guest, Linux x86-64, Rust 1.99, locked offline release,
portable CPU, thin LTO, one codegen unit and debuginfo=1. One bounded target;
one build/Rayon/test worker and one logical CPU per measured child. No package
installs, guest configuration changes or outside game servers. Raw process
command lines remain outside Git, with the account credential redacted.

The existing seed-7 official-500 host/wire fixture runs 1,080 ticks, scripted
heading/boost inputs, deaths, reconnections, maps and original admission. Stock,
mixed and protocol-13 clients share the same physical world. The 2,281-seat
stock extension changes only population and corresponding official world size.
The existing `client-official-500 24` row complements these remote-host paths;
its process includes client decoding/materialization and is not world-only.

Separate PMU phase builds split world, interest, inclusive broadcast, coverage,
snake encoding, food and prey. Coverage booleans are batched per recipient only
in the diagnostic copy. Read boundaries and its temporary vector add overhead;
the parent sums are diagnostic budgets, not plain performance results.

The accepted counting shim keys libm calls by return address, resolves
inline source owners, and calibrates the common measurement routine against a
verified one-`ret` control. Each process requires 64 stable controls and at least
99% PMU scheduling. Known 32-NOP-plus-return probes additionally recover exactly
33 instructions for all three signatures. Corrected libm instructions are raw
bracket minus the corresponding control plus one. Plain and phase-tagged runs
have exactly matching call counts per libm function; their aggregate corrected
instruction totals differ by less than two parts per million. These are user
instructions, not elapsed time.
An attempted external IP/callchain sampler disagrees materially with the
fixture's internal counter, even with a pre-main stop/attach handshake. Its
cause is unresolved and its percentages are excluded from accepted budgets.
Uncalibrated and sampling pilots remain outside Git; they are not reassigned
to callers or used as speedup evidence.

The plain comparison freezes one discarded AB warmup then five pairs ordered
AB, BA, AB, BA, AB for each row, with >=99% PMU scheduling. Outer-fixture counts
include setup, control and drains, excluding process startup/final counter
writing; separate host tick brackets exclude those
outer fixture costs. Builds, tests and traces precede the comparison. Full
decompressed bytes and terminal markers are checked for every available
deterministic catalog row and every host/wire variant. Deliberate radius and
guard-boundary perturbations test wire comparison and the geometry oracle.

## Reproduction and limits

[Plan](PLAN.md), [retained attempts](ATTEMPTS.md). Export `base.tar` and
`candidate.tar` at the measured source pins into fresh task-owned storage.
Transfer this directory's helpers alongside them. With a fresh absolute output
directory `OUT`, run:

```sh
python3 reproduce.py build "$OUT" --arm base
python3 reproduce.py build "$OUT" --arm candidate
python3 reproduce.py build "$OUT" --arm mutation
python3 reproduce.py build "$OUT" --arm base --profile
python3 verify.py "$OUT"
python3 reproduce.py trace "$OUT"
python3 reproduce.py mutation "$OUT"
python3 reproduce.py profile "$OUT"
python3 reproduce.py profile "$OUT" --plain-libm
python3 reproduce.py pairs "$OUT"
python3 reproduce.py reconstruct "$OUT"
python3 select_results.py "$OUT" "$OUT/selected.json"
```

Raw archives, generated source trees, binaries, process inventories, build/test
logs, complete traces and all attempts remain outside Git. Source reconstruction
verifies the final recipe despite later driver additions for validation and
selection. The guest endpoint's cooldown paused verification; the same process
and artifacts survived, and every comparison pair ran after verification finished.

No mature ecological hold, real-socket capacity qualification, elapsed-time
speedup, Windows execution or independent agent review is claimed. Finite trace
equality is engineering evidence for the stated cases, not a universal proof.
