# Offline native CPU peer (A3)

The `feel-replay` Rust example drives production `LocalGame`, including Play,
Connect construction/open, unsplit packet ingress, CSS pointer/key input, frame
preparation, every transport-effect acknowledgment and every issued picture's
successful presentation. It creates no socket, window, winit loop or GPU device.
The #299 `feel-trace` recorder supplies the observations; staged pictures retain
their own copies through issuance and retry, before world commit.

This driver reproduces **A2's equal-schedule lane**: `Driver::packet` and
`Driver::close` eagerly drain and acknowledge prepared pictures before the next
scheduled event. It does not reproduce the desktop host's queued-picture guard
or one-picture-per-redraw order. In [`src/main.rs`](../../apps/wgpu-slice/src/main.rs),
`Running::frame` skips `begin_frame` while a picture is queued and presents at
most one picture per redraw, called from `ApplicationHandler::window_event` on
`RedrawRequested`. Two Setup pictures and subsequent ingress can therefore
interleave differently on desktop. Native-host ordering/backpressure belongs
to plan items C/D; A3's equal-schedule results do not establish that parity.

Run from `client/`, with Rust 1.99 and Node 24. Stage the candidate first. Builds
are explicit and sequential; replay requires the prebuilt executable and a Canvas
Wasm run of that same candidate/input/schedule:

```powershell
$env:CARGO_BUILD_JOBS='2'
$env:WASM_BINDGEN='<absolute wasm-bindgen 0.2.129 executable>'
node tools/build-feel-native.mjs
node tools/build-feel-replay.mjs
node tools/feel-browser-replay.mjs --record tools/feel-replay/browser-synthetic.json --out <fresh-wasm-directory> --browser '<headless Edge executable>' --commands 7 --appearance-at 3
node tools/feel-native-replay.mjs --record tools/feel-replay/browser-synthetic.json --wasm-run <fresh-wasm-directory> --out <fresh-native-directory> --commands 7 --appearance-at 3
```

The existing [odd-startup fixture](browser-startup-synthetic.json) uses `--commands
1 --appearance-at 3`, with `--width 961 --height 541` on the browser command. The
native runner takes geometry from that bound Wasm run and independently verifies
the shared backing policy. Use an explicit private tape path for private replay;
never put its bytes, hashes or paths in Git/public reports. Existing runs and
historical live-feel records stay immutable. A changed candidate requires a fresh
build receipt and new runs; no trace's bindings are rewritten to make it compare.

The build receipt hashes the executable and staged source tree/core. The runner
checks these against current sources and the A2 manifest, and checks the original
input bytes and independently regenerated frozen schedule. The Wasm trace must
match the `traceSha256` that its A2 run wrote into `summary.json` after that run's
own rechecks; A2 runs without it must be regenerated. Entropy is hashed
from actual native request-tagged observations, independently of Wasm. Native
input mapping reuses A2's normalized CSS coordinates/physical-mouse expansion and
the production browser target-admission predicate. It calls the real LocalGame
methods and does not establish OS/DOM event routing or physical-input parity.
Semantic socket IDs match A2; sidecar receipts retain every real Rust effect's
generation/ID and acknowledgment time. Negotiation/login bytes are discarded.

Rust writes `feel-f64-v1` directly from raw binary64 values (including signed
zero), with A1's sorted keys and formatting. Node hashes the entropy/artifacts
and supplies the manifest; Rust's `--assemble` completes the v2 trace using the
same bit-preserving codec. No decimal JSON serialization precedes bit capture.
`--codec` is a standalone interoperability check for an A1 document on stdin.

Its codec domain is narrower than general JS A1: object keys must be ASCII and
must not be canonical array indices (`0` through `4294967294`, in canonical
decimal form). Reads and writes reject these unsupported keys recursively,
including BMP/non-BMP keys whose Rust and JS UTF-16 sort orders can disagree.
String values may contain any Unicode scalar values. Lone surrogates are
rejected on read and are unrepresentable in Rust strings on write. Non-index
keys such as `"01"` and `"4294967295"` are admitted. No key/string normalization
or silently reordered output extends this domain.

Each native run replays the same executable twice, preserving both observation
documents and final traces. `repeat-comparison.json` requires A1 exact parity and
identical document bytes. `comparison.json` uses A1 `cross-target`, while
`exact-comparison.json` disables numerical allowances on those same traces;
`summary.json` reports exact/numerical/failing channels and `differences.json`
retains all differences even when a provenance/completeness gate fails. An exact
channel is a local observation result, not a pass through a failed global gate.

The native runner adopts only the Wasm manifest's **predeclared** numerical policy;
it cannot add or relax one after observing differences. Default A2 runs declare
`null`, so every native/Wasm numeric difference fails. A numerical match requires
the same identified platform-math cause on both peers and the unchanged
[A1 caps and accumulation guard](README.md#match-criteria). Cross-target ring
floats use the existing f32/world caps and a separate accumulation history for
each stored element. Commands, discrete state (including ring indices/pending
counts), pixels and times stay exact; same-core remains fully exact.

To predeclare a policy, pass `--numerical-policy <feel-f64-v1-policy-file>` to the
**browser** runner before execution. It validates the unchanged caps and freezes
the declaration as `numerical-policy.json` in the fresh Wasm run directory before
either peer starts. Its manifest supplies the same policy to the native runner.
Use cause `host-transcendental` with independent function/runtime evidence;
never rewrite an existing trace's declaration after observing its errors. The
comparison report records policy version `feel-cross-target-v2`; trace/manifest
schema versions are unchanged. Gameplay fixes belong in separate PRs.

Focused checks:

```sh
node --test tools/feel-native-replay.test.mjs
cargo test --locked --features feel-trace --manifest-path apps/wgpu-slice/Cargo.toml --example feel-replay
```

The authored driver tests cover effect acknowledgments, safe command capture,
staged/committed identity, delayed presentation and retry without resampling.
These CPU checks do not measure rasterization, offscreen readback, GPU completion,
desktop pacing, OS input, input-to-photon latency or perceptual parity.
