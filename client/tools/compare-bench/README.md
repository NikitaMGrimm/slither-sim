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
  "rounds": 3,
  "telemetryControls": true
}
```

Use Node 24 and existing locked dependencies (`npm ci --ignore-scripts --no-audit
--no-fund`). Run from `client/`:

```
node tools/compare-bench/run.mjs inspect <external-config.json>
node tools/compare-bench/run.mjs prepare <external-config.json>
node tools/compare-bench/build-vlither.mjs <external-config.json>
node tools/compare-bench/run.mjs preflight <external-config.json>
node tools/compare-bench/run.mjs smoke <external-config.json>
node tools/compare-bench/run.mjs run <external-config.json>
```

Build the host separately: `cargo build --release --locked -p slither-sim
--features frame-bench --bin slither-sim`. Pass a release desktop binary by path;
its hash identifies it. Build the desktop input with `--features frame-bench`
for the planner food census; view telemetry/journal support is also required.
Results distinguish received/viewport counts from distinct submitted pellets and
record omission share per view. Missing submission evidence and material food
submission differences qualify comparisons; see [method](method.md#worlds-and-validity).
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
  The first real round also pairs telemetry on/off for Original, Native and NTL,
  including Original/Native at NTL's limited view. Matched native off controls
  require a binary with setup-only view telemetry and its stop acknowledgement;
  they measure scale before warmup, then disable all client telemetry.
  Each session first calibrates NTL zoom and verifies its closed netlog under the
  documented named no-packet reachability-probe allowance; all other non-loopback
  transport evidence stops before timed rounds. If NTL participates and its
  farthest view is limited, every Battledome row uses that physical scale,
  including Vlither and telemetry controls; separate whole-map rows are dropped.
  Without NTL, or if an updated package fits the map, whole-map behavior remains.
  Normal stays at stock zoom. The full first session has 30 rows; budget about
  50-65 minutes before retries (estimate, not a measured duration).

NTL row setup uses 80 wheel notches, with fresh scale/settling pictures before
warmup; calibration keeps 80 + 320 + 640. Vlither uses its stock wheel callback,
preferring whole notches and using fractional 1/120-notch offsets only when
needed for the 5% reference tolerance. Its physical framebuffer scale must match two
host-anchored pictures; timed acceptance keeps the same 5% reference tolerance.
No zoom range or renderer is changed. A wrapper change requires a fresh external
Vlither build/receipt before acquisition; an old binary fails provenance checks.
Matching the view does not establish content/display correlation, exact NTL scene
counts or identical presentation modes. Those qualifications still apply.

`preflight` exercises calibration, matched Original/Native views, native setup
telemetry stop, the NTL row scale, Vlither's physical picture capture and the
Normal setup for every configured client. The full preflight has nine rows when
NTL is limited, including the native setup-stop control. It stops before PresentMon or any measurement window;
each row remains invalid as comparison evidence. Input, occlusion or setup
failure stops the sequence and retains the receipts. Both calibration and row
probes reject competing builds/tests; samples retain blocking process names/PIDs.

Battledome uses 100 internal A4 bots and whole-map server interest. Normal uses
500 internal bots; stress triples ambient attempts and initial/refill target.
After population scaling: target **163500**, **2406.845341348622 attempts/s**.
Actual physical, received, visible and image-candidate pellet counts remain
separate. A nondefault loopback-only host feature supplies identical orbit
steering and socket-player collision protection; bots still die on its body.
When detail pressure applies, results retain the full pre-budget candidate count,
the selected count and the declared food limit. Total host bot population alone
does not establish detail pressure: server interest can deliver fewer snakes.

Replace `mod` to use updated NTL. Manifest/package/tree hashes are recorded;
changed visible UI contracts fail preflight in one small adapter. No minified game
globals are read. The local shell needs no official page/assets. Timed screenshots
provide bounded presence/extent evidence, not exact counts; their acquisition
cost is included in the telemetry on/off control. Image analysis runs afterwards.
Set `vlitherRef` to pin upstream; omission fetches the default tip and records its
commit. The user-space build uses a labelled authored launch/telemetry wrapper
without changing stock render or zoom limits.

See [method.md](method.md) for tool paths, PresentMon rights, isolation, adapter
controls, coverage limits and telemetry on/off perturbation controls. Unknown
overhead or content equivalence blocks a verdict. The harness never elevates,
changes group membership, kills competing work or fights for foreground.
Use `preflight` to check NTL without starting timed rounds. It discloses the exact
closed, attributed, zero-I/O UDP reachability probes; runs are not described as
having no external socket attempts. A display-only failure returns exit 2.

Offline checks: `node --test tools/compare-bench.test.mjs`. The authored 60 Hz
canvas/fast compositor control has a separate manual `synthetic-check.mjs` runner;
only its offline parsing/probe regression runs in CI.
