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

Response t10/t50/t90 use the paired position/heading change divided by the declared
step amplitude, relative to the first changed packet's admission. Every crossing
retains the last-below / first-above interval and a linear estimate inside it.
Missing response levels remain unresolved failures. Overshoot and all impulse
frames are retained. Response duration is not a steady-state positional delay.

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
