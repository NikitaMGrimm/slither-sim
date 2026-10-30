# Offline browser peers (A2)

This is a measuring instrument, with no feel-policy correction. Run from
`client/`, using Rust 1.99, Node 24, wasm-bindgen 0.2.129 and installed Edge or
Chromium. Builds are explicit; replay requires the prebuilt artifacts.

```powershell
$env:CARGO_BUILD_JOBS='2'
$env:WASM_BINDGEN='<absolute wasm-bindgen 0.2.129 executable>'
node tools/build-feel-replay.mjs
node tools/feel-browser-replay.mjs --record tools/feel-replay/browser-synthetic.json --out <fresh-directory-outside-repository> --browser '<headless Edge or Chromium executable>' --commands 4 --appearance-at 3
```

Stage intended source changes before the explicit build. Its receipt binds the
artifacts to the staged tree/core; replay rejects stale builds or unstaged source.

`--commands` is the frozen expected control count, and `--appearance-at` the
independently declared first own-appearance time. Neither is learned from a peer
trace. Optional `--width`, `--height`, `--dpr` default to 960, 540, 1. Private
schema-1 header-free tapes use the same command with their existing immutable
record path. `--receipt` hashes an existing acceptance receipt in place. Private
paths, hashes, bodies, receipts and results must remain outside Git/public logs.

The authored synthetic fixture includes challenge negotiation, an unsplit envelope
with two Setup commands, an own appearance, pointer and compatibility mouse
movement, boost, editor-filtered keys and pong. `npm test` covers pure schedule,
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
center pointer and empty keys are equal before bootstrap. Both peers receive
one scheduled reset/Connect RAF and then the ordinary frame at time zero; there
is no one-sided warmup. Packets at time zero are rejected because they would
precede the initial Connect acknowledgment. Packet/input/frame ties retain A1
ordering, and an explicit final frame covers the complete declared duration.
Each event drains synchronous effects and queued ingress before acknowledgment.
The perpetual next RAF is reported separately from unfinished model work.

Normalized coordinates become real DOM events at explicit per-peer selectors.
Historical mouse events remain mouse events; the runner does not manufacture a
pointer event to conceal a host filtering difference. Fixtures can explicitly
schedule both families. Production listeners record received, routed, gameplay
and filtered events, including compatibility suppression, in receipt sidecars.
Trace targets are shared semantic IDs; sidecars retain actual DOM target IDs.

`feel-trace` is absent from ordinary builds, including its fields, hooks and CPU
sink. The reusable Rust `Recorder` copies raw state at the existing core update
and draw boundaries, including packet application and every Setup draw. Packet
`update` rows retain the last animation delta and do not advance animation. It never advances a clock or
prepares another draw. LocalGame's picture envelope carries a copied observation
through staging/queueing/retry; `issued_feel_observation()` is available before
world commit for A3. Native A3 can attach the same recorder and consume the same
flat numeric ABI (`observation.mjs`). Unsupported projected face layouts fail
closed; the current authored fixture uses paired eyes.

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
