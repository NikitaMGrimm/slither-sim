# NTL motion under Battledome visibility caps

## Finding

On this one-core harness, native 1500 is the strongest tested normal-zoom case:
both repetitions have correction p95 about 2.5 units and draw-gap p99 about 22 ms.
Larger caps have excess corrections and substantial between-replay variation.
Whole-food **small fixed-cohort diagnostics**, including the cosmetic ring,
have near-reference draw gaps, but correction p99 does not consistently match.
That supports investigating split food/snake visibility without establishing
correction-tail parity or validating a whole-food/dynamic-1500 policy. No runtime change.

## Pins and method

Measure the requested candidate `feat/battledome-arena-vision` at
`a979b10cbbd0efd97690d4048b21b487e328c3af`, unchanged release host/adapter.
It branches from `20e5064` and predates minimap fix #196 and socket-wake fix #197;
no backports. This is not qualification of current main or its minimap.
Seed 7, plain Battledome, 64 bots, two remote seats, bots.threads=1,
bots.vision=1500. One inherited logical-CPU affinity bit, jobs/Rayon/test threads 1;
one heavy experiment at a time on a shared Windows desktop. Warm up 90 seconds,
record a stock Forager observer for 120 seconds per setting. Retain every life;
select the longest uninterrupted life, capped at 90 seconds, before replay.
Same seed/config is not identical gameplay: the observer sees different peers.

As in [#203's study](../2026-10-27-legacy-motion/REPORT.md), derive 13 offline
through the pinned production adapter from recorded 19; preserve receipt timing,
frame boundaries and elapsed allocation. This is not a direct legacy socket capture.
The numeric settings use the same normal food stencil; arena delivers all live-wall
food sectors. The primary portable observer requests physical food, excluding
cosmetic ring pellets. Host bots keep their local vision/food stencil in every run.

Replay retained [official 13](../2026-10-26-protocol13-catalog/REPORT.md), first
100 seconds, and each candidate twice through real unmodified NTL 9.78 in sandboxed
bundled Chromium. Loopback page, fail-all external DNS, deny proxy and HTTP allowlist;
no new outside-server sessions. Viewport 1100×800, DPR 1, FPS 60, normal zoom.
The native relay blocks on a per-thread timer, preserves original timing and logs
lateness; no busy wait or global timer change. All 20 attempts pass the predeclared
p99 ≤2 ms relay gate (maximum 0.930 ms), with zero page exceptions; no retries.
Native visibility repeats run in reverse order; official brackets the sweep.

Read-only wrappers sample the actual renderer and correction changes, as before.
Select the largest visible living peer; primary overlap is 100–169 segments,
speed <9, same object and live own snake at adjacent endpoints. Offset innovation
is the change to fx/fy across a relevant message, **not a displayed teleport**.
Quantiles are nearest rank. Frames/messages correlate; repeat replays measure
browser variation, not independent simulations. There is no formal equivalence gate.

## Native sweep (both repetitions)

Primary intervals retain the inherited `0 < draw gap <500 ms` rule. The next
section exposes what that censors. Visible includes drawn death-fades; cached
and body-point totals include fade caches. Workload p50/p95 is per live-own draw
snapshot, not time-weighted or a wire census. Body points include retained/fading
anchors and can balloon during stalls. Sparse correction tails are unstable.

| Setting / replay | Offset p95 / p99 units | Draw-gap p95 / p99 ms (<500) | Visible p50 / p95 | Cached p50 | Body points p50 / p95 | Exposure s / corrections |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| official · 1 | 6.24 / 8.23 | 20.8 / 42.5 | 5 / 8 | 13 | 1089 / 1586 | 24.9 / 122 |
| official · 2 | 4.39 / 5.23 | 18.1 / 26.2 | 5 / 8 | 13 | 1076 / 1585 | 24.7 / 121 |
| arena · 1 | 42.30 / 42.30 | 56.0 / 430.7 | 11 / 13 | 65 | 15013 / 15271 | 5.9 / 16 |
| arena · 2 | 24.37 / 26.32 | 18.2 / 39.3 | 8 / 12 | 65 | 5155 / 6611 | 25.1 / 115 |
| 6000 · 1 | 32.77 / 32.77 | 298.9 / 328.4 | 8 / 11 | 65 | 5905 / 10297 | 4.1 / 17 |
| 6000 · 2 | 6.64 / 18.05 | 33.8 / 46.2 | 6 / 10 | 65 | 4989 / 5424 | 13.4 / 62 |
| 4500 · 1 | 33.74 / 40.53 | 80.3 / 139.4 | 11 / 15 | 63 | 4817 / 7678 | 25.2 / 107 |
| 4500 · 2 | 25.22 / 42.06 | 27.2 / 79.4 | 8 / 15 | 64 | 5050 / 6850 | 31.0 / 155 |
| 3000 · 1 | 42.01 / 47.14 | 23.1 / 106.4 | 10 / 15 | 46 | 3452 / 4624 | 32.7 / 147 |
| 3000 · 2 | 18.39 / 33.69 | 20.0 / 53.9 | 9 / 14 | 49 | 4055 / 4711 | 55.7 / 269 |
| 1500 · 1 | 2.54 / 4.39 | 18.9 / 22.4 | 8 / 11 | 16 | 1147 / 1680 | 16.3 / 72 |
| 1500 · 2 | 2.57 / 2.98 | 18.1 / 21.6 | 8 / 11 | 16 | 1145 / 1681 | 16.3 / 72 |

## Censoring audit

After the first arena replay exposed multi-second stalls, add an explicitly
post-hoc audit without the 500 ms ceiling, leaving the primary results intact.
Rows below have ≥500 ms same-peer overlap gaps; other native/reference repetitions
have none in that stratum. Uncapped all-large (≥100) and all-live draw gaps,
maxima, counts, size/speed mixtures and both versions are in [results.json](results.json).
The repeats' wide spread must not be pooled into a parity or monotonic-cap claim.

| Replay | Gaps ≥500 ms / max ms | Uncapped offset p95 / p99 | Uncapped draw-gap p95 / p99 ms | Uncapped exposure s / corrections |
| --- | ---: | ---: | ---: | ---: |
| arena · 1 | 19 / 1559.1 | 43.54 / 59.09 | 731.1 / 1234.9 | 21.9 / 81 |
| 6000 · 1 | 2 / 867.3 | 41.34 / 41.68 | 328.4 / 867.3 | 5.8 / 23 |
| 4500 · 1 | 2 / 533.3 | 33.94 / 40.53 | 82.4 / 143.2 | 26.3 / 110 |
| 4500 · 2 | 5 / 1008.3 | 42.05 / 42.43 | 27.7 / 102.2 | 34.8 / 173 |
| 3000 · 1 | 9 / 2962.1 | 42.45 / 59.36 | 24.6 / 145.3 | 49.0 / 211 |
| 3000 · 2 | 1 / 838.6 | 18.39 / 33.69 | 20.0 / 56.4 | 56.5 / 269 |

## Whole-food diagnostics

The branch cannot configure arena food separately from snake range. First retain
the whole-window union of identity-epochs whose decoded centerline comes within
1500+84 of own decoded head/front; keep whole epochs to avoid invented appearances.
The physical-food control retains 50 IDs and is too conservative to model native
1500; its corrections remain elevated. The ordinary-food union has no eligible
large-peer exposure, so it cannot establish motion parity. Both are retained.

Add an exploratory bounded control, as in #203: own plus five peer identity-epochs
with the most large-snake (≥100 anchors) appearance/anchor checkpoints within
1500+84. Select by independently decoded geometry before its render measurements,
not correction values. Preserve every retained snake byte/time and every original
food, minimap, HUD, own and pong byte/time. An independent serialized-output audit
checks exact subsequences, whole retained epochs and unchanged non-snake traffic.
This is a fixed small scene, **not** exact dynamic range culling or a count-cap proposal.

For the ordinary-food supplement, a loopback HTTP shim removes only the physical
food opt-in header; all host WebSocket bytes are recorded unchanged, including ring.
The first ordinary observer followed cosmetic food and missed large peers. The
supplement's decision-only copy knows the configured pit ratio 1784/3584 and ignores
food outside pit−10; the connection remains an ordinary browser profile. Same seed,
warmup/duration/selection, then the same five-peer filter. Trajectory differs.

| Setting / replay | Offset p95 / p99 units | Draw-gap p95 / p99 ms (<500) | Visible p50 / p95 | Cached p50 | Body points p50 / p95 | Food p50 | Exposure s / corrections |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Physical food · 1 | 2.89 / 8.02 | 18.2 / 31.5 | 2 / 3 | 6 | 760 / 836 | 1249 | 23.0 / 104 |
| Physical food · 2 | 7.86 / 15.38 | 18.5 / 30.7 | 2 / 3 | 6 | 760 / 837 | 1247 | 23.0 / 104 |
| Food + ring · 1 | 3.38 / 4.89 | 18.5 / 32.1 | 3 / 5 | 6 | 726 / 815 | 1883 | 64.4 / 298 |
| Food + ring · 2 | 3.77 / 25.19 | 18.6 / 30.3 | 3 / 5 | 6 | 727 / 815 | 1882 | 63.7 / 296 |

Draw gaps remain near the reference, with useful large-peer exposure. Correction
p99 does not reproduce reference levels: second physical/ring runs reach 15.38/25.19
versus reference 5.23–8.23. Ring p95 stays 3.38–3.77; physical p95 varies 2.89–7.86.
Thus whole-food correction-tail parity is **inconclusive**, not a pooled pass.
The scene reduction supports a workload-sensitive excess from many snake caches,
but isolates neither every renderer/OS/GPU stall nor exact dynamic-1500 behavior.

## Zoom coverage: client-derived, browser-confirmed

The statically inspected and browser-loaded main script have the same SHA-256
`f7ecd6f9b577dd2343c8d390c1570deb54708ffe138ca7bdd599f58d260f0eb8`.
Default zoom mod holds starting scale 16.2/14 independently of snake size; slow
release is off. Native zoom-out keys and wheel clamp scale to 0.07 (zoom-in 4).
Post-measurement native key presses confirm 0.07; zoom probes are excluded above.
With zoom mod off/slow release on, automatic target is
`0.64285 + 0.514285714 / max(1,(segments+16)/36)`, approached by 0.0002 per render.

Measured canvas 1456×1059, centered CSS scale 0.76542965; viewport clips its edges.
Visible half-extent is `viewport / (2 × CSS scale × camera scale)`, not background
image size. NTL's snake draw-culling rectangle adds 84 world units per axis.

| View | Scale | Visible half-width / half-height | Viewport corner radius |
| --- | ---: | ---: | ---: |
| Normal | 1.157143 | 621.0 / 451.6 | 767.8 |
| Maximum zoom-out | 0.07 | 10265.0 / 7465.5 | 12692.7 |

| Snake radius | Minimum scale covering nominal viewport corners |
| ---: | ---: |
| 1500 | 0.592324 |
| 3000 | 0.296162 |
| 4500 | 0.197441 |
| 6000 | 0.148081 |

Thus all tested numeric caps cover the normal centered view; the normal padded
draw-cull corner is about 895.5. None covers the full maximum-zoom rectangle.
The arena itself clips that rectangle: 6000 can cover its roughly 5100-unit disc
from the center but misses far-side snakes from an off-center player. Coverage
is to any body/head segment, not just peer heads. Camera correction/offset,
custom zoom/HD settings and body thickness prevent treating this as a universal
visibility guarantee. Maximum-zoom performance was not measured.

## Retention and limits

[results.json](results.json) pins every raw stock/derived source, selected life,
payload, render observation, relay log, plan/amendment, build and analysis tool.
Full data/scripts and all weak controls are local-only in durable scratch
`ntl-vision-sweep-20261027`; official input remains in the catalog scratch archive.
No raw names, addresses, third-party code/assets or screenshots are published.
Hashes identify material, not truth or public access. Read-only focus metadata
was added during the second half; logged measurements remain visible/focused.
Primary cap and supplemental-control amendments are retained with their timing.

Reproduction with private access: verify hashes; use the one-core wrapper for
capture_sweep, ordinary capture and run_replays; then run_supplements, serialized
control audit, cohort summary and selected export. The report preserves all
planned settings, repetitions and inconclusive controls. The shared desktop and
browser/compositor scheduling are not isolated; two replays cannot calibrate a
stable performance limit. No production host, encoder, contract, bot policy or pacing change.
Choose the normal-view versus extreme-zoom coverage trade-off before adopting a cap.
