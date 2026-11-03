# Local visible-frame pacing benchmark

Opt-in Windows acquisition for a supplied release slither-client binary, an
updated owner-supplied NTL mod and a GitHub Vlither ref. All game traffic stays
loopback; third-party files, raw evidence and failed attempts stay outside Git.
Acquisition never runs in CI.

PresentMon 2.x measures displayed-present intervals: median/p95/p99, longest
stall, counts over 50/100/250 ms and achieved FPS, with actual refresh, mode and
SyncInterval. **A compositor present is not necessarily a new game frame.** NTL
also reports canvas draw production grouped by rAF timestamp as a separate primary
content measure. Native journals expose duplicate prepared-picture presents.
Exact content/display correlation remains unestablished, so cross-client results
remain labelled comparisons. This does not measure physical scanout or input
latency, and browser rAF never substitutes for ETW.

Put this config and all inputs/results outside Git:

```json
{
  "output": "<external scratch directory>",
  "host": "<release slither-sim.exe built with frame-bench>",
  "binary": "<release slither-wgpu-desktop.exe>",
  "mod": "<NTL manifest folder, ZIP or CRX>",
  "chrome": "<extension-capable Chrome for Testing or Chromium executable>",
  "presentMon": "<PresentMon 2.6 console executable>",
  "vlitherBinary": "<output>/vlither/build/bin/windows_x86_64_release/app.exe",
  "experiment": "primary",
  "targetAdapterLuid": "<RTX LUID from inspect>",
  "targetDisplay": "<4K display device name from inspect>",
  "targetRefreshHz": 240,
  "width": 3840,
  "height": 2160,
  "x": 0,
  "y": 0,
  "seconds": 60,
  "rounds": 3
}
```

Use Node 24 and existing locked dependencies (`npm ci --ignore-scripts --no-audit
--no-fund`). Run from `client/`:

```
node tools/compare-bench/run.mjs inspect <external-config.json>
node tools/compare-bench/run.mjs prepare <external-config.json>
node tools/compare-bench/build-vlither.mjs <external-config.json>
node tools/compare-bench/run.mjs smoke <external-config.json>
node tools/compare-bench/run.mjs run <external-config.json>
```

Build the host separately: `cargo build --release --locked -p slither-sim
--features frame-bench --bin slither-sim`. Pass a release desktop binary by path;
its hash identifies it. View telemetry/journal support is required for acceptance.
Apply local Cargo target/jobs limits per command. No build/test runs during timing.

* Primary: same RTX adapter, 3840x2160 physical client area on the selected
  4K/240 Hz display. Chrome uses browser fullscreen; native/Vlither use borderless
  windows. Chrome gets an ANGLE LUID flag in a throwaway profile. Actual
  per-process GPU engine counters must resolve to that LUID. Native/Vlither have
  no adapter selector; no Windows graphics preference or client default changes.
* Secondary: Chrome unforced (`clients:["ntl"]`, `experiment:"secondary"`), same
  size/display. Optional 1280x720 uses `pixels-optional`; future iGPU comparisons
  are documented separately. Actual default selection is recorded, never assumed.
* Real runs reverse order over at least three rounds; bounded retries preserve
  failures. Full High-effects Original/Native are separate variants. Smoke uses
  primary with five-second windows. Foreground/input/occlusion, host tick health,
  stream coverage, scene evidence, GPU and refresh changes are validity gates.

Battledome uses 100 internal A4 bots and whole-map server interest. Normal uses
500 internal bots; stress triples ambient attempts and initial/refill target.
After population scaling: target **163500**, **2406.845341348622 attempts/s**.
Actual physical, received, visible and image-candidate pellet counts remain
separate. A nondefault loopback-only host feature supplies identical orbit
steering and socket-player collision protection; bots still die on its body.

Replace `mod` to use updated NTL. Manifest/package/tree hashes are recorded;
changed visible UI contracts fail preflight in one small adapter. No minified game
globals are read. The local shell needs no official page/assets. Screenshots
provide presence/extent candidates, not exact counts or continuous camera proof.
Set `vlitherRef` to pin upstream; omission fetches the default tip and records its
commit. The user-space build uses a labelled authored launch/telemetry wrapper
without changing stock render or zoom limits.

See [method.md](method.md) for tool paths, PresentMon rights, isolation, adapter
controls, coverage limits and telemetry on/off perturbation controls. Unknown
overhead or content equivalence blocks a verdict. The harness never elevates,
changes group membership, kills competing work or fights for foreground.

Offline checks: `node --test tools/compare-bench.test.mjs`. The authored 60 Hz
canvas/fast compositor control has a separate manual `synthetic-check.mjs` runner;
only its offline parsing/probe regression runs in CI.
