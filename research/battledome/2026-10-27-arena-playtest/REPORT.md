# Arena playtest v1 — 2026-10-27

This is a playable first version for owner feedback, not an adopted arena design
or a completed safety/capacity study. No redesign PR is open. The owner deferred
remaining directional seeds and held-out confirmation for local playtesting.

## Current behavior

Wall target: 3,584 through 32 seats, then `3584 * sqrt(seats / 32)`:
5,068.54 at 64 and 5,666.80 at 80. Pit/spawn proportions follow the live wall.
All queue-age growth allowances are removed. Existing immediate growth, gradual
shrink and 3% dead zone remain. Fresh 16–32-seat geometry, floor, pit and scoring
rules remain; normal spawn-to-pit travel is still about 4.9–8.8 seconds.
Area per seat is constant above 32, rather than decreasing crowd density.

The owner subsequently liked a separately played 64-bot setup with wall 5,069,
pit 2,523 and spawn band 3,796–4,808. That matches this policy's 64-seat geometry
and supports constant density over the slower fixed-size screen. That setup
also used separately owned pit-only food; this branch does not yet include it.
This is a feel signal, not independent safety or capacity validation.

Preferred admission retains 400 body, 600/2s forward-head exclusion and the 3s
head halo/departure rules. Every selected pose also reserves outside heads'
3s inward routes at 300 clearance. After 2s waiting, at most the first 256 draws
can qualify for a directional floor: 200 body/head clearance, 300/1s forward
paths, and 300/3s normal-speed inward runway. Contact+40 always dominates.
If no strict pose passes 4,096 draws, choose the eligible pose maximizing its
minimum body-floor margin. Cache bounds prune losing scores. One search/tick,
14ms failed retry and socket-before-bot priority remain.

The fallback protects travel toward the middle. It does not promise arbitrary
future turns or boosted departures, and is not collision immunity. Search
work is bounded; worst-case waiting is not guaranteed. The current simultaneous
80-bot fill still reaches about 10s maximum; this is disclosed for playtesting,
not presented as the final short-wait solution.

## Measurements so far

Offline synthetic host lifecycle: real Simulation/Seat commands, framed
delivery, shared pit-aware A4 and respawns, driven in a deterministic virtual
schedule. 180 world seconds/run; strict screens use seeds 7/29/47. Subsequent
directional pilots use seed 7 only, pending owner direction before a larger study.
Release/thin LTO, locked dependencies, two-CPU affinity verified, build jobs 1,
Rayon/test threads 2. No outside servers or owner play-server access.
Virtual wait seconds are not wall-clock/socket capacity results.

| Policy | Seats | Wait p50,s | Wait p95,s (seed range) | Wait max,s | Deaths<=3s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Strict fixed | 32 | 0.00–0.00 | 7.02–8.56 | 10.54 | 0 |
| Strict fixed | 64 | 0.00–0.00 | 18.56–19.66 | 26.06 | 0 |
| Strict fixed | 80 | 1.08–1.74 | 23.34–25.22 | 34.00 | 0 |
| Strict density | 32 | 0.00–0.00 | 7.02–8.56 | 10.54 | 0 |
| Strict density | 64 | 0.00–0.00 | 14.35–15.21 | 17.30 | 0 |
| Strict density | 80 | 0.00–0.00 | 15.29–16.70 | 18.67 | 0 |
| Playtest directional | 32 | 0.00 | 3.89 | 4.94 | 0 |
| Playtest directional | 64 | 0.00 | 9.26 | 9.32 | 0 |
| Playtest directional | 80 | 0.00 | 9.88 | 9.96 | 0 |

All strict screens had zero strict head-path intrusions. The three selected
directional pilots had 11/24/27 strict 600/2 intrusions at 32/64/80 respectively,
while observing zero deaths<=3s, zero first-3s sampled near-miss lives and zero
birth-time inward-route intrusions. Complete 3s-followup births: 91/157/190.
Sampled minimum clearance was at least 200 units, capped at 200 for reporting.
Near-miss threshold: 100 units beyond physical contact; samples every 42ms,
both head/body directions between each young snake and every living opponent.
Sampling does not prove continuous clearance or human comfort. Late lives are
censored; raw early events are retained. Final pending queues were zero in
selected pilots. Strict fixed screens ended with up to two pending requests at 80.

Pilot admission elapsed ms/tick: 0.064/0.085/0.100 at 32/64/80. Virtual busy
ratios were 4.71%/6.17% at 64/80. These include diagnostic overhead and shared
scheduling. They are descriptive fixture timings, not isolated CPU, real-time
host load, throughput or a qualified speedup claim. Stable pilots do not qualify
a long 80-to-32 shrink; final-source shrink load remains unmeasured. No
eight-minute real-time startup claim is made.

## Attempts and provenance

Full fixtures, source snapshots, copied binaries, hashes, plans, per-tick
outputs and failed attempts remain local-only in outputs/pit-redesign.
Selected results retain every run digest. Source-pins identify local-only frozen
source manifests, binaries and plans; hashes do not make them downloadable or
validate a claim. The strict screens used the earlier pit-aware source before
later main food-sharing/presentation merges and the full-arc bot correction; the
directional v2 pilots include #180 and the #181 hard-arc correction. They are
distinct cohorts; none is relabeled as a later-source run. Subsequent main host
changes are integrated on the playable branch; these rows do not qualify that
final source. No personal or machine data is needed to identify the artifacts.

The first fixture attempt omitted the real startup Seat::join call, admitted
no bots and failed on empty wait quantiles. It contributes no gameplay or timing
result. The corrected fixture checks nonempty births. Full failed artifacts and
the superseded startup investigation are retained locally.

The first fixed maximin pilot (32/64/80, seed 7) had no early deaths/near-misses
but waited up to 7.32/17.97/24.37s; one 64-seat strict birth cut an inward route.
It is rejected. A failing-before regression reproduced the strict bypass;
the common cache now guards inward routes for both tiers. A second fixed pilot
reduced maximum waits to 4.94/12.86/17.41s, still too slow. The current playtest
uses density at 64/80 with that corrected directional policy. No compact-body,
immunity or extra-growth design was adopted. Planned remaining directional
seeds and held-out seeds 71/101 have not run; no adoption gate is claimed.

Incremental wall culling is also included in this branch and is being split
into its own infrastructure PR. It mirrors physical wall removals into the food
index and presentation before births/pickups, retaining unchanged sector handles
and hidden ambient births. Focused regressions cover real wall movement and food
diffs. The main food-generation and ecology_at_radius helpers are untouched.

Rules revision is v5 on this play branch, reserved after the separately owned
pit-only-food v4 change; no checkpoint shape changed. Reconcile the two branches
before any future integration. This branch currently uses main's food rules.

## Local play config

No special world overrides are required. For 64 background bots:

```json
{"players":{"max":8},"bots":{"count":64,"threads":2},"simulation":{"world":{"preset":"battledome"},"delivery":"responsive"}}
```

Set bots.count to 32 or 80 for the other sizes. Occupied/queued human seats also
count, so those bot counts assume no connected players. Choose a separate bind;
study smoke tests use 127.0.0.1:18876. The owner controls the local play server.

## Validation of this playable source

Formatting and all five Clippy configurations passed. All-target tests passed
with default, core-only, bot-only and networking features, plus both doctest
configurations, C ABI tests, the Python-crate check and the basic example.
All seven documentation checks passed. The final sequential checks and release
build used one-core affinity, build jobs 1, Rayon 1 and test threads 1 after the
resource correction; the earlier measured cohorts above used two CPUs.

Two separate 21-second CLI smoke runs used default seed and the config above,
64 then 80 bots, on loopback 18876, one core. Four five-second stats samples:
64 bots: load 14.8/37.9/4.4/4.8%; 80 bots: 35.4/32.9/4.2/5.0%.
Every interval had zero deferred ticks and about 142.7–143.0 ticks/s.
This confirms CLI startup, not alive-seat counts, long-run load or shrink cost.
Binary/log hashes are in source-pins; full logs remain local-only.
