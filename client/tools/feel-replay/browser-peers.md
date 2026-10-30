# Offline browser peers (A2)

This is a measuring instrument, with no feel-policy correction. Run from
`client/`, using Rust 1.99, Node 24, wasm-bindgen 0.2.129 and installed Edge or
Chromium. Builds are explicit; replay requires the prebuilt artifacts.

```powershell
$env:CARGO_BUILD_JOBS='2'
$env:WASM_BINDGEN='<absolute wasm-bindgen 0.2.129 executable>'
node tools/build-feel-replay.mjs
node tools/feel-browser-replay.mjs --record tools/feel-replay/browser-synthetic.json --out <fresh-directory-outside-repository> --browser '<headless Edge or Chromium executable>' --commands 7 --appearance-at 3
```

Stage intended source changes before the explicit build. Its receipt binds the
artifacts to the staged tree/core and the hashes of every completed binding,
Wasm binary and generated snippet. The previous receipt is invalidated before
building; interrupted rebuilds leave no authenticating receipt. Replay checks the
hashes before launching either peer and rejects stale/replaced artifacts or
unstaged source. Cleanup tracks process termination from spawn, including signals.

`--commands` is the frozen expected control count, and `--appearance-at` the
independently declared first own-appearance time. Neither is learned from a peer
trace. Optional `--width`, `--height`, `--dpr` default to 960, 540, 1. Private
schema-1 header-free tapes use the same command with their existing immutable
record path. `--receipt` hashes an existing acceptance receipt in place. Private
paths, hashes, bodies, receipts and results must remain outside Git/public logs.

The authored synthetic v3 fixture includes two challenge negotiations, unsplit
Setup pairs, own appearances, pointer/compatibility movement, boost, editor keys,
pong and death through expiry followed by a restart. `npm test` covers pure schedule,
coordination, mapping, assembly and deliberate faults. Browser runs are manual
local checks; they are not required by browserless CI. Additional focused Rust
checks are:

```sh
cargo test --locked --features feel-trace --test feel_trace
cargo test --locked --features feel-trace --manifest-path apps/wgpu-slice/Cargo.toml --test feel_trace
```

The runner uses a dedicated background headless process and CDP pipe, with no npm
dependency added and no debugging listener. A static server binds a free literal
127.0.0.1 port, excluding the reserved ports. CSP, JS capability fences and CDP
request interception deny external requests, WebSockets, workers and beacons.
The in-memory socket is the only transport. Each full original envelope goes to
both hosts; inspecting opcode metadata to freeze Setup boundaries does not split
delivery. The server and browser close even after failure. Failed run directories
are preserved; existing outputs are never overwritten.

Canvas uses `createLocalGameHost` and `game-runtime` with the production owned UI,
sources, real Wasm objects/prototypes and existing socket/clock/Wasm-factory
capabilities. WebGPU uses `LocalGameplay`, `localHost` and `installLocalInput`.
Only its GPU I/O is replaced by a feature-gated CPU presentation sink; all resource
preparation, gameplay, effects and presentation acknowledgments remain production
paths. This gate does not compare pixels, GPU completion, physical input or pacing.

The virtual Date/performance origin, timer/RAF history, high quality, viewport,
resting mouse pointer and empty keys are equal before bootstrap. An optional
`startupPointer` records normalized `x/y` (or explicit `coordinateSpace: "css"`).
It restores the legacy mouse's integer CSS position. Otherwise the fallback is
the integer CSS viewport center, including at odd dimensions. Startup uses those
CSS coordinates directly through DOM events, avoiding a divide/multiply round
trip. The manifest records the source and actual coordinate bits. Both peers receive
one scheduled reset/Connect RAF and then the ordinary frame at time zero; there
is no one-sided warmup. Packets at time zero are rejected because they would
precede the initial Connect acknowledgment. Packet/input/frame ties retain A1
ordering, and an explicit final frame covers the complete declared duration.
Each event drains synchronous effects and queued ingress before acknowledgment.
The perpetual next RAF is reported separately from unfinished model work.

Authored lifecycle `controls` are separate from DOM `inputs`: `close` emits a
transport close through the real socket listeners, and `play` invokes the same
production Play API used at startup. They do not inject DesktopInput or masquerade
as physical input. A frozen `betweenLives` interval names that close, following
Play, and next appearance. Its missing-own samples are legal only after observed
expiry; resumed own must have a new token. Unscheduled absence still fails.

The shared production draw preparation removes expired snakes from the draw
collection before either renderer plans a face. The recorder follows that skip:
retained own data includes raw `deathProgress`/`expired`, and `projected` is null
only for expired own. It does not clamp expired progress or invent an invisible
face. [Contract v2](README.md) makes these states explicit; old artifacts stay
immutable and require their pinned v1 reader.

Normalized coordinates become real DOM events at explicit per-peer selectors.
The named manifest normalization `chromium-primary-mouse-compat-v1` treats legacy
`mousemove`/primary-button records as physical mouse inputs: dispatch the mouse
PointerEvent first, then the compatibility MouseEvent on the same target. This
expansion is identical for both peers, without changing the tape or inserting a
frame between events. An authored input with `encoding: "dom-event"` is already
an explicit DOM event and is not expanded again. The A1 source event remains one
physical input; receipts retain both actual dispatches and any suppressed event.

[Pointer Events 3 §11.2](https://www.w3.org/TR/pointerevents3/#mapping-for-devices-that-support-hover)
defines this mapping for hovering pointers. A canceled primary `pointerdown`
suppresses compatibility `mousedown`, subsequent `mousemove` and `mouseup` until
release; canceling a hover `pointermove` alone does not suppress its mouse event.
Chromium implements the order and suppression in
[SendMousePointerEvent](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/core/input/pointer_event_manager.cc),
including clearing suppression on a no-buttons hover move. Its
[MouseEvent coordinate getters](https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/core/events/mouse_event.h)
floor coordinates, while [PointerEvent getters](https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/core/events/pointer_event.h)
retain fractional move coordinates. For the admitted nonnegative viewport
coordinates the mapper's integer conversion has that same result. It does not
force either host to admit the same event family or coordinates. Residual
differences remain failures. This DOM-listener lane does not claim browser click,
boundary/focus-default or physical-input acceptance.

Production listeners record received, routed, gameplay and filtered events,
including compatibility suppression, in receipt sidecars. Trace targets are
shared semantic IDs; sidecars retain actual DOM target IDs. The authored
`browser-startup-synthetic.json` has no recorded inputs, so an odd-viewport full
path checks the first gaze/command without subsequent input masking startup:

```sh
node tools/feel-browser-replay.mjs --record tools/feel-replay/browser-startup-synthetic.json --out <fresh-directory-outside-repository> --browser '<headless Edge or Chromium executable>' --commands 1 --appearance-at 3 --width 961 --height 541
```

## Mouse coordinate admission (B2)

WebGPU admits both `pointermove` and `mousemove`, matching Canvas. A compatibility
`mousemove` overwrites fractional pointer coordinates with the integer mouse
position when present; isolated pointer movement remains admitted. Primary press
uses `mousedown`, matching the private official mouse listeners. Press updates
the position and boost; release only clears boost. Pointer releases/cancel still reach the
owner over editors and controls, and the matching compatibility mouseup is
de-duplicated. Pointer movement/press is not canceled by this adapter, so normal
mouse compatibility dispatch can reach the mouse listeners.

Native winit positions are physical pixels. `desktop_input::mouse_client_position`
casts the physical position and device scale to f32, computes the f32 reciprocal
of that scale, multiplies each f32 coordinate by it, promotes the f32 result to
f64, then floors each axis before
the existing projection. Negative/outside-window positions are not clamped. This
admission rule follows Blink at revision `2d56607133c816d230f75a4e0bb797bdd69c8c8c`:
[layout zoom includes device scale](https://github.com/chromium/chromium/blob/2d56607133c816d230f75a4e0bb797bdd69c8c8c/third_party/blink/renderer/core/frame/web_frame_widget_impl.cc),
[mouse initialization removes layout zoom](https://github.com/chromium/chromium/blob/2d56607133c816d230f75a4e0bb797bdd69c8c8c/third_party/blink/renderer/core/events/mouse_event.cc),
with [PointF scaling in float precision](https://github.com/chromium/chromium/blob/2d56607133c816d230f75a4e0bb797bdd69c8c8c/ui/gfx/geometry/point_f.h#L77-L82),
then [MouseEvent client coordinate getters floor](https://github.com/chromium/chromium/blob/2d56607133c816d230f75a4e0bb797bdd69c8c8c/third_party/blink/renderer/core/events/mouse_event.h#L139-L140).
The f32 operation order matters: physical -21 at DPR 1.75 scales to
-12.000000953674316, so the getter admits -13 rather than -12. Native regressions
cover a 45-position sweep over DPR 1, 1.25, 1.5, 1.75 and 2, coordinate/scale
narrowing and negative/edge positions. Browser hosts consume DOM getters directly.

The authored `browser-fractional-synthetic.json` preserves fractional coordinates
through dispatch, exercising explicit pointer/mouse pairs and the physical-mouse
compatibility mapping. Its independently frozen expectation is four commands;
it is a DOM-listener regression, not a physical-input or perceptual claim:

```sh
node tools/feel-browser-replay.mjs --record tools/feel-replay/browser-fractional-synthetic.json --out <fresh-directory-outside-repository> --browser '<headless Edge or Chromium executable>' --commands 4 --appearance-at 3 --width 961 --height 541
```

The authored `browser-pointer-only-synthetic.json` removes the first paired
mousemove, leaving fractional pointer movement isolated until the next input.
It freezes the same four-command expectation, with 30 events and 45 boundaries:

```sh
node tools/feel-browser-replay.mjs --record tools/feel-replay/browser-pointer-only-synthetic.json --out <fresh-directory-outside-repository> --browser '<headless Edge or Chromium executable>' --commands 4 --appearance-at 3 --width 961 --height 541
```

`feel-trace` is absent from ordinary builds, including its fields, hooks and CPU
sink. The reusable Rust `Recorder` copies raw state at the existing core update
and draw boundaries, including packet application and every Setup draw. Packet
`update` rows retain the last animation delta and do not advance animation. It never advances a clock or
prepares another draw. LocalGame's picture envelope carries a copied observation
through staging/queueing/retry; `issued_feel_observation()` is available before
world commit for A3. Native A3 can attach the same recorder and consume the same
flat numeric ABI (`observation.mjs`). Unsupported projected face layouts fail
closed: the recorder emits an explicit error and no partial numeric row. The
trace is marked incomplete, and receipts/comparison errors retain the reason.
The current full-path fixtures use paired eyes.

The browser applies the A1 writer (`io.mjs` re-exports its browser-safe codec) to
raw native Numbers before CDP transport. No plain JSON round trip loses signed
zero. `schedule.json`, peer manifests/traces and `comparison.json` use A1 encoding.
All tracked client sources and generated bindings are hashed. The manifest records
HEAD plus the staged candidate tree/core; stage intended changes before running
and retain that distinction when the candidate is not yet committed.

`compareTraces` runs unchanged in same-core mode. Completeness and entropy can
fail before numeric comparison. `differences.json` additionally retains every
exact state/event/command/binding difference, without tolerances or a pass
decision. This diagnostic report never overrides the A1 result. Entropy receipts
observe the page RNG and the production WebGPU factory RNG independently; the
same numeric seed is not an equal-entropy claim. Stream mismatch remains a
failure for later E work. Host input differences belong to B and presentation
lifecycle changes to D. Known failures are evidence, not an invitation to alter
the measuring gate.
