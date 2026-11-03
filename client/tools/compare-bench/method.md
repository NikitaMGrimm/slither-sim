# Visible-window frame pacing (Windows, opt-in)

This local harness compares displayed-frame spacing for a supplied release
`slither-wgpu-desktop`, an owner-supplied NTL extension, and a GitHub Vlither
revision. It opens visible windows and never runs as a benchmark in CI.
Use only with the machine owner's permission. Failed attempts remain in the
external output directory; an invalid result is not a client performance verdict.

The primary measure is **PresentMon ETW display timestamps**, selected by one
PID and one swapchain. The timestamp is `CPUStartQPCTime + DisplayLatency` from
`--v2_metrics --qpc_time_ms`. Differences give median/p95/p99, maximum stall,
counts over 50/100/250 ms, and displayed frames per fixed window. Dropped frames
are excluded from timestamps, but gaps between successful displays remain.
Raw CSV, mode/SyncInterval, warmup, window-boundary gaps and measurement scope
remain available. No browser rAF, offscreen timing, submission rate or estimated
FPS substitutes for this measure. This is not input latency or physical scanout
measurement; DWM, driver overrides and ETW observability still matter.

## Inputs and preparation

Run from `client/` with Node 24 and `npm ci --ignore-scripts --no-audit --no-fund`.
No new Node dependency is needed; CDP uses the existing locked `ws` package.
Keep the configuration, third-party files and all output **outside the checkout**.
Use a roomy scratch drive. The harness never builds during acquisition.

Supply this JSON in an external file (replace the symbolic paths):

```json
{
  "output": "<external scratch directory>",
  "host": "<release slither-sim.exe built with frame-bench>",
  "binary": "<release slither-wgpu-desktop.exe>",
  "mod": "<NTL manifest folder, ZIP or CRX package>",
  "chrome": "<extension-capable Chrome for Testing or Chromium executable>",
  "presentMon": "<PresentMon 2.6 console executable>",
  "vlitherBinary": "<output>/vlither/build/bin/windows_x86_64_release/app.exe",
  "width": 1280,
  "height": 720,
  "x": 0,
  "y": 0,
  "warmupSeconds": 5,
  "seconds": 60,
  "rounds": 3
}
```

Run `node tools/compare-bench/run.mjs inspect <external-config.json>` first.
It reports physical monitor rectangles/modes and DXGI adapter **LUIDs**. Task
Manager's GPU number is not an adapter identity. Add `experiment:"primary"`,
`targetAdapterLuid:"<AMD iGPU LUID from inspect>"`,
`targetDisplay:"<4K monitor device name from inspect>"` and
`targetRefreshHz:240` to the external config. Primary automatically requests that
LUID for Chrome and invalidates any client that actually renders elsewhere.

The matrix is explicit:

| Experiment | Adapter request | Window/display |
| --- | --- | --- |
| `primary` (also the only smoke case) | AMD iGPU, actual LUID required | 1280x720 on the AMD-driven 4K/240 Hz monitor |
| `secondary` | Every client's default; no Chrome override | Same window size and monitor |
| `rtx-optional` | Only clients with an available selector | RTX-driven display; separate experiment |

Per run, Windows GPU Engine counters for the actual render process (Chrome's
dedicated GPU child) are mapped to DXGI LUIDs. Active 3D engines identify render
adapters; copy engines remain in the raw evidence and are not mistaken for the
render GPU. The app's selected wgpu adapter, Vlither's Vulkan physical device,
and Chrome's actual ANGLE renderer/device metadata are retained independently.
Missing counter/mapping evidence invalidates acceptance. The window's monitor
comes from its HWND; `EnumDisplaySettings` queries that monitor's current refresh,
and the collector uses physical DPI-aware coordinates. Intervals also report
display-refresh periods. A monitor move, refresh change or GPU mismatch cannot
pass as a direct comparison. `index.json` labels mismatched/invalid pairings.
DisplayConfig independently binds each active monitor path to source/target
adapter LUIDs and supplies its rational refresh rate. Rendering on an iGPU does
not prove the monitor is physically driven by it; WMI's adapter resolution fields
are not used to infer that relationship. Results explicitly label cross-adapter
render/output paths and preserve DXGI output mappings and WMI metadata separately.

Chrome accepts `adapter:{"chromeLuid":"<DXGI LUID>"}` using
`--use-gl=angle --use-angle=d3d11 --use-adapter-luid=<signed-high>,<unsigned-low>`
only in its throwaway profile. The harness verifies the **result**, not just the
flag. See [Chromium's LUID handling](https://github.com/chromium/chromium/blob/main/ui/gl/gl_display.cc).
`"default"` omits those flags for the secondary experiment.
The supplied native client has no adapter selector and retains its LowPower
request. Current Vlither exposes no selector and scores discrete GPUs higher;
the wrapper does not alter that policy. Unsupported explicit `native` or
`vlither` selections are rejected instead of ignored. To request another GPU
without a client selector, the owner would need Windows Settings → System →
Display → Graphics → the exact executable → Options → Power saving (AMD), then
restart and verify. An application that explicitly selects a Vulkan device may
ignore that preference: if counters still show the discrete GPU, primary remains
blocked until Vlither exposes a selector or an explicitly approved device-selection
change is supplied. The harness never changes Windows graphics settings or our
default. Secondary runs retain default GPU evidence to inform that later decision.

Use the same physical client area, position, display and DPI throughout. Pick
`x,y` on the intended display. The harness resizes using `SWP_NOACTIVATE`, uses
directed window messages for our client's Enter/wheel, and never calls
SetForegroundWindow, SendInput or moves the system pointer. Client startup itself
can activate a window. If the owner switches away or covers it, the attempt is
invalid; the harness does not reclaim focus. Do not minimize windows to hide them.

PresentMon must already have ETW rights. Test its console using a **unique**
`--session_name` with `--timed 1 --terminate_after_timed`. The harness never uses
`--restart_as_admin`, stops another trace, changes groups or elevates. If access
is denied, the owner can have an administrator add the account to Windows
**Performance Log Users**, then sign out/in once; alternatively the owner can
run the collector in an approved elevated session. Unprivileged preparation,
host smoke, source checks and offline analysis remain possible. Do not substitute
NTL rAF timing. [PresentMon's console reference](https://github.com/GameTechDev/PresentMon/blob/main/README-ConsoleApplication.md)
defines the CSV fields and permission limits.

Build the host once with `cargo build --release --locked --features frame-bench
--bin slither-sim`. Pass the client binary separately; its hash, rather than a
required commit, identifies the implementation under test. The ordinary desktop
binary's opt-in `SLITHER_FRAME_PACING=1` log now includes passive camera, quality
and pellet metadata every two seconds. Older binaries still launch, but lack of
view telemetry invalidates acceptance. Use a release build. Set Cargo target and
job limits **per command** according to local resource requirements.

```text
node tools/compare-bench/run.mjs prepare <external-config.json>
node tools/compare-bench/build-vlither.mjs <external-config.json>
node tools/compare-bench/run.mjs smoke <external-config.json>
node tools/compare-bench/run.mjs run <external-config.json>
```

`prepare` clones [Vlither](https://github.com/for-loop9/vlither), fetches origin,
and checks out `vlitherRef` (optional; otherwise the latest remote default-branch
tip). It refuses tracked dirt. The exact resolved commit is recorded, even when
the default ref moves. Build only inside the designated external scratch root.
Vlither uses Premake/GCC/Slang and Vulkan headers/import library, not CMake.
Normal user-level installation is sufficient where these tools are available;
an SDK installation requiring administrator access is a blocker.

For the build command, add `vlitherTools` to the external config with absolute
paths named `premake`, `make`, `gccDirectory`, `slangc`, `vulkanHeaders`, and
`vulkanImportLibrary`. `vulkanHeaders` is a directory containing `include/vulkan`;
the build adds a local `lib/libvulkan-1.a` import-library copy there. It uses
`-j2`, records commands and errors under Vlither's `build/`, and compiles shaders.
It does not install tools, change global PATH or edit upstream tracked sources.
Each attempt has a fresh Vlither settings file; external resource junctions
point to its checkout's assets, so owner settings are never reused or overwritten.

**Vlither is a labelled instrumented build.** The authored `vlither-wrapper.c`
replaces its main object in scratch, includes the checkout's unchanged main
translation unit, automatically joins only literal loopback, requests FIFO/vsync,
feeds wheel input into the stock zoom clamp, and emits a 0.5 s census. It adds
no zoom range or rendering feature. The build receipt hashes wrapper/executable
and pins the upstream commit; changed interfaces must compile and pass runtime
protocol/view checks. It is not an unmodified release-binary result. Normal
Vlither connects through the title screen's IPv4:Port field and uses protocol 19.
Source currently supports wheel and N/M zoom, with a stock minimum of 0.025.
The harness reports the reached scale and labels a camera that cannot fit the
arena plus camera offset as **limited view**, retaining the invalid comparison.

To update NTL, replace `mod` with the new folder/package and rerun. Package hashes,
manifest version, and a deterministic per-file SHA-256 tree are retained.
ZIP/CRX extraction rejects traversal, symlinks and packages over 512 MiB expanded.
No mod internals are rewritten. The minimal dependency is its bootstrap DOM,
local server picker and read-only camera/food globals. If they change, preflight
names the missing contract and refuses timing acceptance. Never refresh these
checks by guessing a new variable's meaning. Verify the updated mod locally.

NTL uses an authored minimal HTML shell served from loopback under a mapped
game hostname. **No official page, source or assets are downloaded.** An updated
mod that requires the public page is blocked until a local bootstrap is verified.
The extension stays inside Chromium. A new scratch profile stores disabled
`local-network-access-check` and `local-network-access-check-websockets` experiment
settings, with corresponding feature flags. The owner's profile is never used.
All DNS except loopback/mapped local game names fails; a local deny proxy rejects
external requests and CONNECT. QUIC/background networking is disabled. A page
WebSocket guard admits only the exact loopback host URL. Chrome netlog supplies
network-level TCP-peer evidence, separately from CDP WebSocket events. Unknown,
remote or missing peers invalidate the attempt. Keep netlog private.

## Worlds and validity

Both committed scenarios start a fresh host at `127.0.0.1:0 --stats 0.5
--frame-bench`. Readiness consumes the actual bound port; ports 8766, 18884 and
18885 are forbidden. `bots.count` and `bots.threads` are the existing host's
**in-process A4 workers**, not socket bots. One socket player is reserved.

* Battledome starts from `preset: battledome`, 100 bots, stock seat-following
  radius (3584 reference, growing to approximately 6367 with 101 occupied seats),
  and `vision:13000` (larger than that diameter from any in-arena player position).
  Census records actual radius, admitted bot seats and alive bots. The feature-gated host hook
  gives **only the socket player** every food sector intersecting the arena
  bounds; internal bots retain ordinary 45-sector interest and normal visibility.
  Raising `vision` alone would not expand food interest. A camera fit test includes
  offset from the arena centre; a zoom slider endpoint alone is not acceptance.
* Normal uses 500 bots, normal client zoom, `sizeToPopulation:true` and
  `dynamicRadius:false`. "3x spawn rate" means three times ambient placement
  **attempts/s**, plus three times the startup/refill target to sustain stress:
  base `foodCount:327`, `ambientReplenishPerSecond:4.813690682697244`.
  After host startup area scaling: radius 14981.655449248592, target **163500**,
  rate **2406.845341348622** attempts/s (floating-point final digits may differ).
  Food admission, eating, boost and death drops affect actual counts. Normal
  interest stays ordinary, including `vision:1500`. No claim of 3x successful
  placements or exactly 3x visible pellets is made.

The host-only feature is a minimal simulator-lane change. It is absent from
ordinary builds and cannot be activated by JSON. The CLI switch is rejected
on public binds. It overrides the socket player's heading toward a radius-0.75
orbit, with boost off, identically across clients. The socket player is exempt
from body-collision deaths for this measurement; bots still collide, including
against its body, and boundary/pit deaths remain enabled. This is an altered
benchmark workload, absent from ordinary builds and not mechanics evidence.
Any dead or unadmitted sample still invalidates the run. No client invulnerability
setting is shipped, and no third-party gameplay code is patched for survival.
The same hook reports authoritative total physical pellets and socket wire-cell
counts. Wire cells may collapse co-located pellets; Battledome decorations add
synthetic pit-marker food. Client `retainedPellets` and `visiblePellets` count
retained instances and camera-eligible instances, including eaten flights
and markers; they are not counts of GPU quads or guaranteed opaque pixels.
Subpixel LOD remains each client's own policy. Report all these separate counts.

Acceptance additionally requires a visible, foreground, nonminimized window
without any intersecting higher window, unchanged client size/display/DPI,
known adapter/refresh/vsync, the intended camera, loopback network evidence,
and host tick health. Any deferred tick/lost time or sampled rate below 98% of
`1/dt` invalidates. Sampling runs at 250 ms; it cannot prove that a sub-sample
focus change, death or occlusion never occurred. Keep that observation limit.
Window checks are deliberately conservative about harmless overlays.
Per-run machine CPU and host CPU cores consumed are recorded (divide by logical
processor count for percent of total CPU). Build tools invalidate a timed run.
Host census/monitoring overhead shares this machine and is included in load.

Before warmup, admission has a bounded 60-second readiness check for every bot
seat and the living socket player. That is workload readiness, not waiting for
the PC to become quiet; competing machine work is sampled during acquisition.
`smoke` uses the primary experiment, one round and five seconds per window. `run` requires at least three
rounds, reversing client order each round; Original and Native are separate
full-effects/High-requested variants with isolated preferences and no custom art.
Adaptive quality remains stock and is logged. No reduced-effects diagnostic is
added: matching NTL effects is not defined by a shared existing setting.
Failures are retained and collection proceeds to the next client/scenario.
No automatic quiet-time booking or unlimited retry occurs. For retries start
another session; never overwrite or discard failed attempts. Compare only runs
with matched conditions, and disclose actual present mode/vsync differences.

This harness does not publish results. Input paths, logs, profiles, CSVs and
screenshots belong outside Git. It closes only its own process trees, browser,
servers and trace session after each attempt, including failure/interruption.
Do not publish named comparisons without the owner's decision.

Offline parsing/validity checks (no clients, GPU, ETW or game server):

```text
node --test tools/compare-bench.test.mjs
```

These tests join the existing `npm test` glob; acquisition is never invoked by CI.
