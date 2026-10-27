# Constant-density Battledome admission — 2026-10-27

The owner approved build `82c46fe` after playing the plain Battledome preset
with 64 bots, eight player slots and responsive delivery. This PR preserves
that behavior on main `ac85528`. It is not a completed capacity/safety study.

## Player behavior

Wall target: `3584 * sqrt(max(seats, 32) / 32)`:

| Seats | Wall | Pit | Spawn band |
| ---: | ---: | ---: | ---: |
| 32 | 3,584 | 1,784 | 2,684–3,400 |
| 64 | 5,069 | 2,523 | 3,796–4,808 |
| 80 | 5,667 | 2,821 | 4,244–5,376 |

Wall, pit and spawn band scale together. Area per seat stays constant above 32;
fresh 16–32-seat geometry and normal spawn-to-pit travel (4.9–8.8s) are unchanged.
The 64-seat geometry matches the separately approved fixed-radius play setup.
No queue-age growth allowances remain. Existing immediate growth, 23.4375-unit/s
shrink and 3% dead zone remain. The mass floor, boost, pit and kill rules are
those on main, including #187's pit-only food. No new player mechanic is added.

Preferred admission retains 400-unit body clearance, 600-unit/two-second
forward-head exclusion, the three-second head halo and inward/tangent runways.
Every selected pose also leaves outside heads' three-second inward routes
clear by `max(contact+40, 300)`.

After two world seconds waiting, at most the first 256 of the 4,096 draws can
also qualify for a directional fallback: body/head clearance at least 200,
forward-head paths 300 over one second, and a three-second normal-speed inward
runway at 300 clearance. Contact+40 always dominates. If no preferred pose
passes, choose the eligible pose with greatest minimum body-floor margin.
Ordering/ties stay deterministic. One search per tick, 14ms failed retry and
socket-before-bot priority remain. Work is bounded; worst-case wait is not.
This reserves modeled inward travel, not arbitrary future turns or boosts.

Arena sizing advances rules-v4 to `slither-sim-rules-v5`, rejecting v1–v4
continuations. Checkpoint format stays v2; no saved fields are added. Queue
ages remain host state; checkpoints retain the live radius.

## Retained measurements

Offline synthetic host lifecycle: real Simulation/Seat commands, framed
delivery, shared pit-aware A4 and respawns on a deterministic virtual schedule.
180 world seconds/run, serial release/thin-LTO builds with locked dependencies
and at most two CPUs. Strict screens use seeds 7/29/47; directional pilots use
seed 7. World-time waits and elapsed fixture timings do not qualify sockets.

| Policy | Seats | Wait p50,s (seed range) | Wait p95,s (seed range) | Max,s | Deaths ≤3s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Strict fixed | 32 | 0–0 | 7.02–8.56 | 10.54 | 0 |
| Strict fixed | 64 | 0–0 | 18.56–19.66 | 26.06 | 0 |
| Strict fixed | 80 | 1.08–1.74 | 23.34–25.22 | 34.00 | 0 |
| Strict density | 32 | 0–0 | 7.02–8.56 | 10.54 | 0 |
| Strict density | 64 | 0–0 | 14.35–15.21 | 17.30 | 0 |
| Strict density | 80 | 0–0 | 15.29–16.70 | 18.67 | 0 |
| Directional pilot | 32 | 0 | 3.89 | 4.94 | 0 |
| Directional pilot | 64 | 0 | 9.26 | 9.32 | 0 |
| Directional pilot | 80 | 0 | 9.88 | 9.96 | 0 |

Strict screens had zero strict head-path intrusions. Directional pilots had
11/24/27 intrusions against the preferred 600/2s exclusion at 32/64/80, while
observing zero early deaths, sampled near-miss lives and birth-time inward-route
intrusions. Complete three-second follow-ups: 91/157/190 births. Near-misses mean
either head within 100 beyond physical contact of the other's body, sampled
every 42ms during the young life; minimum sampled clearance was at least the
200-unit reporting cap. Sampling does not prove continuous safety or human
dodge comfort. Late births are censored. Selected pilots ended with no waiting
requests; strict fixed screens ended with up to two pending at 80.

Admission elapsed ms/tick: 0.064/0.085/0.100 at 32/64/80. Virtual busy ratios
at 64/80 were 4.71%/6.17%. Diagnostic overhead/shared scheduling are included;
these are not isolated CPU times, a qualified speedup or a capacity result.
All gameplay rows predate #187. Strict screens also predate later food-sharing,
presentation and full-arc A4 fixes; directional v2 includes #180 and the #181 arc
fix. Source cohorts remain separate; none is relabeled as final-source data.

## Owner approval and short startup checks

The owner approved the full `82c46fe` preset build ("looks good"). The maintainer
reported a subsequent 75s check through the local relay using two stock headless
clients: wall 5,068, pit about 2,523, zero pellets outside the pit, no early
deaths (shortest life about 19s), `(bot)` names, 7.6–9.2% load and zero deferred
ticks. This is maintainer-provided evidence; raw relay captures were not supplied
to this study. It supports the 64-seat playtest, not 80-seat capacity or long-run
safety. Remaining directional/held-out runs were deferred for owner playtesting.

Historical CLI smoke runs were serial, 21s each, default seed, 64 then 80 bots,
responsive Battledome with eight player slots, loopback 18876 and one-core
affinity. Four five-second load samples before food integration (`56c8bd1`):
64: 14.8/37.9/4.4/4.8%; 80: 35.4/32.9/4.2/5.0%. After food (`82c46fe`):
64: 28.4/11.0/2.9/3.6%; 80: 36.3/33.3/4.0/8.8%. All intervals had zero deferred
ticks, about 142.7–143.0 ticks/s. These confirm startup, not alive-seat counts.
Long 80→32 shrink load and eight-minute startup performance remain unmeasured.

## Rebase and cache regressions

`56cc183`, `0a6568c` and `0485107` are dropped: #189 (`c115e05`) already contains
their complete moving-wall/pit food cull, tests and release note. In particular,
`0485107` adds no behavior beyond merged #189. #188's subscription union cache
is preserved. It caches membership, while edge culls independently dirty food
listings. Strengthened wall/pit regressions hold both membership identities
constant, require the edge listing to refresh and the quiet handle to survive,
and verify decoded removals for both clients. A negative control retaining
dirty cached listings fails both tests; production cache logic needs no change.

The current full CONTRIBUTING Rust group and seven docs checks pass, serialized
with build jobs 1, Rayon 1, test threads 1 and one-core affinity. Approved-build
and rebased binaries produce byte-identical complete trace windows: portable
and protocol (32 operations), official-500 (one decision), Battledome-64 (four),
and retained step_all/next_frame (384 each, including reconnect/decoder replay).
Harness bytes match. No timing comparison is inferred from these traces.

## Failed attempts and provenance

The initial fixture omitted Seat::join, admitted nobody and failed on empty
wait quantiles; it supplies no timing/gameplay result. The first fixed maximin
pilot (32/64/80, seed 7) had no early deaths/near-misses but maximum waits
7.32/17.97/24.37s and one 64-seat strict birth cutting an inward route. Rejected:
a failing-before regression exposed that strict bypass; both tiers now guard
inward routes. The corrected fixed pilot still reached 4.94/12.86/17.41s, so the
playtest uses density at 64/80. No immunity, compact body or extra-growth design
was adopted. Seeds 71/101 and the remaining directional seeds have not run;
no completed multi-seed adoption gate is claimed. The superseded startup study
and all failed/rejected outputs are retained locally.

`results.json` retains all 26 summaries and full-result digests. Full fixtures,
frozen plans, source snapshots, binaries, attempts and traces remain local-only
under outputs/pit-redesign and outputs/arena-pr. `source-pins.json` preserves
historical source/binary/plan hashes and identifies the approved build and PR
validation sources. Hashes identify artifacts; they do not make them publicly
downloadable or validate empirical claims. No outside game servers were used.
