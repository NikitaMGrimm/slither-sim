# Frozen Forager port — 2026-10-26

This change ports the received-observation Forager without changing runtime bot
selection or simulator mechanics. A1 is the sole public `PolicyId`. A4-mean is
retained as test-only resolved data; it is not adopted by this port.

## Source and parameters

The oracle is the untouched kernel at
[`be3a035dfab0c5717bef90daf88c6d3ada0e0ffe`](https://github.com/NikitaMGrimm/slither-sim/tree/be3a035dfab0c5717bef90daf88c6d3ada0e0ffe).
The port starts from main `77f23d4b53eb7324f6f77f68a74674de2b2450e6`.
The [manifest](../../../src/bot/data/resolved.json) pins the original kernel and
adapter hashes, all 54 parameter fields, all 16 fan entries and fan length.
Floating parameters use raw binary64 hex bits; integer and boolean types remain
explicit. Runtime releases never resolve against evolving defaults.

Original override files are retained byte for byte:

- [A1](../../../src/bot/data/a1-overrides.json): SHA256
  `0aa7682bd5989828b58fdcfc74c3db2332d1f0d617d28a76252ca6ab652f5471`.
- [A4-mean](../../../src/bot/data/a4-mean-overrides.json): SHA256
  `0c5f57ea712ce86e796bff079bb5911b9cd1c2217fb9d8d808bb9f061bd2719a`.

These two immutable inputs retain their original CRLF bytes through a narrow Git
attribute exception. Source, manifests and other new text use LF.

Both are overrides. Omitted fields use the frozen kernel's defaults. An override
fan replaces the whole fan with a zero-filled array, copies its entries and sets
its length, matching the historical loader. The standalone oracle independently
checks every resolved field against that resolution.

The private historical study adopted A1. Its original A4 confirmation did not
satisfy every adoption requirement; its cost measurements were contaminated and
a separate quiet rerun was pending at port preparation. Those studies used
modified training scenarios and older mechanics. Their full inputs and tooling
are private and are not reproduced here. This port makes no new playing-strength,
official-server fidelity or cost claim and does not replace those failed attempts.

## Mechanical source audit

The kernel's tokens match be3a035 after excluding comments, visibility changes
and making private parameter validation test-only. Arithmetic, loop order, ties,
limits and inline regressions are preserved. Imported integration regressions
remain as private unit tests so raw tuning parameters need not become public.
The twelve-case [legacy native fixture](../../../tests/fixtures/forager-native-parity.json)
retains its historical default parameters and zero timing; it is not an A1 oracle.

The physics helpers used by the predictor retain their arithmetic: `advance`,
`base_speed`, `pickup_circle`, `scale`, `segment_distance`, `swept_distance`,
`turn_rate`, the relevant speed-state methods and point geometry. Main's removed
client-anchor helper and changed `crossed_anchor` signature are not used.
`LETHAL_OUTWARD` only names the existing boundary literal; `constrain` retains its
arithmetic. No world-generation, applied-action or reference-opponent changes
are included.

The adapter preserves minimum valid receipt age, measured RTT splitting,
processing duration, ping-range uncertainty, the first 64 sent commands in input
order and opponent-ID alignment. It preserves the first wanted-history report
rule, including its treatment of the report's validity flag, nearest-251-step
snapping and unknown-Intent NaNs. It reads no authoritative state.

## Selected conformance data

The [selected fixture](../../../tests/fixtures/forager-conformance.json) currently
contains **44 prepared inputs and no oracle expectations**. It intentionally
cannot pass conformance in this draft. Generation and execution are pending
because the desktop's measurement lock prohibits local Rust builds and tests.

Twelve full records were selected at indices 0 and 17 from each of six private
local/synthetic corpus groups, without inspecting policy outputs. Their source
record hashes identify private inputs, not downloadable public archives. The
complete selected frames are committed. A complete 32-decision synthetic receipt
tape adds RTT variation, empty/short/64/over-limit histories, boost toggles,
respawns and reconnections, missing observations, incompatible contracts,
stale/missing receipts, reordered opponent rows, dense/long bodies, prey, racing,
rim and exit inputs. These are controlled frame sequences, not live world traces.

All floating frame values are `{ "f64": "<16 hex digits>" }`; signed zero and
array order survive decoding. Expected Timing and Intent values use raw hex bits,
including unknown-Intent NaNs. Expected actions record presence, heading bits and
boost. No public-mode guard is applied. The conformance test checks derived input
bits before output bits, then replays forward/reverse with shared and fresh
scratch and verifies input preservation. The allocation regression also exercises
the immutable A1 and A4-mean parameter sets after warmup.

Pending additions: all four seats at every decision in a 24-decision current-main
sequence (96 full inputs), followed by original-kernel output generation on
Windows x86-64 and Linux x86-64 under the same toolchain and debug flags. Compare
both complete output files before marking `oracleStatus` complete. Any math
difference must be reported by case/parameter set with raw bits; tolerances are
not permitted. macOS CI is a further required check.

## Reproduction and verification

[oracle.rs](oracle.rs) is a bounded standalone exporter, not a training harness.
Build it in a separate Cargo project depending on the untouched be3a035 crate,
with `serde_json`'s `float_roundtrip` feature. It takes three positional arguments:
the bit-encoded selected fixture, `src/bot/data`, and a fresh output filename.
It calls the original `ForagerBot::timing` and `Bot::decide`; intent extraction is
copied verbatim from the original adapter. It never calls the port. Compare
outputs on both platforms and retain the matching expectations in the fixture.

Local Rust execution, Windows/Linux conformance counts, package and WASM checks
are **pending**, not passed. No cross-platform math identity has been established.
The shared check runner now includes bot-only Clippy/tests, networking-only tests
and bot-only WASM checking alongside the existing Rust/package groups. CI results
and toolchain/build identifiers will be recorded after execution.

Host/native background wiring, examples, JSON bot stepping, headless forage,
public-mode guards, outside-server captures, cost/strength gates and A4 adoption
remain outside this PR. Raw-wire caller equivalence belongs to the later runtime
and JSON integration changes.
