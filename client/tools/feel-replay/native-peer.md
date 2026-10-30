# Offline native CPU peer (A3)

The `feel-replay` Rust example drives production `LocalGame`, including Play,
Connect construction/open, unsplit packet ingress, CSS pointer/key input, frame
preparation, every transport-effect acknowledgment and every issued picture's
successful presentation. It creates no socket, window, winit loop or GPU device.
The #299 `feel-trace` recorder supplies the observations; staged pictures retain
their own copies through issuance and retry, before world commit. Recorder-local
identities bind successful picture acknowledgements to those observations,
including equal-valued preparations. Recoverable draw-memory skips remain in the
trace with `prepared: true, submitted: false, committed: false`; they increment
neither successful-draw counter. The A1 v2 shape and flat observation ABI stay
unchanged.

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
It follows the post-#304 browser families: only primary `mousedown` presses,
both `pointermove` and `mousemove` movements, and paired release de-duplication.
An isolated pointer move retains its fractional CSS coordinates; a compatibility
mousemove overwrites them with the mouse getter's integer position. Pointerdown
is not canceled, so it does not suppress compatibility mouse events.

This A2 lane constructs DOM events from normalized **CSS** options. Its synthetic
MouseEvent getters floor those double options, including explicit `dom-event`
records; the native mapper observes and submits that same position. It reuses
`normalizeInput`, `mouseReplay` and the production admission predicate rather
than applying the physical-coordinate conversion to CSS again. Production winit
still uses shared `desktop_input::mouse_client_position` for physical/f32/DPR
admission. These are distinct input boundaries, not an extra f32 rounding step
on already initialized CSS DOM events.
Semantic socket IDs match A2; the `effects` array in `native-observations.json`
retains every real Rust effect's generation/ID and acknowledgment time. Negotiation/login bytes are discarded.

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
retains all differences even when the comparison's provenance/completeness gate
fails. Earlier build, input, Wasm-trace, schedule or startup binding failures stop
before comparison and write only `failure.txt`. An exact
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
