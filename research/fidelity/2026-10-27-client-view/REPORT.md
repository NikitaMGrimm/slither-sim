# Client-view fidelity: prediction, corrections and remaining mechanisms

## Result and decision

The pinned Rust client provides an independent packet/presentation implementation
for a first **numeric client-view comparison**. It does not recover authoritative
server rules. No runtime behavior changes, new official-server collection, or
adoption gate are part of this study.

In three retained official windows, visible other-snake head innovation p95 is
**6.31–8.77 world units**, versus **2.94** in one local host window. Immediate
corrected-head changes are much smaller: **0.178–0.203**, versus **0.086**.
These are different populations, routes and behaviors; smaller corrections do
not establish greater fidelity. The local sample has **no** visible 100–169-segment
cruise exposure. There is no matched large-snake or equivalence conclusion.

Two source findings change how to interpret the remaining gaps:

- **106 versus 111 is not a universal fixed count offset.** The equivalent count
  deficit grows with size: 1.58 segments at 37, 4.41 at 100, 13.11 at 293.
  Fractional mass, the length end adjustment and retained dying points do not feed
  the client's `scang`. Count timing remains relevant during growth/loss, but the
  inspected code supplies no explanation for a settled fixed-size turn slope.
- **Large prey correction distances are expected even with the existing model.**
  Client prediction moves smoothly; SlitherSim floors prey coordinates each 7 ms
  substep. One retained official diagonal agrees with that flooring arithmetic.
  Do not interpret a 55-unit prey innovation as a 55-unit simulator physics error.

Recommended next work: **(1) a fixed-size turn/count-phase investigation;
(2) matched local scheduler/transient experiments before collecting more timing
data.** Capture proposals are below for owner approval; none were executed.

## Evidence tier, pins and access

This is exploratory source analysis plus retrospective retained-recording and
synthetic local-host measurement. It fits no new authoritative rule and changes
no fixture expectation, contract ID, workspace dependency or default. Historical
turn, ramp and prey residuals in the task are context, not independently rerun
mechanics qualification.

| Component | Revision / identity |
| --- | --- |
| SlitherSim runtime and host | `aee542100496b8cbc031b8ab19bb58e78bd8af2b` |
| SlitherClient, clean `origin/main` clone | `8d0772c015d6b9b73b80253db8c7349aa600b201` |
| Client dependency | Git revision above, locked in [Cargo.toml](harness/Cargo.toml) and [Cargo.lock](harness/Cargo.lock) |
| Host scenario | [host-config.json](host-config.json): seed 7, 128 Forager bots, one socket observer, fixed radius 12000, 12000 food, 80 prey, official delivery |
| Original acquisition / replay resources | Two logical-CPU affinity bits; `-j 2`, Rayon/test threads 2, host bot workers 1; runs serial, before the owner's resource-limit reduction |
| Subsequent checks / reproduction defaults | One logical CPU; `-j 1`, Rayon/test threads 1; one build or harness process at a time |

The owner's feature checkout was inspected read-only for its initial instructions
and status. All client source analysis used the separate clean pinned clone.
Neither checkout was changed. The standalone research crate has its own empty
workspace declaration; the simulator workspace has no client dependency.

**Access: local-only.** Full official recordings, local recordings, per-packet
rows, logs, early attempts and private locator files remain outside Git in the
task scratch archive `client-view-20261027`. There is no public raw-data URL.
[selected-results.json](selected-results.json) contains bounded aggregates,
capture/output/tool hashes, source revisions and coverage; hashes identify bytes,
not download availability or scientific truth. No official JS, artwork,
credentials, endpoint addresses, nicknames or machine/account details are exported.
SlitherClient's authored source is MIT; no official source was consulted or copied.

## Formula map

The complete source inventory, units, phase order and exact pinned file/line
citations are in [FORMULA-MAP.md](FORMULA-MAP.md). Its scope includes received state,
animation, body/length/score, prey, food, host release and physical update owners.

| Area | Client prediction / presentation | Simulator model and implication |
| --- | --- | --- |
| Turn and `scang` | `s=min(6,1+(n-2)/106)`, `Q=.13+.87*((7-s)/6)^2`; turn by `mamu*d*Q*min(1,sp/spangdv)`. Logical count, not smoothed length. | Physical scale uses 106; authoritative turn uses 111 and a lower scale clamp of 1. Normal speeds saturate the speed-turn factor. Integer count timing cannot be substituted for a settled size-proportional difference. |
| Speed / boost | Retains reported speed for travel; no held-boost physical acceleration. Visual speed eases by .1 rising/.3 falling, plus .0001, once per update. | Normalized progress uses .015/.02 per 8 ms, divided by `1+.305*(scale-1)`, giving the 4/3 ratio. The visual easing is not independent support for these physical slopes or growth retention. |
| Head / time | Turn then move `sp*d/4`, capped at one body spacing per update. `d=clamp(elapsed_ms/8,0,5)*lag`. Normal packet extrapolation is zero. | Physical travel has no presentation cap. Elapsed headers, receipt timestamps, animation steps and physics ticks are different clocks; raw prediction error includes stale speed, steering and route delay. |
| Body anchors | Insert new front, retain a dying tail, then contract from new index 3. Visible corrections use 53 Float32 samples. | Contract old body from index 3 before inserting front; non-growth removes tail physically. This source difference deserves explicit geometry-phase checks, not an unvalidated physical rewrite. |
| Head / angle rings | Head offset refreshes immediately; angle ring has 26 samples and refreshes on later positive whole steps. | A correction innovation is not necessarily an immediate displayed positional snap. Raw contraction is not a separately observed body error. |
| Size / `fam` timing | Growth increments logical count and recomputes scale before length scheduling and reanchor. Fam-only packets change length, not scale. Tail packets decrement logical count; fading points remain. | Speed samples count before boost loss; turn samples it afterward; wall contact retains the earlier scale; anchor growth precedes collisions and pickups. These are distinct phase hypotheses during transitions. |
| Prey | Full-width angles, smooth `sp*d/4` motion, received steering, 53-sample positional corrections. No autonomous threat/flight inference. | Per-7-ms absolute-coordinate flooring; independent decision clock; fitted threat/flight speed; unknown activation/departure triggers. Quantization alone can dominate raw client innovations. |
| Food | Sector/offset decoding, 6-unit wobble, spawn fade and consumption flight. | Physical food is stationary; nutrition, pickups, birth admission and recipient filtering are separate. Fade or received occupancy cannot identify global generation. |
| Radius / wall / collision | Eases received boundary targets. Head predictor has no wall sliding or authoritative collision solver. | Radius controller is a product model; sliding is calibrated; all snakes move before collision resolution. None is recovered by reading client animation. |

For unsaturated counts, equality of the turn formulas requires
`n_client = 2 + 106*(n_sim-2)/111`. The required deficit is `5*(n_sim-2)/111`.
At the same count, the simulator/client rate ratio is 1.0045 at 37, 1.0136 at 100,
and 1.0514 at 293, assuming equal clocks, coefficient and saturated speed factor.
One missing segment, a five-segment offset, the -0.6 displayed-length adjustment,
or a constant phase delay cannot explain that entire relationship. A phase delay
changes an intercept, not a settled sustained slope. This rules out those simple
explanations; it does not prove a server implementation or establish 111 beyond
the retained fit's range.

## Replay method and definitions

[replay.py](replay.py) reads only capture envelopes/base64 and connection/framing
metadata. Gameplay bytes go to the pinned client's `decode_frame`,
`ClientState::apply_frame_with_events` and `AnimatedClient`, including its food
and prey worlds. Metric values do not use SlitherSim's decoder. The existing
packet census is used separately for archive completeness and source-hash checks.

Every original frame is validated first. Packet probes retain original order and
share a timestamp; they introduce no animation updates between bundled commands.
After every frame the complete snake objects, food world and prey world must equal
an independent clone fed the original complete frame. **43,315 primary frame checks
pass**, and all corresponding checks pass in both sensitivity modes. Received-state
frame counters/header bookkeeping are deliberately outside that comparison.

Reset per setup/connection; preserve identities through removals and reuse within
an epoch. First full appearance identifies own. The native generic presentation
leaves game-over opaque, so the measurement gates live-own exposure at `v` before
following bundled packets, as the existing census does. Own removal/death also
gates exposure via the client's active-own lookup, including non-death removals
that retain an undrawn object marked alive. All post-death/undrawn data remain in
local diagnostic rows.

Primary animation is a deterministic 60 Hz clock, initialized at setup, with
lagging=false. At each tick, advance the actual client and prepare one numeric
1100×800 draw using its camera/zoom/culling. Packet arrival samples the state from
the most recent tick: there is up to 16.67 ms of animation phase, deliberately
part of this view. Asset factories supply fixed **synthetic numeric metadata without
pixels**, with fixed entropy; this does not change head/prey motion. No real browser,
Wasm runtime, images, frame stalls, input controls or visual quality were tested.

| Metric | Definition / denominator |
| --- | --- |
| Head innovation | Distance between raw predicted peer head before an anchor and raw head after the client's zero-extrapolation reanchor. One sample per known, same-object anchor. |
| Immediate head change | Distance between corrected heads immediately before/after that packet; internal packet-time state, not a filmed teleport or a draw-to-draw velocity measurement. |
| Body correction | New front's positional correction, plus maximum raw contraction and maximum corrected-position change across existing physical points. Match each old point to new index `i+1`; exclude the inserted front from existing-point comparisons. One maximum per anchor, not one statistical sample per point. |
| Heading innovation | Shortest absolute angular distance from raw prediction to a present reported heading. Omitted fields supply no sample. This is an endpoint innovation, not angular-velocity fit error in rad/s. |
| Speed innovation | Absolute difference from last retained speed to a present report. Units are wire speed per 32 ms; divide by .032 for world units/s. It measures held-speed prediction, not latent acceleration recovery. |
| Prey innovation | Same-instance, uneaten raw position correction; also corrected-position change, present heading and speed innovations. Received prey is a diagnostic; the primary visible subset requires its corrected center inside the viewport while own is live. This center test is not sprite-edge or occlusion visibility. |

Snake primary samples require the client's last numeric draw to mark the peer
visible and own to be live. Body visibility is whole-snake visibility; a visible
snake's head need not itself be onscreen. Quantiles use nearest rank within each
window. Samples are event-weighted and correlated; they are neither time-weighted
player experience nor independent trials. Every tail, long gap and maximum remains;
no residual-size censor, outlier trim or pooled equivalence test is applied.

## Selection and comparisons

Select the first **120 seconds after the first incoming receipt** of `low-2/network0`,
`low-3/network0`, and `second-1/network0` from the retained October 25 capture archive.
These are convenience-selected adjacent/second-invocation timing recordings, not
held-out samples for this new metric. Selection was fixed before outcome inspection;
every reconnect/setup in each window remains. Initial `low-1/network0` has only
header/metadata and no incoming frames: retain its hash as zero-exposure exclusion.
No new official session or replacement was attempted.

Local: unchanged pinned release host, 30 seconds warmup, then one 120-second
loopback Forager observer recording with [capture_local.py](capture_local.py).
This is an actual socket-host recording, not a hand-built traffic fixture or
virtual-time substitute. All data and the sole admitted life remain. Food/population,
observer policy, snake size/boost mixture, route and load are not matched to the
official samples. Official total population was not extracted by this harness;
the configured local 128 bots are not evidence of matched population.

### Visible peers, receipt clock / 60 Hz

Distances are world units. Heading is radians. Speed is units per 32 ms.

| Window | Frames / setups | Anchors | Head p50 / p95 / p99 | Immediate head p95 | Heading samples / p95 | Speed samples / p95 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| official-low2 | 11302 / 4 | 849 | 1.982 / 8.766 / 27.663 | .203 | 1862 / .07998 | 549 / .88889 |
| official-low3 | 11544 / 2 | 940 | 1.968 / 6.431 / 27.490 | .178 | 1886 / .09817 | 662 / .88889 |
| official-second1 | 11375 / 1 | 880 | 1.995 / 6.312 / 11.041 | .187 | 1660 / .09817 | 742 / .83333 |
| local-seed7 | 9094 / 1 | 627 | .955 / 2.944 / 5.656 | .086 | 2061 / .04952 | 277 / .83333 |

Maximum head innovations are 60.00, 127.62, 20.15 and 6.98 respectively. The
headline quantiles must not hide those tails. Speed p95 .83333–.88889 corresponds
to 26.04–27.78 world units/s; it is stale-report variation, not the task's
approximately .1 speed-ramp slope residual.

| Window | Anchor-weighted segment p50 / p95 | Wire-speed p50 | Body raw contraction max p95 | New-front correction p95 | Existing body immediate change max p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| official-low2 | 13 / 134 | 11.667 | 18.109 | 6.576 | 1.511 |
| official-low3 | 10 / 77 | 6.944 | 18.060 | 5.988 | 1.517 |
| official-second1 | 20 / 87 | 9.944 | 18.060 | 5.996 | 1.514 |
| local-seed7 | 24 / 45 | 5.889 | 18.060 | 2.555 | 1.413 |

The roughly 18-unit contraction is the client's deterministic reconstruction
response, not an independent authoritative-body miss. Correction rings greatly
reduce its immediate displayed change. The broad similarity does not validate
the simulator's different insertion/contraction phase.

The inherited 100–169-segment, speed<9 overlap stratum has only **30, 0, 17, 0**
visible anchors respectively. Head p95 in its two nonempty official windows is
19.929 and 8.738; the other cells are missing, not zero. No local large-snake
comparison or turn-denominator estimate is possible here.

### Prey and the quantized-motion counterexample

| Window | All received corrections / innovation p95 | Viewport-center corrections / innovation p95 | Viewport immediate change p95 |
| --- | ---: | ---: | ---: |
| official-low2 | 254 / 53.756 | 29 / 53.628 | .998 |
| official-low3 | 403 / 55.343 | 80 / 61.113 | 1.147 |
| official-second1 | 454 / 57.666 | 82 / 48.778 | .943 |
| local-seed7 | 298 / 55.202 | 8 / 55.202 | 1.067 |

The local visible sample has eight correlated corrections: its p95 is its maximum.
It cannot establish prey parity. Heading/speed distributions and exact present-field
denominators are retained in selected results; field omission policies differ, so
all-`j` heading/speed quantiles are not exchangeable samples of the same process.

A concrete source/arithmetic witness explains why prey innovation is so large.
In official-low3, connection 1, JSONL lines **496, 541, 589**, prey 566 has full-width
heading/wanted fields, direction `0`, retained speed **6.060**, and positions
`(37540,31474) -> (37588,31426) -> (37636,31378)`. Server gaps are 336 and 338 ms;
receipt gaps are 339.774 and 334.138 ms. The first heading is 6.071039047232 rad.
Smooth 7-ms travel is `(1.295906149,-.279121647)`; flooring the new absolute
coordinates from integer positions yields `(+1,-1)`. Forty-eight such modeled
steps yield exactly the observed `(+48,-48)` diagonal. Smooth travel over 335 ms
differs from that diagonal by **37.371 units**.

This checks the existing flooring explanation against a retained witness. The
3-unit transmitted position grid does not reveal the exact hidden step count;
this is not a new universal flooring proof. No shortened-angle parser bug is
supported by these packets. Matching raw prey corrections can coexist with
unknown threat decisions, the approximately .27 retained flight-speed residual,
or activation/departure rules.

### Clock sensitivity

Same complete input windows, no packet rescheduling or fitted clock offset.
Server mode substitutes cumulative per-setup elapsed headers for receipts; it is
an emission timeline, not known physics/input time. The 125 Hz sensitivity uses
8-ms animation updates on receipts; it is not a measured stock display rate.
Visibility and sample counts can change slightly with the clock.

| Window | Head innovation p95: receipt 60 / server 60 / receipt 125 Hz |
| --- | ---: |
| official-low2 | 8.766 / 5.771 / 7.024 |
| official-low3 | 6.431 / 5.719 / 5.676 |
| official-second1 | 6.312 / 5.696 / 5.282 |
| local-seed7 | 2.944 / 2.932 / 1.935 |

Timing and animation phase materially move this metric. Those sensitivities
cannot isolate a hidden dirty comparator, distinguish network from all host
effects, or supply a causal estimate for scheduler replacement. They do show why
wire-fit residuals and client-view corrections need separate reporting.

## Ranked gaps and investigation plan

These are **source-informed priorities**, not impact estimates identified by the
unmatched comparison. Impact rank is expected breadth of ordinary visible motion;
rare wall/collision events have high consequence even with a lower breadth rank.
Testability rank concerns isolating a mechanism, not merely gathering more traffic.

| Gap | Visible breadth rank | Testability rank | What remains / discriminating evidence |
| --- | ---: | ---: | --- |
| Update/send scheduler and dirty state | 1 | 6 | Affects all predicted entities; clock sensitivity is material. Unknown state-change/input-application times prevent general dirty-rule identification from receipt gaps alone. |
| Body anchor / correction phase | 2 | 1 | Exact source-order difference is testable offline with authored geometry and packet-by-packet reference-client execution. Authoritative online body truth still needs independent evidence. |
| Turn size scale and segment timing | 3 | 2 | Systematic size dependence, 5.14% rate difference at 293; fixed-size sustained turns separate it from transient count phase. |
| Boost ramps through growth/loss | 4 | 3 | .305, 4/3 and latent progress across changing size remain unqualified. Establish candidate separation beyond wire/time uncertainty before capture. |
| Prey steering / flight / activation | 5 | 4 | Shared large raw innovation has a quantization explanation; present-field speed/heading coverage and controlled threat transitions are more informative than total correction size. |
| Food generation versus filtering | 6 | 7 | Broad abundance/workload consequences; two recipients can establish delivery differences, not the global rejected-birth process. |
| Wall sliding | 7 | 5 | Large local consequence at the rim; compare authoritative anchors to physical target radius, not eased drawn radius. Controlled shallow approaches are feasible but contact times remain bounded. |
| Radius controller | 8 | 8 | Client reveals easing only. Population/clock correlates have not identified the official controller; freeze competing forecasts before a new window. |
| Online collision move/test phasing | 9 | 9 | Death outcome is high impact; breadth is unmeasured. Requires carefully controlled near misses, coverage and hidden-opponent exclusions, with opposing phase predictions separated by quantization. |

### 1. Turn scale, count phase and anchor geometry

Start offline: reconstruct the original fit's count convention from retained
fixtures and compare logical count, physical anchor count, dying count, fam and
displayed length independently. Use the known body-order difference as a
deterministic geometry probe, not a reason to change physics. Freeze competing
106/111, fixed-count-offset and event-phase candidates before evaluating fresh data.

**Proposed capture, approval required:** at most two simultaneous connections,
one actor and one nearby observer. Target stable sizes near 37, 100 and 244, with
both turn directions and nonboost/boost plateaus. Acquire at most 20 minutes per
invocation; stop inconclusive if target sizes or safe shared visibility are not
reached. Do not replace a failed invocation. Use two development and two later
held-out invocations, with the latter protocol frozen before acquisition.

Maintain a deterministic target at last-received actor heading plus/minus pi/2,
refreshed every 42 ms, recording every outgoing command and both complete streams.
Predeclare sustained-turn eligibility: no growth/tail change for at least one
second around a scored interval, sufficient wanted-heading margin, continuous
identity/visibility and a clean timing bound. Retain every excluded transition and
reason. Score unwrapped raw anchor directions/positions on the server timeline;
do not use smoothed rendered heading as authoritative rotation.

Compare settled size-dependent slopes first; then score separately the first five
anchors around growth/tail events for old/new-count phase predictions. A universal
offset predicts a constant count deficit, while the denominator model requires a
deficit proportional to n-2. Require model separation beyond angle/grid and
timing intervals at every scored size. Held-out failure or insufficient large-size
exposure leaves the result inconclusive; no fit-range extension by extrapolation.

### 2. Scheduler and boost-transient client cost

First run **matched offline local experiments**: freeze one world/input/seed trace,
retain authoritative state and original packets externally, and replay the same
state trace through named release-policy candidates. Keep client tick rate, draw
size, packet order, visibility, latency and size/boost strata fixed. Vary report
release only in a research adapter; never delay/reorder a removal behind ID reuse.
Compare correction distributions per life and stratum, including bytes/events and
long tails. This can rank player-visible cost without claiming an official dirty
rule. A slower, smoother or more frequent stream is not automatically more faithful.

Include constant-size rise/fall controls and legal growth/loss patterns during ramps.
Compare normalized-progress retention, direct retained speed, and old/new-size
sampling. **Power gate first:** one segment changes cruise speed by only .4/106
wire units below saturation, much less than a 1/18 speed byte; a single growth
event is not automatically discriminating. Advance to capture only if legal anchor
growth sequences or transition timestamps separate candidates beyond speed-byte
and timing intervals. Otherwise retain the negative power result and collect nothing.

**Conditional proposed timing capture, approval required:** two connections maximum,
actor plus observer, bounded 10-minute invocations. Interleave frozen constant-size
heading/boost holds and preplanned ramp/growth attempts, with pings and complete
send/receive logs. Separate commands sent while the six-tick gate appears armed
from open-gate controls; preserve all unchanged tuples and short-gap exceptions.
Use predeclared development/held-out invocations and candidate prediction intervals.
This can reject specified gate/rearm or growth-phase models; it cannot observe
server input-application time or identify arbitrary dirty-state rules. No claim
or tolerance is selected from favorable median receipt gaps.

## Reproduction and checks

Requires Rust 1.99+, Node 24 for the existing collector, and standard-library
Python. Git/crate downloads are build inputs; the study makes **no outside game
connections**. To build the host, use a separate checkout at the simulator pin,
not an arbitrary current binary. Use an external target/output root throughout.
The Python entry points limit their process and children to one available CPU;
set equivalent affinity for Cargo before invoking it directly.

```powershell
$study = 'research/fidelity/2026-10-27-client-view'
# $external is outside both tracked trees; $simPin is a clean checkout at aee5421...
cargo build --locked --release -j 1 --manifest-path "$study/harness/Cargo.toml" --target-dir "$external/client-target"
cargo build --locked --release -j 1 --manifest-path "$simPin/Cargo.toml" --target-dir "$external/host-target" --bin slither-sim
python "$study/capture_local.py" --repo "$simPin" --host "$external/host-target/release/slither-sim.exe" --output "$external/local-seed7"
# Private inputs.json contains the four labels and capture locators; never commit it.
python "$study/matrix.py" --inputs "$external/inputs.json" --binary "$external/client-target/release/client-view-research.exe" --output "$external/measurements"
```

Run the repository packet census on each full archive into the external evidence
directory (`official-low2-census.json`, `official-low3-census.json`,
`official-second1-census.json`, `local-census.json`). Preserve the local host
manifest and excluded low1 archive digest there. Then [select_results.py](select_results.py)
rebuilds only the bounded JSON from those checks and the matrix results. Fresh local
recording hashes will differ with socket/input timing; deterministic replay of the
same retained inputs is the reproducibility claim.

The owner's later one-CPU limit applies to all subsequent work. Disposable target
directories and repeated exported inputs are removed after checks; raw recordings,
attempt notes, manifests, summaries and selected per-packet evidence are retained.
Build the client/harness in its own external target directory and remove that
directory when finished. Set `RAYON_NUM_THREADS=1` and `RUST_TEST_THREADS=1`; use
`cargo test ... -- --test-threads=1` for explicit test invocation.

Completed checks:

- Locked release builds of pinned host and standalone harness, two CPUs.
- Harness formatting, Clippy `--all-targets -- -D warnings`, one Rust test and
  eight synthetic Python regressions: head smoothing, front-point identity,
  connection reuse, game-over and non-death own-removal gating, prey prediction/present fields, regressing
  time and invalid trailing-packet rejection; nearest-rank/missing exposure too.
- Twelve complete replays, zero presentation parity mismatches; four complete
  protocol-19 archive censuses with matching source hashes.
- Independent read-only source/evidence review identified a non-death own-removal
  gating defect. The main agent corrected it and added the removal regression;
  all twelve replays were repeated on one CPU with unchanged distributions.
- Pinned client source boundary guard: 245 index and 926 index/history versions
  passed, clean clone afterward. An initial wrong-cwd guard invocation rejected
  unrelated existing simulator artifacts; it was stopped and corrected. This
  invocation error and early test/replay attempts remain in external run notes.
- Documentation link/navigation test and repository diff/privacy review.

Full simulator Rust/tools/Wasm groups were not run: no simulator runtime or shared
tool code changed. No browser/pixel/reference-JS/live-server acceptance was run.
No capture proposal is approved or executed by this PR.
