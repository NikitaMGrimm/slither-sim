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
PresentMon 2.6's `--track_etw_status` reports loss, buffer pressure and present-ring
overflow each second. Positive loss/overflow ends the attempt at the next monitor
check; a missing CSV after ten seconds also ends it. Only that round enters the
existing bounded retry queue. Failed evidence remains retained. On early exit,
the exact task-owned trace is stopped before killing its consumer, avoiding an
orphan. No unrelated trace is stopped. Historical loss without these live
counters cannot be attributed retrospectively.
Cleanup also runs after interruption or a nonzero consumer exit. PresentMon 2.6
returns exit 7 if the session already disappeared; only its exact missing-session
diagnostic for this owned name is harmless. Other cleanup failures remain errors.
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
node tools/compare-bench/run.mjs preflight <external-config.json>
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
Before any timed round, a fresh untimed NTL preflight joins local Battledome and
checks 80, then 320, then 640 additional wheel notches, retaining every picture.
Three outer-circle scales must plateau within 3%. Host radius and projected
player offset anchor the circle (4% radius or 30-pixel camera/receipt allowance),
rejecting an inner pit circle. Unknown extent stops preflight. The receipt is
bound to this session's mod hash. If clipped and NTL participates, **every**
Battledome row (Original, Native, NTL and Vlither) is `ntl-farthest`, including
first-round telemetry controls. Separate whole-map rows are omitted. Normal
remains stock zoom. Without NTL, or when a new package actually fits the map,
Battledome retains whole-map behavior. Directed stock wheel input targets the
calibrated physical scale; timed views must remain within 5% of that reference, so
two accepted rows can differ by up to 10.53% (1.05 / 0.95). Matching the view
removes only the different-view qualification: unestablished content/display
correlation, missing exact scene evidence and different/unknown presentation
modes still prevent a direct comparison. No acceptance gate is relaxed.
Each Battledome NTL row applies the same UI settings used by calibration: zoom
enabled and slow return disabled, even when the calibrated extent is limited.
Visible zoom-control settings must match the reference receipt. Rows use 80
notches (the calibration's first plateau sample) and check two anchored pictures against the reference before
warmup: scale within 5%, settled within 3%, and the same whole-map state/row label.
Calibration with inconsistent whole-map states fails rather than choosing a
label. A limited-view row that reports whole-map state is invalid. The wheel
cadence and settling never count as warmup or measured time. Calibration still
uses all 1,040 notches. The shorter row setup saves about 77 seconds of wheel
cadence across the four NTL Battledome acquisitions, without reusing acceptance
from another package or skipping the scale/settling checks.

Vlither's stock wheel path multiplies `ms_zoom` by an exponential with its
configured `zoom_step` (fresh settings default 0.1); the GLFW scroll callback
accepts fractional offsets. The adapter reads that step and the resulting zoom
from the 0.5 s census. It checks both adjacent whole notches and prefers the one
with the smaller relative error when it fits the same 5% reference tolerance.
Only when neither fits, directed `WM_MOUSEWHEEL` supplies a
fractional offset in 1/120-notch units through the same stock GLFW callback.
Every adjustment and its method is recorded. Clamps and renderer stay unchanged;
an unreachable target or unsettled camera fails before warmup. The 5% timed
tolerance is unchanged. See upstream [input](https://github.com/for-loop9/vlither/blob/8612bc12186547fc385ea38a399b144973338384/app/src/game/input.c)
and [callback](https://github.com/for-loop9/vlither/blob/8612bc12186547fc385ea38a399b144973338384/thermite/src/framework/tmouse.c).

Vlither `gsc` is already in physical framebuffer pixels per world unit, as used
by [redraw](https://github.com/for-loop9/vlither/blob/8612bc12186547fc385ea38a399b144973338384/app/src/game/redraw.c).
Do not apply the native client's backing-size conversion or a CSS/DPI multiplier.
The wrapper reports framebuffer, renderer and context sizes; all must equal the
measured physical client area. Two unobscured, foreground client-area PNGs must
show an outer circle anchored to nearby host radius/player receipts, with pellet
presence, within 5% of both telemetry and NTL's reference and settled within 3%.
The circle fitter uses red/gold edges, including a filled exterior tint, rather
than arbitrary samples inside a filled region. Pictures are captured/decoded
before warmup, not during measurement. Missing/contradictory picture evidence
fails closed. This is scale and presence evidence, not exact pixel scene counts.

## Isolation and overhead

NTL loads in an authored local HTML shell under a hostname mapped to loopback.
No official page/source/assets are fetched. If an update requires them, stop and
report the bootstrap requirement. A new throwaway profile carries disabled local
network access experiments; the owner's profile is untouched. DNS denies all
other hosts, a loopback deny proxy rejects external requests/CONNECT, QUIC is off,
and WebRTC `disable_non_proxied_udp` is set before loading the extension. Netlog
Default records transport peers; remote TCP/UDP/QUIC/socket evidence outside the
named allowance below, or a missing successful loopback game connection, fails
proof. Keep logs private.
NTL negotiates protocol 13; host census checks the negotiated protocol and life.
Untrusted endpoint values use structured CDP arguments, not constructed source.
The untimed preflight closes Chrome and checks its complete netlog before timed
rounds. One named allowance, **no-packet reachability probe**, permits only UDP
connects to exactly `[2001:4860:4860::8888]:443`. The underlying UDP socket and its
UDP-client wrapper must both have complete closed lifetimes, only the recognized
non-I/O event shape, and source dependencies linking them to an uncached IPv6
reachability check in that lifetime. Any sent/received bytes or socket I/O event
(even a zero-byte event), TCP, another endpoint, unknown event shape, missing
attribution or missing closure rejects the allowance. Port changes fail closed
with an error naming this allowance and the rejected peer.

The [verified Chromium revision](https://github.com/chromium/chromium/blob/f842f52ef39fd97b449eb27cff414bf05a98ddce/net/dns/host_resolver_manager.cc#L1644)
connects a datagram socket and inspects its local address on success, without
socket reads/writes; its configured probe port is 443. Retained netlogs confirmed
the closed, zero-I/O source chain. This is an explicit external socket attempt,
not a claim of no external socket attempts. Each network proof records
`noPacketReachabilityProbeCount`, the named `allowances` with per-socket receipts,
and `externalSocketAttemptsObserved`. All other non-loopback traffic fails.

`preflight` runs only NTL's untimed local join, zoom, network and display checks;
it never starts PresentMon or timed rounds. Its single display snapshot is not
continuous run acceptance. A display-only failure returns exit 2 with the network
and zoom results retained, without touching an occluding owner window.
For `run` and `smoke`, any missing or non-passing NTL display gate raises the
named `NTL display preflight rejected` error with its reasons before a run plan
is written or any round begins.

Native clients have a separate, explicitly incomplete safety record. An
unprivileged helper starts just after client launch and periodically samples
TCP connections (including non-established states) and UDP local bindings for
the observed owned process tree, with process creation times guarding PID reuse.
It continues through client shutdown; actual sample timestamps, startup delay,
errors and observed process IDs are retained. The requested interval is one
second plus collection time. Any observed non-loopback or wildcard communicating
endpoint invalidates, including a connection present only in an early sample.
Windows wildcard TCP `Bound` reservations with unspecified peer and remote port
zero are retained separately: they are not connections or listeners. Wildcard
TCP listeners and UDP bindings still invalidate.
The record is labelled sampled and complete:false, never networkVerified.
Windows UDP tables do not expose per-datagram destinations; short connections or
children between polls can be missed. Incompleteness alone does not invalidate a
native run; unavailable collection is a tooling failure. This helper adds host
CPU load during measurement and remains enabled in both telemetry controls.

CDP Network is enabled only for join confirmation and disabled immediately on
Setup. Events have a fixed bound; no WebSocket frame serialization or game-state
polling continues during the window. Screenshots run before/after the timed
window and, with telemetry on, every 1.5 seconds plus capture time for bounded
view coverage. Timestamped PNGs are decoded after the window. Capture/readback
itself can perturb the client; the control below includes that cost.
The production probe still adds per-draw/rAF observation, and Default
netlog still adds logging. These costs must not be assumed negligible.
Our opt-in telemetry still scans food/snakes and logs every two seconds; the
food census uses one traversal. The native journal adds one bounded append per
successful present and no per-frame I/O. Vlither's wrapper census also has cost;
Vlither has no telemetry-off control, so its instrumentation cost is unmeasured.

Real runs pair telemetry on/off in their first round for Original, Native and
NTL, alternating the order by client. When NTL is clipped, Battledome controls
use only `ntl-farthest`. `run-plan.json` records the full matrix: four client/
quality variants, two scenarios, three rounds and six first-round off controls
give 30 acquisitions. Budget about 50-65 minutes before retries, a planning
estimate rather than a measured session duration. Dropping eight whole-map
acquisitions and shortening NTL's wheel sequence saves time; Vlither adds
untimed wheel matching and picture checks.
Set `telemetryControls:false` only after the initial perturbation session; an
explicit `telemetry:on|off` selects a separate diagnostic. Retain ETW intervals
and per-process CPU for both, and report changes in median/p95/p99. Off omits the
browser production hook, timed screenshots and both native telemetry paths. A
matched native off row enables only a setup view line with zoom and viewport,
without food/snake scans or a present journal. Directed wheel input measures its
own scale, rather than inheriting another row's wheel response. After matching,
the harness creates its private stop file and requires the client's explicit
stop acknowledgement before warmup. The client drops both telemetry and the
file-polling path; neither continues in the measured window. `viewSetup` records
the measured scale, method and stop time. Binaries without setup-only telemetry
support fail before warmup; supply a binary built with that support. Off deliberately fails
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
Battledome adds synthetic pit-marker food. Both our client and Vlither expose received-uneaten counts, which are
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
Vlither's census counts living snakes selected by stock redraw visibility and
positive live opacity, not every delivered snake. Clipped views must not claim
that all delivered snakes are visible. The **host** still delivers whole-map
food and living-snake interest identically to every Battledome client, even when
the client camera is clipped; delivery and visible extent are separate evidence.

NTL screenshots report colored small-component **pellet candidates**, not exact
pellet counts; a fitted red/gold circle is a candidate visible boundary. Host
radius/player-offset anchoring qualifies its extent, not exact scene completeness.
Missing or contradictory anchors fail Battledome acceptance. Presence-only images
do not establish exact delivered snake/pellet counts. No exact count is invented.

Every stream carries a QPC-domain receipt timestamp: host stats/census and native
logs are mapped from UNIX receipt time using a fresh per-attempt QPC/UNIX pair, while
window samples use QPC directly and browser rAF uses timeOrigin. This mapping
includes IPC/scheduling delay and is not hardware synchronization. Current clock
mapping disagreement fails acceptance. Stream coverage needs both boundaries and
bounded gaps (window 750 ms; host stats/census 1200 ms; native views 3000 ms).
Browser screenshots must satisfy the bounded view coverage; slow captures or
missing anchors invalidate rather than filling gaps with stale observations.
Telemetry-off diagnostics deliberately lack continuous scene proof. Host
life/delivery census continues throughout.

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
without physical input, before view setup/pictures. The same physical-input token
is checked again after warmup and carried into window sampling; setup input or
focus loss never causes a second foreground request. Input/focus loss or occlusion stops sampling; obstructing HWND classes/PIDs
are recorded privately, and no focus fighting occurs.
Interrupting collection stops the round loop without another preflight. The
current and remaining rounds stay in `uncollectedRounds`; retained attempts are
written before the summary, including when interruption occurs during preflight.
All acquisitions remain in the index and on disk. Comparisons select only one
unblocked, complete attempt per round, requiring exactly the planned row set.
Abandoned, interrupted, incomplete and superseded attempts are explicitly
excluded with reasons in the index, attempt log and round summary. The index
and summary share `selection` (selected attempts/directories and row count);
downstream aggregates must use that selection, never all retained acquisitions.
Readiness (all bot seats plus living player) has a 60-second bound. Smoke uses
primary, one round and five-second windows; real runs require at least three.
Original/Native are separate full High-effects variants with isolated preferences;
adaptive quality stays stock and is logged. No reduced-effects mode is added.
All owned windows, servers, children and unique trace sessions close on failure,
including evidence-write failures. No named-client numbers are published by this
tool; publication is the owner's decision.

Offline checks: `node --test tools/compare-bench.test.mjs`. Authored synthetic
fixtures require no clients, GPU, game host or ETW, and join the existing CI tests.
