# Local visible-frame pacing benchmark

Opt-in Windows acquisition for a supplied release slither-client binary, an
owner-supplied NTL mod, and a GitHub Vlither ref. Nothing is downloaded from or
connected to a public game server. Acquisition is never run in CI.

The primary measure is the interval between **actually displayed frames** from
PresentMon 2.x ETW timestamps, identically for every client: median, p95, p99,
longest gap, stalls over 50/100/250 ms and achieved FPS over a fixed post-join
window. Raw CSV, presentation mode/vsync, refresh periods and every failed attempt
are retained. This does not measure physical scanout, input latency or offscreen
rendering; browser rAF and submission timings are never substitutes.

## Run

Use Node 24 and the client's existing locked dependencies (`npm ci --ignore-scripts
--no-audit --no-fund`). Put this config, all inputs and results outside Git:

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
  "targetAdapterLuid": "<AMD LUID from inspect>",
  "targetDisplay": "<4K monitor device name from inspect>",
  "targetRefreshHz": 240,
  "width": 1280,
  "height": 720,
  "x": 0,
  "y": 0,
  "seconds": 60,
  "rounds": 3
}
```

Run from `client/`:

```text
node tools/compare-bench/run.mjs inspect <external-config.json>
node tools/compare-bench/run.mjs prepare <external-config.json>
node tools/compare-bench/build-vlither.mjs <external-config.json>
node tools/compare-bench/run.mjs smoke <external-config.json>
node tools/compare-bench/run.mjs run <external-config.json>
```

Build the host separately with `cargo build --release --locked --features
frame-bench --bin slither-sim`. Pass a release desktop binary by path; its hash
identifies the code tested. Older binaries without passive view telemetry fail
acceptance clearly. No build or test is started during acquisition.

* **Primary:** AMD rendering on the 4K/240 Hz monitor. Actual per-process GPU-engine
  counters must resolve to the requested adapter LUID. Chrome gets a throwaway
  profile with an explicit ANGLE LUID flag. Native and current Vlither expose no
  selector; mismatches remain blocked. No Windows graphics setting is changed.
* **Secondary:** every client selects its default adapter, same window/display.
  Use `experiment:"secondary"` and omit the Chrome override. Mixed adapters are
  labelled comparisons. Optional RTX runs are a separate experiment.
* Real runs reverse client order over at least three rounds, with separate full
  High-requested Original and Native runs. Smoke uses primary only and five-second
  windows. All runs record actual monitor refresh/DPI, render/output LUIDs, machine
  and host CPU load, host tick health, pellet census and camera fit. Occlusion,
  loss of foreground, death, overload or missing evidence invalidates a result.

The scenarios are Battledome with 100 internal bots and whole-map interest, and
a normal map with 500 internal bots. Normal stress triples both ambient attempt
rate and food target: after population scaling, target **163500**, refill
**2406.845341348622 attempts/s**. Actual physical, received and visible counts
are separate. The feature-gated host supplies identical circular steering and
socket-player body-collision protection; bots and boundary/pit rules remain real.
This altered local workload is disclosed in every result.

Replace `mod` to use updated NTL; package/tree hashes and manifest version are
recorded, and changed bootstrap/view contracts fail preflight. Set `vlitherRef`
to pin a revision; omission fetches the latest default branch and records its
exact commit. The Vlither build requires explicit user-space tool paths and a
**labelled authored launch/telemetry wrapper**, without changing stock rendering
or zoom limits. See [method and setup details](method.md) for build inputs,
PresentMon rights, network isolation, adapter controls, survival and evidence
limits. It never elevates or changes group membership.

Offline tests: `node --test tools/compare-bench.test.mjs`. These use authored
synthetic inputs and need no client, GPU, ETW session or game server.
