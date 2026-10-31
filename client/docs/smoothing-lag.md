# Offline production-core smoothing probe

This is an open-loop CPU baseline, not a smoothing setting or a latency-to-screen
measurement. `feel-replay --smoothing-lag-probe` drives production
`receive / service_ingress / begin_redraw / take_frame / presented`. A CPU sink
acknowledges each issued picture immediately; no socket, window or GPU exists.
Production smoothing, input gates and policy are unchanged. Feel-trace v3 cells
are unchanged; the opt-in geometry journal is separate and uses the existing
immutable observation sequence through issue/retry/acknowledgement.

## Method and frozen schedule

Each run has 2 s warmup and 6 s measured exposure, at 960×540 CSS / DPR 1, high
quality and builtin skin 0. Own snake 7 and enemy 8 start 120 units apart with
twelve exact 42-unit anchors. Normal is **180 units/s**, boost **437.5 units/s**;
these are authored plateaus, not claims about a particular server's cruise speed.
Truth is an independent continuous polyline traversed at constant speed. Straight
truth has heading zero. Turn truth repeats headings 0, +π/8, 0, −π/8 for four
42-unit edges each. Position is continuous at corners; velocity changes there
are genuine authored turns. No production interpolation is used to construct
truth. Directed wire anchors have exact 42-unit spacing and quantized directions
matching these angles. The two snakes share a path translated by 120 units.

The 48 strata are normal/boost × straight/turn × jitter 0/±4/±12 ms ×
60/170/240 Hz/irregular frames. All position/heading packets have 20 ms base
delivery delay. Anchor delivery offsets repeat `[0,+1,-1,0,-1,+1,+1,-1]` times
the jitter amplitude, clamped to ordered delivery. No packet reordering occurs.
Pongs every 32 ms prevent unrelated lag damping; they do not refresh pose age.
Irregular frame intervals repeat `[4,7,19,5,11,23,6,9]` ms. Packet ties retain
generation order, and packets are admitted before a redraw at the same time.
Setup is prepared before appearances at the same logical origin, so initialization
does not accidentally advance a newly appeared head by pre-appearance time.

Each baseline has a second run with exactly the same schedule. It applies +24
world units in x to reported anchor positions during [3,4.3) s, and +π/8 to
reported headings during [5.3,6.6) s. Truth stays continuous and unchanged.
The first changed anchor uses an absolute correction; subsequent ordinary
anchors retain 42-unit steps. Absolute corrections quantize to integer world
units, so the positional step includes at most half a unit per axis of wire
quantization. These impulse runs are reported separately from baseline errors.
Paired head/heading differences measure the correction response against the
unperturbed production run, without subtracting it from the baseline numbers.

## Observations and metrics

At the existing numeric draw seam the recorder copies both selected snakes'
prepared raw/corrected heads, headings, scale and actual f32 painter centers in
head-to-tail order. It never prepares a second picture. Copies carry appearance
token, picture observation sequence, camera/zoom and packet-derived head.
The journal is limited to 4,096 prepared observations and 8,193 painter points
per selected snake; any loss fails the run, never samples it. Ordinary builds
contain no geometry hook. The finite driver drains every picture and exports
all warmup/measured frames, packets, lifecycle records, controls and failures.
An error during the production loop exports the observed prefix with
`completed:false`; the runner retains it and continues the remaining strata.

`e = truth − drawn` is reported as Euclidean norm, tangent and lateral components.
Positive `1000·e_parallel/v` means drawn behind; negative means ahead. This
lag-equivalent is undefined below 10⁻⁶ units/s and is **not a true delay during
turns**. Packet-state age uses the latest admitted anchor's generation time, not
the last pong or receipt time. Raw prediction is retained as a diagnostic, never
treated as truth. Correction magnitude is drawn-versus-raw head displacement.

Body stations are 0/42/84/168/252 units along each ordered painter centerline.
Each is compared with the continuous truth path at that same tailward arclength,
with signed lateral error and distance to its declared truth segment. No global
nearest-neighbor matching or time alignment is used. Missing stations are retained
as failures. Body length and spacing distributions are separate. Errors are
normalized by `r=14.5·scale` and `R=14.5·(scale_own+scale_enemy)`; time above
0.5R/1R is retained. Paired-head clearance sign is only a geometric diagnostic,
not the simulator's swept collision probe or an official hitbox claim.

World and camera-projected screen velocity, acceleration and jerk are computed
at the proper successive midpoint times, including irregular intervals. Each
head/body station has RMS/p95/max and residual derivatives versus continuous
truth; camera world/residual motion is separate. Packet-boundary screen steps
span the last frame before and first after a new admitted anchor, with a separate
truth-residual step. They are finite frame displacements, not instantaneous jump
measurements. Turns, boost and delivery jitter remain separate strata. Wrapped
heading steps, direction reversals, ack gaps, duplicates, skipped pictures and
longest stale-head runs are reported too.
Float32 painter quantization contributes to high-order derivatives. Completeness
means coverage and joins, not that the proposed positional safety targets pass;
their violations remain valid baseline results.

Response t10/t50/t90 use the paired position/heading change divided by the declared
step amplitude, relative to the first changed packet's admission. Every crossing
retains the last-below / first-above interval and a linear estimate inside it.
Missing response levels remain unresolved failures. Overshoot and all impulse
frames are retained. Response duration is not a steady-state positional delay.
First crossings, sustained crossings (no later fall below the level within the
one-second response window) and recrossing counts are separate. Some 170/240 Hz
heading impulses briefly show the full change before easing back: the production
angle offset refreshes on a positive integer animation advance. A near-zero
first t50 in that case is a transient, not immediate settling. The probe retains
both timings and does not change that production behavior.

## Controls and provenance

Unit controls verify lag sign/units, undefined stationary lag, ordered stations,
irregular derivatives, a known 40 ms delayed / 100 ms ramp (t50=90 ms), negative
steps and zero detector false positives. The production stationary no-anchor
control must have zero head error/correction and undefined lag. Production moving
straight controls compare 0 and 40 ms delivery: their **difference** must match
40 ms within one frame interval. Both absolute errors are retained. Packet
application before a frame update produces a frame-phase lead even with zero
delivery delay; this is observed behavior, not an offset removed from results.
A moving no-anchor pilot failed completeness when the unchanged body anchors
left camera culling bounds. That failed pilot is retained outside Git; it is not
used as a passing zero-correction control.

All distributions are descriptive observations of this deterministic tape, not
independent trials or confidence intervals. p99 with fewer than 1,000 samples is
explicitly descriptive only; larger frame counts still do not establish a
run-level confidence bound. Screen pixels, live servers, real network jitter,
GPU/display pacing, physical input and closed-loop reaction remain unverified.
Instrumentation overhead is not measured by these logical-clock experiments.
No candidate smoothing change or owner feel acceptance is evaluated here.

## Reproduce

From `client/`, on a clean committed source with Rust 1.99 and Node 24:

```powershell
$env:CARGO_BUILD_JOBS='2'
$env:CARGO_TARGET_DIR=(Join-Path $PWD 'target/wgpu-slice-check')
cargo build --release --locked -p slither-wgpu-slice --features feel-trace --example feel-replay
node tools/smoothing-lag.mjs --binary target/wgpu-slice-check/release/examples/feel-replay.exe --out C:/new/external/probe-run
```

The runner creates a fresh directory, keeps every raw baseline/impulse/control
output and stderr, and writes `summary.json` plus `baseline.md`. Failed cases stay
in the table and make the command fail. Its receipt pins source revision/tree,
binary SHA-256 and every scenario config; retain the explicit build command and
toolchain receipt too. It opens no network endpoint. Full outputs stay outside
Git. The small source-defined tests run with:

```sh
node --test tools/smoothing-lag.test.mjs
cargo test --locked -p slither-client --features feel-trace --lib feel_trace
cargo test --locked -p slither-wgpu-slice --features feel-trace --example feel-replay
```

## Baseline snapshot (2026-10-31)

Windows release CPU capture at `f9e8b743a311fe707ed87711e6ec4053fa76a543`, source tree
`5337fae22f4701106c341a885bc5a328f8dd7946`, binary SHA-256
`fe92c799a3d725d04ceb6737cfcd99f92d0e1ac3a9ed595d3be99c6732b59d81`. Analysis at
`f1693ca25a48783466ba7440143ce74b8d155b82` adds sustained crossings without changing
the retained raw captures or baseline metrics. All 48 baseline/impulse pairs and
three production controls completed: **112,632 retained frames**, two selected
snakes per picture, zero coverage failures. The complete earlier matrix at
`21af2859` is also retained; all case results and controls matched exactly
after the partial-failure export fix. This is deterministic reproducibility, not
independent trials.

Stationary zero-correction control: zero head error/correction, undefined lag.
Moving 0/40 ms controls measured a **41.1765 ms** shift in median lag-equivalent,
within a **5.8824 ms** frame-phase bound. Absolute zero-delay median was
**−3.8080 ms** and was not subtracted. Ten boost strata exceeded the proposed
p95 ≤0.25R engineering target: 170 Hz with jitter ±4/±12 ms, and 240 Hz at all
three jitter amplitudes, for both straight and turn. No stratum exceeded the
p99 ≤0.5R target. These are baseline outcomes, not safety acceptance.

All table values are milliseconds. Lag is signed p50/p95 against truth at
preparation. Position t50 is a paired step-response estimate after admission.
Heading cells show **first→sustained t50**, separately for own/enemy: ten strata
have a transient first t50 earlier than the sustained response. For example,
170 Hz normal/straight/no-jitter briefly reached 100% at +1.57 ms, dropped near
0% at +7.45 ms, then eased toward the changed heading. First-crossing estimates
clipped to zero are bounded by the retained receipt-to-next-frame interval;
they do not establish zero physical latency or immediate settling.

Own/enemy match at the shown precision because they follow translated copies
of the same scripted trajectory. Normal/boost, turning and jitter remain
separate. At 60 Hz and irregular cadence p99 uses fewer than 1,000 measured
frames and is descriptive only.

| Rate | Speed / motion | Jitter ±ms | Own lag p50/p95 ms | Enemy lag p50/p95 ms | Position t50 own/enemy ms | Heading first→sustained t50 own/enemy ms | Status |
|---|---|---:|---:|---:|---:|---:|---|
| 60 | normal/straight | 0 | 16.67 / 16.67 | 16.67 / 16.67 | 213.33 / 213.33 | 109.15→109.15 / 109.15→109.15 | complete |
| 60 | normal/straight | 4 | 9.84 / 16.65 | 9.84 / 16.65 | 200.67 / 200.67 | 105.15→105.15 / 105.15→105.15 | complete |
| 60 | normal/straight | 12 | 9.84 / 16.65 | 9.84 / 16.65 | 208.67 / 208.67 | 97.15→97.15 / 97.15→97.15 | complete |
| 60 | normal/turn | 0 | 16.67 / 16.67 | 16.67 / 16.67 | 213.42 / 213.42 | 109.15→109.15 / 109.15→109.15 | complete |
| 60 | normal/turn | 4 | 9.84 / 16.65 | 9.84 / 16.65 | 200.75 / 200.75 | 105.15→105.15 / 105.15→105.15 | complete |
| 60 | normal/turn | 12 | 9.84 / 16.65 | 9.84 / 16.65 | 208.75 / 208.75 | 97.15→97.15 / 97.15→97.15 | complete |
| 60 | boost/straight | 0 | 10.81 / 13.68 | 10.81 / 13.68 | 208.00 / 208.00 | 99.82→99.82 / 99.82→99.82 | complete |
| 60 | boost/straight | 4 | 11.34 / 16.03 | 11.34 / 16.03 | 212.00 / 212.00 | 103.82→103.82 / 103.82→103.82 | complete |
| 60 | boost/straight | 12 | 9.05 / 13.20 | 9.05 / 13.20 | 203.33 / 203.33 | 111.82→111.82 / 111.82→111.82 | complete |
| 60 | boost/turn | 0 | 10.68 / 13.36 | 10.68 / 13.36 | 207.12 / 207.12 | 99.82→99.82 / 99.82→99.82 | complete |
| 60 | boost/turn | 4 | 11.14 / 15.59 | 11.14 / 15.59 | 211.12 / 211.12 | 103.82→103.82 / 103.82→103.82 | complete |
| 60 | boost/turn | 12 | 8.73 / 12.66 | 8.73 / 12.66 | 202.46 / 202.46 | 111.82→111.82 / 111.82→111.82 | complete |
| 170 | normal/straight | 0 | 17.57 / 18.99 | 17.57 / 18.99 | 211.37 / 211.37 | 0.00→104.51 / 0.00→104.51 | complete |
| 170 | normal/straight | 4 | 17.18 / 21.86 | 17.18 / 21.86 | 215.37 / 215.37 | 100.51→100.51 / 100.51→100.51 | complete |
| 170 | normal/straight | 12 | 14.83 / 28.54 | 14.83 / 28.54 | 217.49 / 217.49 | 104.27→104.27 / 104.27→104.27 | complete |
| 170 | normal/turn | 0 | 17.56 / 18.97 | 17.56 / 18.97 | 211.43 / 211.43 | 0.00→104.51 / 0.00→104.51 | complete |
| 170 | normal/turn | 4 | 17.15 / 21.65 | 17.15 / 21.65 | 215.43 / 215.43 | 100.51→100.51 / 100.51→100.51 | complete |
| 170 | normal/turn | 12 | 14.84 / 27.87 | 14.84 / 27.87 | 217.55 / 217.55 | 104.27→104.27 / 104.27→104.27 | complete |
| 170 | boost/straight | 0 | 17.15 / 17.77 | 17.15 / 17.77 | 213.88 / 213.88 | 106.94→106.94 / 106.94→106.94 | complete |
| 170 | boost/straight | 4 | 16.80 / 19.39 | 16.80 / 19.39 | 212.00 / 212.00 | 99.18→99.18 / 99.18→99.18 | complete |
| 170 | boost/straight | 12 | 15.57 / 19.69 | 15.57 / 19.69 | 208.24 / 208.24 | 1.29→107.18 / 1.29→107.18 | complete |
| 170 | boost/turn | 0 | 17.09 / 17.82 | 17.09 / 17.82 | 213.27 / 213.27 | 106.94→106.94 / 106.94→106.94 | complete |
| 170 | boost/turn | 4 | 16.67 / 19.12 | 16.67 / 19.12 | 205.50 / 205.50 | 99.18→99.18 / 99.18→99.18 | complete |
| 170 | boost/turn | 12 | 15.45 / 18.75 | 15.45 / 18.75 | 207.62 / 207.62 | 1.29→107.18 / 1.29→107.18 | complete |
| 240 | normal/straight | 0 | 16.67 / 16.67 | 16.67 / 16.67 | 209.17 / 209.17 | 0.00→107.08 / 0.00→107.08 | complete |
| 240 | normal/straight | 4 | 15.67 / 20.71 | 15.67 / 20.71 | 204.83 / 204.83 | 103.08→103.08 / 103.08→103.08 | complete |
| 240 | normal/straight | 12 | 13.37 / 28.60 | 13.37 / 28.60 | 208.67 / 208.67 | 103.42→103.42 / 103.42→103.42 | complete |
| 240 | normal/turn | 0 | 16.67 / 16.67 | 16.67 / 16.67 | 213.38 / 213.38 | 0.00→107.08 / 0.00→107.08 | complete |
| 240 | normal/turn | 4 | 15.66 / 20.20 | 15.66 / 20.20 | 209.04 / 209.04 | 103.08→103.08 / 103.08→103.08 | complete |
| 240 | normal/turn | 12 | 13.40 / 27.26 | 13.40 / 27.26 | 208.71 / 208.71 | 103.42→103.42 / 103.42→103.42 | complete |
| 240 | boost/straight | 0 | 17.51 / 19.42 | 17.51 / 19.42 | 216.33 / 216.33 | 106.08→106.08 / 106.08→106.08 | complete |
| 240 | boost/straight | 4 | 17.62 / 19.53 | 17.62 / 19.53 | 216.17 / 216.17 | 1.75→110.08 / 1.75→110.08 | complete |
| 240 | boost/straight | 12 | 16.24 / 20.30 | 16.24 / 20.30 | 215.83 / 215.83 | 1.42→109.75 / 1.42→109.75 | complete |
| 240 | boost/turn | 0 | 17.51 / 19.37 | 17.51 / 19.37 | 211.73 / 211.73 | 106.08→106.08 / 106.08→106.08 | complete |
| 240 | boost/turn | 4 | 17.48 / 19.33 | 17.48 / 19.33 | 215.73 / 215.73 | 1.75→110.08 / 1.75→110.08 | complete |
| 240 | boost/turn | 12 | 16.10 / 19.25 | 16.10 / 19.25 | 215.40 / 215.40 | 1.42→109.75 / 1.42→109.75 | complete |
| irregular | normal/straight | 0 | 12.50 / 16.61 | 12.50 / 16.61 | 196.17 / 196.17 | 98.56→98.56 / 98.56→98.56 | complete |
| irregular | normal/straight | 4 | 12.60 / 19.63 | 12.60 / 19.63 | 200.17 / 200.17 | 101.83→101.83 / 101.83→101.83 | complete |
| irregular | normal/straight | 12 | 11.20 / 23.14 | 11.20 / 23.14 | 208.17 / 208.17 | 93.83→93.83 / 93.83→93.83 | complete |
| irregular | normal/turn | 0 | 12.56 / 16.26 | 12.56 / 16.26 | 196.28 / 196.28 | 98.56→98.56 / 98.56→98.56 | complete |
| irregular | normal/turn | 4 | 12.76 / 19.21 | 12.76 / 19.21 | 200.28 / 200.28 | 101.83→101.83 / 101.83→101.83 | complete |
| irregular | normal/turn | 12 | 11.00 / 22.83 | 11.00 / 22.83 | 208.28 / 208.28 | 93.83→93.83 / 93.83→93.83 | complete |
| irregular | boost/straight | 0 | 11.57 / 14.24 | 11.57 / 14.24 | 195.00 / 195.00 | 96.50→96.50 / 96.50→96.50 | complete |
| irregular | boost/straight | 4 | 13.56 / 16.91 | 13.56 / 16.91 | 199.00 / 199.00 | 100.50→100.50 / 100.50→100.50 | complete |
| irregular | boost/straight | 12 | 9.43 / 16.08 | 9.43 / 16.08 | 207.00 / 207.00 | 101.23→101.23 / 101.23→101.23 | complete |
| irregular | boost/turn | 0 | 11.43 / 14.26 | 11.43 / 14.26 | 194.27 / 194.27 | 96.50→96.50 / 96.50→96.50 | complete |
| irregular | boost/turn | 4 | 13.30 / 16.58 | 13.30 / 16.58 | 198.27 / 198.27 | 100.50→100.50 / 100.50→100.50 | complete |
| irregular | boost/turn | 12 | 9.16 / 15.21 | 9.16 / 15.21 | 206.27 / 206.27 | 101.23→101.23 / 101.23→101.23 | complete |

Full per-stratum spatial/body/age/smoothness distributions, t10/t50/t90 brackets,
recrossings, raw frames, failed pilots and both complete matrices remain outside
Git. This analysis summary SHA-256 is
`f3a1c66f0474b1731acbc5fde65f7879237d5f7172bb6890ee6cbf10bb52d113`; its manifest hashes all 99 raw case/control
files. No baseline frame was trimmed or aligned.
