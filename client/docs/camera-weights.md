# Camera smoothing weights

`CameraCorrections` uses 62 binary64 constants computed with Chromium/V8's
`Math.cos`, which is what the official browser client evaluates. Native libm
implementations can round differently. Freezing the browser values preserves
the Canvas Wasm reference and makes native use those same weights. Other
transcendental calculations retain their existing target-specific dispatch.

The authored generator evaluates this expression in a fresh background headless
Edge process, using the offline replay tool's CDP pipe and isolated profile:

```javascript
let weight = 0.5 * (1 - Math.cos(Math.PI * (61 - index) / 61));
weight += (0.5 * (1 - Math.cos(Math.PI * weight)) - weight) * 0.5;
```

Run from `client/`, with no server or external page:

```powershell
node tools/generate-camera-weights.mjs '<absolute Edge executable>'
```

The generator returns the browser/V8 versions and all 62 big-endian binary64
bit patterns. It never evaluates the formula with Node's `Math.cos`: that
implementation can differ from Edge. No official source or recording is needed.
The selected values were freshly generated with Edge 156.0.4314.50 / V8 15.6.12.7
and all 62 matched the retained independent A3 browser-weight proof.

The native MSVC formula differs at indices 16, 41, 45 and 55 by 1, 1, 2 and 16
ULP respectively. Unit tests pin every selected bit pattern and separately
allow that observed spread when checking the native formula. This allowance
documents formula rounding; replay acceptance still requires exact values.

For validation, build and run fresh [A2 browser replays](../tools/feel-replay/browser-peers.md)
on main and the staged candidate. Compare every event, command, sample and final
field bit-for-bit while retaining each run's distinct source/artifact provenance.
Then build the [A3 native peer](../tools/feel-replay/native-peer.md) against the
candidate A2 runs and require exact cross-target results, including camera ring
storage and offsets. Keep all private tapes and run artifacts outside Git.
These equal-schedule checks do not establish GPU, desktop pacing, physical-input
or perceptual parity.
