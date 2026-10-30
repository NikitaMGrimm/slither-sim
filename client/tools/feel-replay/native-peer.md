# Offline native CPU peer (A3)

The `feel-replay` Rust example drives production `LocalGame`, including Play,
Connect construction/open, unsplit packet ingress, CSS pointer/key input, frame
preparation, every transport-effect acknowledgment and every issued picture's
successful presentation. It creates no socket, window, winit loop or GPU device.
The #299 `feel-trace` recorder supplies the observations; staged pictures retain
their own copies through issuance and retry, before world commit.

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
input bytes and independently regenerated frozen schedule. Entropy is hashed
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

Each native run replays the same executable twice, preserving both observation
documents and final traces. `repeat-comparison.json` requires A1 exact parity and
identical document bytes. `comparison.json` uses unmodified A1 `cross-target`;
`summary.json` reports exact/numerical/failing channels and `differences.json`
retains all differences even when a provenance/completeness gate fails. An exact
channel is a local observation result, not a pass through a failed global gate.

The native runner adopts only the Wasm manifest's **predeclared** numerical policy;
it cannot add or relax one after observing differences. Default A2 runs declare
`null`, so every native/Wasm numeric difference fails. A numerical match requires
the same identified platform-math cause on both peers and the unchanged
[A1 caps and accumulation guard](README.md#match-criteria). Commands, discrete
state, correction rings, pixels and times remain exact. A math-related difference
in such a channel still fails. Gameplay fixes belong in separate PRs.

Focused checks:

```sh
node --test tools/feel-native-replay.test.mjs
cargo test --locked --features feel-trace --manifest-path apps/wgpu-slice/Cargo.toml --example feel-replay
```

The authored driver tests cover effect acknowledgments, safe command capture,
staged/committed identity, delayed presentation and retry without resampling.
These CPU checks do not measure rasterization, offscreen readback, GPU completion,
desktop pacing, OS input, input-to-photon latency or perceptual parity.
