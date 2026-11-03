# Visible-window frame pacing (Windows, opt-in)

This harness acquires local evidence for a supplied release desktop binary, an
owner-supplied NTL extension and a GitHub Vlither revision. Acquisition opens
visible windows and never runs in CI. All attempts, including failures, remain
outside Git. An acquisition is not a client performance verdict.

## Measures and acceptance limits

PresentMon 2.x `--v2_metrics --qpc_time_ms` supplies display timestamps from
`CPUStartQPCTime + DisplayLatency`, for exactly one owned PID/swapchain. Timestamp
differences give median/p95/p99, longest stall, counts over 50/100/250 ms and
achieved FPS over a fixed window after join. Gaps retain stalls; dropped presents
do not become displayed frames. The collector targets the owned render PID and disables optional GPU-duration
and input-latency tracking for every client; display tracking stays enabled.
GPU Engine counters identify the adapter independently. Raw CSV, boundary gaps, actual present modes,
SyncInterval and intervals in display-refresh periods are retained.

**Compositor presents are not necessarily new game pictures.** NTL additionally
reports its primary content-production measure: standard rAF callbacks containing
actual Canvas2D/WebGL draw activity, grouped by canvas and rAF timestamp. A CSS
animation cannot turn a 60 Hz canvas into 240 game frames/s. This is production,
not scanout, pixel uniqueness, or proof that a particular draw was displayed.
The probe wraps standard draw APIs; it does not inspect mod game state. WebGPU,
worker/OffscreenCanvas production and an ambiguous full-viewport canvas fail
acceptance rather than silently falling back to compositor FPS.

Our optional bounded in-memory journal holds 262144 successful present receipts
and prepared-picture identities (4 MiB), then exports on graceful exit. Repeated identities
are reported separately. A new prepared identity is not proof of changed pixels.
Vlither has no equivalent content/display correlation. Until exact correlation is
established, browser/native pairings remain **labelled comparisons only**, even
if acquisition gates pass. Different or unknown presentation modes also require
qualification; Composed Flip and Independent Flip are retained as findings.
Neither rAF nor offscreen/submission rates replace ETW display measurements.
This harness does not measure input latency or physical scanout.

`synthetic-content.html` is an authored control: a canvas capped at 60 Hz beside
a separate compositor animation. `node tools/compare-bench/synthetic-check.mjs
<external-config.json>` collects both production and ETW evidence in a visible
window, without a mod or game host. Its offline counterpart drives 240 callbacks
and checks exactly 60 productions. A blocked foreground acquisition is not a
successful live demonstration.

## Inputs and run matrix

Run from `client/` with Node 24 and existing locked dependencies (`npm ci
--ignore-scripts --no-audit --no-fund`). Keep config, results, third-party source,
assets, profiles and recordings outside the checkout. See [README](README.md)
for a minimal configuration. No local machine paths belong in committed files.

First run `node tools/compare-bench/run.mjs inspect <external-config.json>`.
It records DXGI adapter LUIDs, current DisplayConfig output bindings, physical
monitor rectangles, mode/refresh, DPI-related metadata and driver versions.
Task Manager's GPU index and WMI's resolution association are not identities.

| Experiment | Render adapter | Window/display |
| --- | --- | --- |
| `primary` | Same RTX adapter, verified by LUID | 3840x2160 physical client area on the selected 4K/240 Hz display |
| `secondary` | Chrome's unforced default | Same display and client size |
| `pixels-optional` | Explicit matching RTX request for Chrome | 1280x720 on the same display; separate experiment |
| `igpu-future` | Same iGPU if each client can be controlled | Future option; requires separate verification and owner-managed preferences where necessary |

Set `targetAdapterLuid`, `targetDisplay`, `targetRefreshHz`, `width`, `height`
and `x,y` explicitly. Primary requires NVIDIA, 4K and 240 Hz; use the inspected
LUID, never a vendor-name guess. Primary forces Chrome through
`--use-gl=angle --use-angle=d3d11 --use-adapter-luid=<signed-high>,<unsigned-low>`.
The secondary matrix uses `clients:["ntl"]`, `experiment:"secondary"` and no
Chrome override. Per-process GPU Engine 3D counters (Chrome GPU child) resolve
actual rendering to LUIDs independently of ANGLE/wgpu/Vulkan startup metadata.
Copy engines are retained but do not identify the rendering GPU. Failed counters
are skipped, current samples are cleared, and missing evidence fails acceptance.

Our client retains its LowPower request and exposes no adapter selector. Current
Vlither exposes no selector and favors discrete GPUs. The wrapper changes neither
selection policy. Unsupported native/Vlither requests fail clearly. The harness
never changes Windows graphics settings. An owner could set a preference for the
exact executable under Settings > System > Display > Graphics, restart, then
verify counters; explicit Vulkan selection may ignore that preference. Cross-GPU
pairs always remain labelled. DisplayConfig is queried on every sample: render
and output LUIDs can differ, and a change or disagreement in refresh invalidates.

Native (winit) and Vlither (GLFW) use borderless task-owned HWNDs, resized in
physical coordinates. At 4K, Chrome positions its app window through CDP and
uses the standard Fullscreen API; the browser owns its frame insets. Its
throwaway profile uses --test-type=gpu to suppress startup infobars that would
otherwise shrink even a fullscreen web viewport (see Chromium's
[infobar setup](https://github.com/chromium/chromium/blob/main/chrome/browser/ui/startup/infobar_utils.cc)).
The captured browser viewport must also match the requested physical client area
exactly. Chrome retains the OS device scale and records devicePixelRatio;
CSS rectangles are converted to physical pixels for canvas selection.
This is not a promise of exclusive fullscreen. The entire client
rectangle must lie inside the target monitor; spanning/clipping fails. Actual
refresh and presentation mode still determine the experiment.

## Builds, provenance and updates

Build the host separately with `cargo build --release --locked -p slither-sim
--features frame-bench --bin slither-sim`. Select the package even from `client/`.
Set local Cargo target/jobs limits per command. Never build/test during timing.
Pass a release desktop binary by path; SHA-256 identifies it independently of
repository HEAD. Optional `binaryReceipt` must match its executable hash and can
record a base commit, passive instrumentation patch hash and build command.

```
node tools/compare-bench/run.mjs prepare <external-config.json>
node tools/compare-bench/build-vlither.mjs <external-config.json>
node tools/compare-bench/run.mjs smoke <external-config.json>
node tools/compare-bench/run.mjs run <external-config.json>
```

`prepare` fetches https://github.com/for-loop9/vlither and checks out `vlitherRef`,
or the latest remote default-branch tip. It refuses tracked dirt and records the
exact commit. Build only in the configured scratch root. Vlither uses Premake,
GCC, Slang and Vulkan headers/import library. Supply `vlitherTools` absolute paths
named `premake`, `make`, `gccDirectory`, `slangc`, `vulkanHeaders` (containing
`include/vulkan`) and `vulkanImportLibrary`. Build uses `-j2` and keeps commands,
errors and the receipt outside Git. Admin-only installation is a blocker.

Vlither is explicitly a **labelled instrumented build**. The authored
`vlither-wrapper.c` includes its unchanged upstream main translation unit in
scratch, replaces its main object, joins literal loopback, requests FIFO/vsync,
feeds the stock wheel zoom path and emits a 0.5 s view census. It changes no zoom
range or renderer. Receipt hashes bind wrapper, executable and upstream commit;
changed interfaces must compile and pass protocol/view checks. Fresh per-attempt
settings isolate owner preferences; assets stay in the external checkout.
Stock Vlither uses IPv4:Port and protocol 19, supports wheel/N/M zoom and clamps
to 0.025. A camera unable to fit the map is retained and labelled limited view.

Replace `mod` with an updated folder, ZIP or CRX2/3 and rerun. Manifest version,
package hash and deterministic per-file/tree SHA-256 are recorded. Missing/failed
provenance prevents loading. Extraction rejects traversal, symlinks and expanded
packages over 512 MiB. The only NTL-specific adapter is `ntl-ui-adapter.mjs`:
visible login/settings DOM labels, including the mod's zoom-enabled checkbox and
slow zoom-return setting. No minified globals, functions or food arrays are read.
Changed UI fails preflight clearly. Whole-map zoom uses repeated standard wheel
events, then screenshot evidence. It never assigns a hidden zoom variable.

## Isolation and overhead

NTL loads in an authored local HTML shell under a hostname mapped to loopback.
No official page/source/assets are fetched. If an update requires them, stop and
report the bootstrap requirement. A new throwaway profile carries disabled local
network access experiments; the owner's profile is untouched. DNS denies all
other hosts, a loopback deny proxy rejects external requests/CONNECT, QUIC is off,
and WebRTC `disable_non_proxied_udp` is set before loading the extension. Netlog
Default records transport peers; remote TCP/UDP/QUIC/socket evidence or absence
of the successful loopback game connection fails proof. Keep logs private.
NTL negotiates protocol 13; host census checks the negotiated protocol and life.
Untrusted endpoint values use structured CDP arguments, not constructed source.

Native clients have a separate, explicitly incomplete safety record. An
unprivileged helper starts just after client launch and periodically samples
TCP connections (including non-established states) and UDP local bindings for
the observed owned process tree, with process creation times guarding PID reuse.
It continues through client shutdown; actual sample timestamps, startup delay,
errors and observed process IDs are retained. The requested interval is one
second plus collection time. Any observed non-loopback or wildcard endpoint
invalidates, including a connection present only in an early sample.
The record is labelled sampled and complete:false, never networkVerified.
Windows UDP tables do not expose per-datagram destinations; short connections or
children between polls can be missed. Incompleteness alone does not invalidate a
native run; unavailable collection is a tooling failure. This helper adds host
CPU load during measurement and remains enabled in both telemetry controls.

CDP Network is enabled only for join confirmation and disabled immediately on
Setup. Events have a fixed bound; no WebSocket frame serialization or game-state
polling continues during the window. Screenshots run before/after the timed
window. The production probe still adds per-draw/rAF observation, and Default
netlog still adds logging. These costs must not be assumed negligible.
Our opt-in telemetry still scans food/snakes and logs every two seconds; the
food census uses one traversal. The native journal adds one bounded append per
successful present and no per-frame I/O. Vlither's wrapper census also has cost.

Measure perturbation separately: alternate otherwise identical `telemetry:"on"`
and `telemetry:"off"` configs for Original/Native and NTL, retain ETW intervals
and per-process CPU for both, and report changes in median/p95/p99. Off omits the
browser production hook and both native telemetry paths. It deliberately fails
scene/content acceptance and cannot become comparison evidence. Both variants
retain safety netlog, window/GPU sampling and host stats; that residual common
cost is not isolated by this control. If foreground/load gates block the paired
control, overhead remains unmeasured and a verdict remains blocked.

PresentMon must already have ETW rights; no self-elevation, trace takeover or
security/group changes occur. If denied, an administrator could add the account
to Performance Log Users and the owner sign out/in once, or the owner could run
the collector in an approved elevated session. Offline tests, preparation and
local host checks remain possible without it.

## Worlds and validity

Fresh hosts bind `127.0.0.1:0 --stats 0.5 --frame-bench`; readiness reads the bound
port and rejects 8766, 18884 and 18885. Existing `bots.count`/`bots.threads` run
in-process A4 workers, not socket bots. One socket player is reserved.

* Battledome starts from `preset:battledome`, 100 bots, stock seat-scaled radius
  (3584 reference, about 6367 at 101 occupied seats) and `vision:13000`. The
  nondefault host feature gives only the socket player all arena food sectors;
  raising vision alone would not expand food interest. Internal bots keep
  ordinary food interest; world-wide snake/prey vision also increases host cost.
* Normal has 500 bots, stock zoom, `sizeToPopulation:true`, `dynamicRadius:false`.
  "3x spawn" means three times ambient placement attempts/s and startup/refill
  target. Config values are `foodCount:327` and
  `ambientReplenishPerSecond:4.813690682697244`. Population area scaling gives
  radius 14981.655449248592, target **163500** and
  **2406.845341348622 attempts/s** (final floating-point digits may vary).
  Actual successful placement/visible counts are not promised to triple.

The feature is absent from ordinary builds and cannot be enabled through JSON.
It requires loopback. The socket player follows a radius-0.75 orbit, boost off,
and is protected from head/body collision deaths. Rim/pit deaths remain enabled;
bots still die on the player's body and affect workload, identically in policy
across clients. Any sampled death invalidates. Ordinary food `advance` is never
called for whole-map bench seats: `SectorInterest::all` changes `subscribed`
without `last_seen`; connection reset discards this transient state.

Census records current admitted connections, actual alive population, physical
pellets and delivered wire cells. Co-located pellets collapse in wire cells;
Battledome adds synthetic pit-marker food. Native received-uneaten counts are
checked against nearby decorated wire counts with an explicit 32 + 5% allowance
for delivery/sampling lag; retained eaten flights are excluded where exposed.
Visible/retained counts are instances, not opaque pixels or GPU quads.
Available drawn-live-snake counts are checked against host delivery receipts
within 1200 ms. Whole-map views must fall inside that nearby count range with a
two-snake allowance for appearance/death delivery and presentation lag. A living
local player must still be drawn; the allowance never excuses a zero count.
Normal/limited views check the delivered upper bound and living local player,
because the host does not know the precise visible subset. Contradictions
invalidate. Missing counts (including NTL without game internals) explicitly
leave drawn-snake scene acceptance unestablished and prevent direct comparison.

NTL screenshots report colored small-component **pellet candidates**, not exact
pellet counts; a fitted red/gold circle is a candidate visible boundary. Presence-only
image evidence cannot establish complete scene delivery or identify an inner
pit circle as the outer map. Exact NTL counts and whole-map extent remain
qualified until independently anchored. No image-derived count is invented.

Every stream carries a QPC-domain receipt timestamp: host stats/census and native
logs are mapped from UNIX receipt time using a fresh per-attempt QPC/UNIX pair, while
window samples use QPC directly and browser rAF uses timeOrigin. This mapping
includes IPC/scheduling delay and is not hardware synchronization. Current clock
mapping disagreement fails acceptance. Stream coverage needs both boundaries and
bounded gaps (window 750 ms; host stats/census 1200 ms; native views 3000 ms).
Endpoint-only browser screenshots intentionally do not satisfy continuous view
coverage for long windows; those attempts remain invalid, not silently filled
with stale observations. Host life/delivery census continues throughout.

Foreground, full visibility, containment, fixed size/display/DPI/refresh, known
render LUID, loopback proof, scene evidence and host tick health are required.
Deferred ticks/lost time or sampled rate below 98% of `1/dt` invalidate. Sampling
at roughly 250 ms cannot prove sub-sample focus/occlusion/death absence. Per-run
machine CPU, host CPU cores and client process CPU seconds are retained; Chrome
includes its dedicated children. Missing current GPU samples fail. Build/test
processes invalidate; the harness never terminates competing owner work.

A round checks competing work and owner input first, then alternates clients,
reversing order each round. At most three attempts per round retain all failures;
no quiet-time booking occurs. Foreground is requested once only after five seconds
without physical input. Input/focus loss or occlusion stops sampling; obstructing HWND classes/PIDs
are recorded privately, and no focus fighting occurs.
Readiness (all bot seats plus living player) has a 60-second bound. Smoke uses
primary, one round and five-second windows; real runs require at least three.
Original/Native are separate full High-effects variants with isolated preferences;
adaptive quality stays stock and is logged. No reduced-effects mode is added.
All owned windows, servers, children and unique trace sessions close on failure,
including evidence-write failures. No named-client numbers are published by this
tool; publication is the owner's decision.

Offline checks: `node --test tools/compare-bench.test.mjs`. Authored synthetic
fixtures require no clients, GPU, game host or ETW, and join the existing CI tests.
