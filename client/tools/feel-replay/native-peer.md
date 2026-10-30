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

The default driver reproduces **A2's equal-schedule lane**: `Driver::packet` and
`Driver::close` eagerly drain and acknowledge prepared pictures before the next
scheduled event. It does not reproduce the desktop host's queued-picture guard
or one-picture-per-redraw order. The separate **desktop-ordering lane** below
uses [`src/main.rs`](../../apps/wgpu-slice/src/main.rs)'s redraw lifecycle.
Equal-schedule results alone do not establish desktop ordering parity.

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

## Desktop ordering and presentation backpressure

Build both peers as above, then freeze a Canvas run at the intended cadence.
`--hz` defaults to 60 on the browser runner; use 144 and 240 as separate runs.
Give every native delay a fresh output directory:

```powershell
node tools/feel-browser-replay.mjs --record tools/feel-replay/browser-synthetic.json --out <fresh-canvas-directory> --browser '<headless Edge executable>' --commands 7 --appearance-at 3 --hz 144
node tools/feel-desktop-replay.mjs --record tools/feel-replay/browser-synthetic.json --wasm-run <fresh-canvas-directory> --out <fresh-desktop-directory> --commands 7 --appearance-at 3 --hz 144 --ack-ms 0
```

Repeat with `--ack-ms frame` (one display interval) and `--ack-ms 40` (a deliberate
40 ms block at every admitted presentation). These are authored synchronous host
blocking windows, not measured FIFO, GPU completion or scanout. One-frame delays
end at the next authored display boundary, avoiding addition/multiplication
rounding that could invent missed redraws. `presentation-plan.json` is frozen
before invoking Rust. No real timer drives the game. During a block, redraws
coalesce and input/transport waits; acknowledgment precedes service at return.
Restart packets also wait for the authored reset-presentation/Connect barrier.
Original scheduled packet times remain separate from service times.

The driver uses production `service_ingress`, `begin_redraw`, effect draining,
`take_frame` and `presented`, issuing at most one picture per redraw. Superseded
Setup pictures are not counted as successful presents. Their already-recorded
core draw/camera samples remain. Work awaiting acknowledgment, pending I/O and
host-owned pictures still fence ingress/preparation; retry does not resample.
Draw IDs enumerate preparations; `quality.successfulDraws` and final `draws`
count only real successful acknowledgments, using the shared sequence/receipt
model. As in Canvas assembly, sample counts are cumulative committed draws in
preparation order after receipt matching. The lifecycle sidecar records a delayed
desktop acknowledgment at its actual event, without another preparation sample.
Memory-skipped Setup pictures neither fence ingress nor receive quality credit;
completed Setup pictures count once toward quality whether presented or superseded.

Each run executes the unchanged equal-schedule lane and a native equal-schedule
control with the desktop lane's admitted service/sample times. The latter
separates expected cadence/blocking effects from lifecycle artifacts; it is a CPU
control, not another Canvas Wasm execution. `paced-core-differences.json` compares
every sample timestamp, delta, lag, own/camera cell and command byte/time, excluding
event IDs and presentation accounting. Full differences against the original
Canvas and native equal lane are retained and classified without tolerance.

The runner authenticates the staged candidate, executable, input, schedule and
completed A2 trace. Its exit code checks shared cumulative draw accounting,
recorder errors, packet/acknowledgment counts, pending work and the paced core
control. Repeat byte equality remains diagnostic. The strict
original A1 gate remains in `a1-comparison.json`: changed service times, skipped
redraws, tail acknowledgments or superseded Setup submissions can fail it.
Diagnostic success does not promote it to a pass. Draw status is recorded at the
preparation event; the lifecycle sidecar records later issuance/acknowledgment.

### Findings

At base `5f41ef7e`, two queued Setup snapshots suppressed two fresh updates per
life and deferred subsequent own appearance. The authored two-life
motion/boost/death/restart case lost four update/draw pairs at zero ack delay:

| Cadence | Added first-own ingress delay | Missing update/draw pairs |
| --- | ---: | ---: |
| 60 Hz | 47 ms | 4 |
| 144 Hz | 17.833333333333336 ms | 4 |
| 240 Hz | 9.5 ms | 4 |

`service_ingress` now supersedes obsolete Setup snapshots before applying ready
deferred packets. `begin_redraw` supersedes remaining Setup snapshots before a
fresh frame. The desktop host uses these at its existing service/redraw
boundaries. Shared camera and frame timing are unchanged. Scripted acknowledgment
regressions retain every core Setup sample and compare update/camera histories,
commands and packet state with a synchronous control, including two Setup
commands followed by appearance before redraw.

Superseded Setup draws still count exactly once toward quality's raw draw
statistics, as on Canvas. Dropping that count at the <=24 threshold can trigger
an earlier low-quality transition and change the subsequent face-heading target.
The long-stall regression checks that threshold without another draw or sample.

Retried Setup pictures may also be superseded after ownership returns to
LocalGame: the queue drops their owned pixel copies, retains one quality credit,
and leaves no acknowledgment or ingress fence. Redraw and ingress regressions
check both sides of the quality threshold against an eager control.

A private tape's frozen 60 Hz command expectation can fail at higher cadence
even when both browser peers agree. Preserve those failures instead of learning
an expectation from a run. These authored CPU histories establish neither
desktop pacing nor perceptual parity.
