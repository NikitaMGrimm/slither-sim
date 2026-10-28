# Client frame materialization, 2026-10-28

Keep: exact materialization changes reduce the complete official-500
`next_frame` instruction cost by **2.7467%** (five alternating pairs; range
2.7466–2.7468%). Client frames, ordered bot actions and world traces match.
[Selected results, build pins and trace hashes](results.json). Baseline runtime:
`0aba6f97070a06c73b604869761d4c5c14965a78`; candidate:
`64490e2edf6339221ed5aeacef126e90b0da0df7`.

## Change and profile

The fresh official-500 profile still finds most bot-path instructions outside
policy. Broadcast is the largest stage, followed by policy and frame
materialization. Nested diagnostics put ordered food materialization at about
17.3k instructions per decision, metadata at 6.7k, HUD at 4.9k and head prediction
at 3.3k. Brackets include diagnostic overhead and are not additive savings.

Fresh final-arm stage profiles, thousand user instructions per background bot
decision, same 99,793 decisions. These are separate diagnostic runs, not the
plain paired comparison:

| Stage | Base k | Candidate k |
| --- | ---: | ---: |
| World-wide broadcast, amortized | 65.556 | 65.465 |
| Delivery decoding/application | 13.678 | 14.398 |
| `ClientFrame` materialization | 43.055 | 36.196 |
| Shared `ForagerBot::decide` | 59.417 | 59.417 |
| Four bot stages | 181.706 | 175.476 |
| Complete `next_frame`, same denominator | 231.059 | 224.773 |

Frame materialization falls 15.9%; the four-stage sum falls 3.43%. Decoding
grows because completed intervals are now integrated at report time. Broadcast
remains the largest stage. The complete interval also includes physics,
admission, command work and destruction outside the four brackets; their
different scopes must not be added. #242's older broadcast figure does not
describe the remaining cost on this task-start main.

Without phase hooks, the five-pair median total is **229.253k -> 222.956k user
instructions per background bot decision**, a saving of about 6.297k. This
separate whole-interval result includes physics and background admission;
it is the end-to-end instruction comparison. One untallied warmup pair is
retained separately. All twelve invocations preserve the accepted world
endpoint, and every counted bracket has 100% PMU scheduling. No sample is
excluded or replaced. Repetitions describe one fixed seeded continuation,
not independent-world uncertainty.

Two independent changes preserve all received fields:

- Cache the food tree's ordered materialization until insertion/replacement,
  successful removal or nonempty sector clear. Each returned frame owns a
  separate contiguous copy. Reconnect replaces the entire cache with the
  session. `Food` contains only values; making it `Copy` lets vector cloning
  copy those values in bulk. Clone-to-copy caller edits are mechanical.
- Fold completed head-motion intervals when the next report arrives, in the
  original left-to-right floating-point addition order. Polling integrates
  only the unfinished last interval. Report clocks are already monotonic;
  equal-time intervals, anchor resets and the original oldest-report fold all
  retain their coordinate bits.

Both `SimulatorBackend` background slots and host `Bots` call the shared
`ClientState::frame`. The sole A4-mean policy is unchanged. All deliveries still
use the original encoded bytes and decoder. No authoritative observation path,
structured-delivery bypass, delivery-clock change or reduced simulation rate.
Spawn-admission geometry is untouched.

Delivery integrates main `4e68c8047e2ff61c985f449506ff7bca5273c3ff`, including
the separately qualified spawn-admission optimization and v0.7.1 release.
The client implementation, `Food` declaration, shared policy and new regression
tests equal the measured candidate exactly. Counts remain pinned to the
baseline/candidate above; they are not relabelled as a new measurement of the
combined runtime. The merge conflict was the released changelog section; the
new entry stays under Unreleased while main's released entries are preserved.

## Method and parity

Offline synthetic official 500, seed 7, official delivery, 1,000 warmup decisions
then 200 counted decisions. As in #242, one controlled seat uses scripted
headings; the other 499 use A4-mean through the background client path.
Reuse #242's `bots.rs`, `counter.rs` and bot-stage
hooks at `ccc87eddd9439d375eedab62e66e642313f0bb5a`; adapt current hook locations,
population selection and untimed trace output rather than copying the policy
or rebuilding the workload. Every counted continuation has 99,793 background
decisions; reported living population ranges from 498 to 500 and ends at 500,
with 497 deaths and mean terminal length 10.988 segments over the complete run.
This 50.4-second fixture is not a mature ecological equilibrium claim.

A 4-vCPU Zen 5 EPYC KVM guest, Linux x86-64, Rust 1.99.0, locked offline default
features, portable release, thin LTO, one codegen unit and debuginfo=1. One
logical CPU per process, Cargo jobs/Rayon/test threads one. Another
instruction-count worker shares the guest; no timing-sensitive holds or system
changes. Report current-thread user instructions only, with >=99% scheduling
required for every counted bracket. Separate instrumented profiles attribute
stages. Five alternating `next_frame` bracket pairs without phase hooks follow one
untallied warmup pair; startup, the first 1,000 decisions and outer controlled
action submission are excluded. Physics, background lifecycle, delivery,
decoding, policy and background-frame destruction remain inside `next_frame`.

The final bot trace is valid, byte-identical JSONL: 1,200 decisions, 597,804
ordered background actions, 1,200 controlled-client frames and 202 complete
world checkpoints (first decision, warmup endpoint and every counted decision).
The terminal marker and donor report are parsed separately. The trace digest is
`5fcf1ae34b5a017c7d5349797ce0f0a7795e7ead432933e5d6b13e527bce3bf5`.
The world snapshots include authoritative RNG and lifecycle state; private
transport queues are not serialized. Exact frame regressions and ordered
actions complement this finite trace evidence.

All 23 deterministic catalog rows are compared with one shared harness and
their registered trace windows. These include portable/protocol, every
sandbox/official/Battledome step and bot row, both client rows, and the shared
A4-mean row. Socket rows have no deterministic trace and are excluded.

The independent bounded-replay oracle compares head-coordinate bits through
1,100 irregular/equal-time reports, zero speed, large clocks/coordinates,
anchor resets and multiple passes through the old 256-report cap. The food
regression compares complete warmed frames with a fresh replay after each edit,
including same-key replacements, ordered insertions, deletions, sector clears,
clone/repeated polling, ownership, rejected bundles and reconnect. Perturbing
motion duration or removing food-insertion invalidation fails the respective
oracle; exact restoration passes.

The complete Rust group passes at the measured candidate, including all Clippy feature configurations,
default/core/bot/networking all-target tests, doctests, C ABI, Python crate check
and the basic example. All seven documentation tests also pass with the final
study, index and changelog. After integrating main, all library unit tests and
the protocol, received-state, simulation, spawn, server, runner, transport and
contract integration suites pass, as do formatting, all-target workspace
Clippy and documentation checks. The entire feature matrix was not repeated
after this merge; the measured client files are unchanged. No independent agent
review, full tools group, native socket hold or Windows runtime build is claimed.

## Reproduce and limits

See [plan](PLAN.md) and [attempts](ATTEMPTS.md), including rejected pilots,
interrupted superseded runs, the stdout trace-flush correction and the guest's
compiler-wrapper failure. The host wrapper drops Cargo's hyphenated binary-path
environment key; test commands use a task-local direct Rust compiler path.
No production test or system wrapper is modified.

Export `base.tar` and `candidate.tar` at the full pins above, and `donor.tar`
at the population-scaling pin restricted to its study directory. With Linux
user-PMU access, Rust 1.99 and cached locked dependencies, use fresh storage:

```sh
STUDY=research/performance/2026-10-28-client-frame-materialization
ARCHIVES=/path/to/archives
OUT=/path/outside/repository/new-client-frames
mkdir "$OUT"
for ARM in base candidate base-profile candidate-profile trace-base trace-candidate; do
  python3 "$STUDY/reproduce.py" build "$ARCHIVES" "$OUT" --arm "$ARM" --cpu 1
done
python3 "$STUDY/reproduce.py" trace "$ARCHIVES" "$OUT" --cpu 1
python3 "$STUDY/validate_trace.py" "$OUT" --cpu 1
python3 "$STUDY/reproduce.py" matrix "$ARCHIVES" "$OUT" --cpu 1
python3 "$STUDY/reproduce.py" profile "$ARCHIVES" "$OUT" --cpu 1
python3 "$STUDY/reproduce.py" pairs "$ARCHIVES" "$OUT" --cpu 1
python3 "$STUDY/select_results.py" "$OUT" "$OUT/selected.json"
```

Use `verify.py CLEAN_CANDIDATE CHECK_OUTPUT --cpu 1` for mutations and checks;
on wrapper installations set `RUSTC`/`RUSTDOC` to the direct toolchain binaries.
Raw source archives, generated Rust, per-arm build pins, copied binaries, logs,
all attempts and compressed traces remain outside Git. Targets are bounded and
deleted after copying/hash verification or completed tests. The final driver
adds trace-only flushing, validation and explicit arm selection; original
counting-binary generated sources and hashes remain retained. Hashes identify
artifacts; they do not establish performance or correctness by themselves.

No 2,000-seat qualification: persistent free space fell below 1 GiB retaining
the 500-seat trace and matrix; a larger trace pair was not attempted in that budget.
No extrapolation to higher populations, playing strength, elapsed-time speed,
deadline tails or host capacity. A food snapshot adds one retained ordered
food vector per polled session, bounded by the existing decoder food limit;
peak memory is not measured here. Clients still receive exactly the reports
they previously received, with every snake simulated on every physics tick.
