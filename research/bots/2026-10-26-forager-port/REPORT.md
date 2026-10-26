# Frozen Forager port — 2026-10-26

This change ports the received-observation Forager without changing runtime bot
selection or simulator mechanics. A1 is the sole public `PolicyId`. A4-mean is
retained as test-only resolved data; it is not adopted by this port.

This records the port's historical scope. The subsequent
[A4 adoption](../2026-10-26-a4-adoption/REPORT.md) makes A4-mean selectable and the
default without changing these parameters or conformance fixtures.

## Source and parameters

The oracle is the untouched kernel at
[`be3a035dfab0c5717bef90daf88c6d3ada0e0ffe`](https://github.com/NikitaMGrimm/slither-sim/tree/be3a035dfab0c5717bef90daf88c6d3ada0e0ffe).
The port starts from main `77f23d4b53eb7324f6f77f68a74674de2b2450e6`.
The [manifest](../../../src/bot/data/resolved.json) pins the original kernel and
adapter hashes, all 54 parameter fields, all 16 fan entries and fan length.
Floating parameters use raw binary64 hex bits; integer and boolean types remain
explicit. Runtime releases never resolve against evolving defaults.

Original override files are retained byte for byte:

- [A1](https://github.com/NikitaMGrimm/slither-sim/blob/f069d9a073d72e7913f6ba297d2d956b833a141d/src/bot/data/a1-overrides.json): SHA256
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

The [shared inputs](../../../tests/fixtures/forager-conformance.json) contain 140
full frames. Twelve records were selected at indices 0 and 17 from each of six
private local/synthetic corpus groups, without inspecting policy outputs. Source
record hashes identify private inputs, not downloadable public archives. A complete
32-decision synthetic receipt tape covers RTT, empty/short/64/over-limit command
histories, boost toggles, respawns/reconnections, missing observations, incompatible
contracts, stale/missing receipts, reordered rows, dense/long bodies, prey, racing,
rim and exit inputs. Another 96 frames record all four seats at every decision in
a 24-decision current-main sequence using predetermined controls for playing slots.
These are offline frame sequences, not outside-server captures or live life traces.

All floating frame values are `{ "f64": "<16 hex digits>" }`; array order and
signed zero survive decoding. Derived Timing/Intent values use raw hex bits,
including unknown-Intent NaNs. Actions record presence, heading bits and boost.
No public-mode guard is applied.

Compact projections of the independent untouched-kernel outputs are retained separately:

- [Windows x86_64 MSVC](../../../tests/fixtures/forager-oracle-windows-x86_64-msvc.json).
- [Linux x86_64 GNU](../../../tests/fixtures/forager-oracle-linux-x86_64-gnu.json).

Each file comes from its own raw oracle run, with all 140 cases under both resolved
parameter sets. Raw files remain locally retained; their SHA-256s are recorded as
`rawOutputSha256` in [verification.json](verification.json). The committed files
are exact projections: each case contains only `id`, `expected` and `inputSha256`.
Every complete `expected` JSON subtree is copied byte for byte from that platform's
raw output. Fields are dropped; expected values are never recomputed or derived
from the other platform.

`inputSha256` hashes the original bit-encoded frame as compact UTF-8 JSON with
recursively sorted object keys, preserved non-ASCII characters and no trailing
newline. Inputs exist only in the shared fixture. The Rust test canonicalizes and
hashes each shared frame before comparing any Timing/Intent or action bits. SHA-256
support is test-only; the `bot` feature has no dependency additions. The test
selects an oracle only for its exact OS/architecture/environment and requires
zero mismatches including heading bits. No tolerance is used.

On every target, a separate test compares two fresh instances and one instance
reused across interleaved slots, lives and connections, in forward and reverse
order without resetting. It verifies exact action/derived bits, input preservation,
action presence, boolean boost and finite headings. The warmed allocation regression
also exercises both immutable parameter sets.

**macOS conformance is unverified because no macOS oracle exists.** macOS and all
other targets without an exact target oracle run determinism and structural
invariants only. The test names and an explicit notice identify this limitation;
passing those tests is not oracle equality.

## Compact fixture measurements

The projection follow-up passed Windows and WSL Linux conformance again: 140 A1
and 140 A4-mean cases per platform, zero mismatches, with input-frame hashes checked
before expected outputs. The projection CLI reproduces both committed files
byte for byte, and all 140 `expected` subtrees per platform match their raw output
bytes. The missing-input failure mode remains a hard assertion in the test.

| File | Bytes |
| --- | ---: |
| `forager-conformance.json` | 8,352,478 |
| `forager-oracle-windows-x86_64-msvc.json` | 1,546,854 |
| `forager-oracle-linux-x86_64-gnu.json` | 1,546,851 |
| Total | 11,446,183 |

The three fixtures shrink from 28,118,423 to 11,446,183 bytes.
At follow-up source `98a394d`, `python tools/check.py rust package wasm` passed
all 18 commands. The verified compressed crate shrinks from
3,101,422 bytes (2.96 MiB) to 1,333,935 bytes (1.27 MiB).
`PolicyId` is now non-exhaustive; constructor/decision docs and feature-default
wording were corrected. No expected value or kernel arithmetic changed.

## Last-bit platform differences

Complete original-kernel comparison found these four heading differences. They
are last-bit differences between the platforms' math libraries; no individual
function has been isolated. Timing/Intent bits, presence and boost all matched.
Both versions of every differing case remain in the projected oracle files and
[verification data](verification.json).

| Policy | Case | Windows heading bits | Linux heading bits |
| --- | --- | --- | --- |
| A1 | `current-8-1` | `3feac74d32095f93` | `3feac74d32095f92` |
| A4-mean | `selected-5-17` | `bfdff45ed8fb1661` | `bfdff45ed8fb1660` |
| A4-mean | `current-17-0` | `bfec3a5c096a3b19` | `bfec3a5c096a3b18` |
| A4-mean | `current-22-0` | `bfe5f095cfc48e54` | `bfe5f095cfc48e53` |

## Reproduction and verification

[oracle.rs](oracle.rs) is a bounded standalone exporter. Build it in a separate
Cargo project depending on untouched be3a035, with `serde_json`'s `float_roundtrip`
feature. Its three arguments are shared bit-encoded inputs, `src/bot/data`, and a
fresh output filename. It calls original `ForagerBot::timing`/`Bot::decide` and
verbatim original intent extraction. It never calls the port. Each target must
produce its own expectations; do not copy or normalize another target's headings.
Then run [project_oracle.py](project_oracle.py) with the raw output and a different
output path:

```sh
python research/bots/2026-10-26-forager-port/project_oracle.py raw.json projected.json
```

This standard-library projection copies raw `expected` subtrees verbatim without
reserializing or recomputing their values, adds hashes of their original input frames and
retains the platform/protocol/kernel metadata. Keep the raw file and record both
raw and projected file hashes. Rerunning the projection reproduces the committed
format without executing either policy implementation.

Both oracle runs used Rust 1.99.0 (`b940084d7`, LLVM 23.1.1), dev profile, optimization
level 0, debug information disabled, incremental compilation disabled and empty
Rust flags. Windows used UCRT file version 10.0.26100.9444; WSL used glibc 2.39.
The dependency lock was shared between oracle builds. Exact hashes,
compiler targets, resource limits and four differences are in [verification.json](verification.json).
The original output arrays repeated identically on each platform after adding
explicit exporter completeness/platform metadata.

Windows and WSL Linux port conformance each passed all 140 A1 and 140 A4-mean cases
with zero mismatches. Fresh/shared determinism also passed on both, including after integrating main
through `b6bc684`. The Windows run of `python tools/check.py rust package wasm`
passed all 18 commands: formatting, every Clippy feature build, default/core-only/
bot-only/networking-only tests, doctests, C ABI tests, Python crate checking,
the basic example, package verification and core-only/bot-only WASM. Packaging
retained all three full-frame files and the legacy fixture before projection
(3.0 MiB compressed); updated fixture and package measurements are recorded below.
CI results on Linux, Windows and macOS are tracked in
[PR #124](https://github.com/NikitaMGrimm/slither-sim/pull/124); all checks must pass
before ready promotion.

Initial attempts are retained in the private execution record: the first build
found the missing `Action` import; the frame generator initially sent a control to
a non-playing slot; the first oracle attempts rejected an incomplete synthetic
HUD object. These were corrected before the complete oracle runs. The initial
cross-platform mismatch prompted a maintainer decision to preserve separate
exact oracle sets, not a tolerance or numerical kernel change.

Host/native background wiring, examples, JSON bot stepping, headless forage,
public-mode guards, outside-server captures, cost/strength gates and A4 adoption
remain outside this PR. Raw-wire caller equivalence belongs to later integration.
