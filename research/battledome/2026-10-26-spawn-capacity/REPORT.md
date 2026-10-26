# Crowded Battledome admission, 2026-10-26

Strict growth (B) fills the measured 40/64/80 seats with existing safety checks; fixed relaxation (A) is rejected for intrusions/queues.
This synthetic **local-simulator** comparison measures neither official rules nor a community Battledome server.

## Policy

Through 32 seats retain the 3,584 wall, 1,784 pit and 2,684–3,400 spawn band.
Target `3584 * (1 + 1.5 * (max(seats,32)/32 - 1))`: 4,928/8,960/11,648 at 40/64/80.
Occupied/queued seats count before search; selected-request ages 2/4 world seconds permit 1.5/2 times target.
Keep immediate growth, 23.4375 units/s shrink and the 3% dead zone. Further growth
waits for the latest birth's three-second departure window; admissions pause with no search work,
so repeated births cannot defer growth. Pit/spawn radii and ecology follow the live wall; snakes stay put.
Keep 400-unit body clearance, 600-unit/two-second head paths, three-second normal head runway
and inward/tangent departures; contact + 40 remains a lower bound. One 4,096-candidate
search/tick, 14 ms after failure; sockets precede FIFO bots.
Engulfment is not entry; later physical entry or birth inside commits the one-way pit. No immunity.
Existing entry bit/live radius suffice; queue age/latest-birth tick are transient host state.
Checkpoint format stays v2; rules v3 rejects v1/v2. Live-radius mapping fixes pit cells and own-marker alignment;
decorative food follows the edge. A tried body 400/300/200, head horizon 1.5/1/0.5 s,
path 600/400/200 and departure 400/300/200 after 2/3/4 s, retaining contact/runway;
the best relaxed body margin followed strict exhaustion. None of those tiers is adopted.

## Method and measurement scope

[PLAN.md](PLAN.md) is unchanged. Rust 1.99.0, locked release/thin LTO, shared Windows
x86-64; four real loopback probes plus 36/60/76 A4 bots, one bot worker, no latency.
Probes decide/42 ms, ping/250 ms, stay alive five seconds, close/100 ms and rejoin.
Final strict cohort: 30 sequential 60-wall-second holds, seeds 7/29/47/71/101,
40/64/80 seats, alternating control/growth order; six rejected A holds use seeds 7/29 (two/seat).
Their fixed-pit path survives later moving-pit fixes.
Every selected parent/child mask was verified 0x30, with PowerShell spot checks.
Build/check masks 0xC0, CARGO_BUILD_JOBS=2, RAYON_NUM_THREADS=2, RUST_TEST_THREADS=2.
Earlier search-only holds overlapped checks; final strict holds followed the Rust group.
Shared frequency/cache/scheduling noise remains; all seeds had been seen, so this
is adaptive corrective exploration, without independent held-out evidence.
Queue waits use world time; sockets use wall time through first own appearance/handshake.
Triples are p50/p95/max, index `round((N-1)*p)` over completed requests; pending requests remain right-censored.
Early-death denominators include complete 3 s followup or deaths within 3 s;
early administrative close/shutdown is censored. Late living uses world seconds 30–60.
Intrusions test newborn polylines against living heads' strict 600/2 paths.
GetThreadTimes times whole admit_waiting (queue/growth/connect/search and nested telemetry),
plus nested searches; socket intake/broadcast are excluded. Counters accumulate without I/O and emit after ticks.
Strict costs use completed consecutive tick records; A/host busy use pre-stop counters that can omit closeout ticks.
Host busy includes telemetry/scheduling, not process CPU. Zero/near-zero CPU is below useful resolution,
not zero cost; admission elapsed/tick is a conservative true CPU ceiling. A's two search-only holds/seat
do not rank policies. [results.json](results.json) keeps 36 run summaries and pooled table values, with declared columns/units.

## Selected outcomes

Five holds/cell for strict policies, two for A. Pending counts sum shutdowns;
early deaths are deaths/qualified lives, intrusions are hits/all births.

| Seats / policy | Runs | Mean late living | Completed queue wait | Pending | Deaths ≤3 s | Strict intrusions |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| 40 / Main strict fixed | 5 | 30.53 | 1.960 / 30.982 / 42.959 | 42 | 0/443 | 0/474 |
| 40 / A: fixed relaxation | 2 | 34.45 | 3.038 / 11.991 / 14.224 | 7 | 0/216 | 101/228 |
| 40 / B: strict growth | 5 | 39.89 | 0.007 / 4.774 / 4.963 | 0 | 0/431 | 0/432 |
| 64 / Main strict fixed | 5 | 31.22 | 2.408 / 54.285 / 60.032 | 166 | 0/466 | 0/501 |
| 64 / A: fixed relaxation | 2 | 46.72 | 8.218 / 33.257 / 40.985 | 33 | 0/229 | 138/243 |
| 64 / B: strict growth | 5 | 63.89 | 0.042 / 1.162 / 4.977 | 0 | 0/577 | 0/579 |
| 80 / Main strict fixed | 5 | 31.45 | 2.520 / 56.035 / 60.025 | 235 | 0/435 | 0/465 |
| 80 / A: fixed relaxation | 2 | 44.52 | 8.456 / 49.266 / 60.018 | 70 | 0/226 | 127/236 |
| 80 / B: strict growth | 5 | 79.92 | 0.105 / 0.511 / 0.567 | 0 | 0/643 | 0/646 |

| Seats / policy | Initial bot join | Bot respawn | Socket wall wait, all joins |
| --- | --- | --- | --- |
| 40 / Main strict fixed | 11.907 / 37.849 / 42.959 | 13.202 / 24.542 / 30.758 | 0.571 / 3.032 / 5.383 |
| 40 / A: fixed relaxation | 4.018 / 13.209 / 14.224 | 4.011 / 5.824 / 7.434 | 1.420 / 3.448 / 4.258 |
| 40 / B: strict growth | 0.735 / 4.900 / 4.963 | 0.000 / 0.000 / 0.000 | 0.006 / 0.016 / 0.030 |
| 64 / Main strict fixed | 24.227 / 58.800 / 60.032 | 45.199 / 47.187 / 47.187 | 0.551 / 2.970 / 5.003 |
| 64 / A: fixed relaxation | 11.515 / 34.237 / 40.985 | 15.659 / 25.508 / 27.300 | 2.284 / 4.025 / 4.028 |
| 64 / B: strict growth | 0.238 / 1.575 / 4.977 | 0.000 / 0.000 / 0.000 | 0.006 / 0.026 / 0.046 |
| 80 / Main strict fixed | 23.527 / 58.814 / 60.025 | — | 0.789 / 3.576 / 6.730 |
| 80 / A: fixed relaxation | 19.012 / 53.914 / 60.018 | 27.713 / 37.002 / 37.765 | 2.116 / 4.035 / 4.449 |
| 80 / B: strict growth | 0.294 / 0.532 / 0.567 | 0.000 / 0.000 / 0.000 | 0.005 / 0.018 / 0.029 |

No selected early death occurred. Missing respawn quantiles mean none admitted, not zero wait.
Rows retain 3 s censoring, maximum pending age, errors and deferrals. All 23 errors (13 control, 8 B, 2 A)
were “connection reset without closing handshake”; hosts exited successfully, with no deferred ticks.
Stopping hosts before joining readers makes shutdown plausible; missing timestamps prevent exact attribution.

| Seats / policy | Full admission CPU ms/tick median (range) | Search CPU ms/tick median (range) | Full admission elapsed ms/tick median | Host busy ms/tick median | Searches |
| --- | --- | --- | ---: | ---: | ---: |
| 40 / Main strict fixed | 1.7037 (1.6602–1.8211) | 1.6527 (1.6110–1.7628) | 2.0930 | 2.299 | 21679 |
| 40 / A: fixed relaxation | — | 2.5790 (2.3745–2.7835) | — | 3.222 | 8686 |
| 40 / B: strict growth | 0.0582 (0.0455–0.0583) | 0.0565 (0.0437–0.0583) | 0.0672 | 0.835 | 1083 |
| 64 / Main strict fixed | 1.6926 (1.6548–1.7353) | 1.6526 (1.6111–1.7025) | 2.1033 | 2.308 | 21700 |
| 64 / A: fixed relaxation | — | 2.8775 (2.8268–2.9282) | — | 3.507 | 8696 |
| 64 / B: strict growth | 0.0510 (0.0437–0.0621) | 0.0510 (0.0419–0.0602) | 0.0651 | 0.375 | 1122 |
| 80 / Main strict fixed | 1.6600 (1.5793–1.7701) | 1.6199 (1.5283–1.7264) | 2.0870 | 2.297 | 21673 |
| 80 / A: fixed relaxation | — | 3.0736 (2.9595–3.1877) | — | 3.674 | 8692 |
| 80 / B: strict growth | below resolution (0.0000–0.0018) | below resolution (0.0000–0.0018) | 0.0060 | 0.409 | 648 |

Repeated failed searches bind fixed admission; growth supplies clear geometry.
The separate [all-socket baseline](https://github.com/NikitaMGrimm/slither-sim/blob/4915803/research/performance/2026-10-26-host-known-slots/REPORT.md)
uses one WSL2 host CPU: process CPU 3.203/2.859/3.132 ms/tick and inclusive
admission shares 90.39/89.36/90.53% at 40/64/80. At 64/80 wait p50/p99 is
21.651/35.862 and 33.738/48.643 s, about 29/44 waiting. Shares cannot be multiplied
into measured admission CPU/tick. Our bot-plus-probe workload does not replace
that grade; both final strict arms include main's known-slot optimization.

## Prior cohorts and local-only archives

Exact removed files are in `outputs/spawn-capacity/pruned-record-c1ccde1/`.
Counts below are completed rows, not unique runs: the same six rejected-A holds
are reused in the last three files; do not add them as independent repetitions.

| File | Completed rows | Key metric and disposition |
| --- | --- | --- |
| `initial-results.json` | 45: 15 each control/A/B | B had 3 pit deaths ≤3 s (64/101, ages 1.764/0.357/2.604 s); A had 878 strict intrusions. Safety failed; affinity also unqualified. |
| `unqualified-corrected-results.json` | 25: 13 control, 12 B | B had 0 early deaths/intrusions, but unchecked Win32 affinity failed: observed 0xFFFF, ERROR_INVALID_HANDLE (6). Excluded from resource/timing claims. |
| `rejected-moving-edge-results.json` | 20: 7 control, 7 B, 6 reused A | Verified 0x30; B had 1 pit death ≤3 s (40/7, age 2.534 s), exposing shrink false re-entry. Rejected. |
| `rejected-pre-hold-results.json` | 30: 12 control, 12 B, 6 reused A | B had 1 pit death ≤3 s (40/7, age 2.891 s) after actual entry near a recent expansion. Rejected; motivated the birth hold. |
| `pre-delivery-results.json` | 22: 8 control, 8 B, 6 reused A | B had 0 early deaths/intrusions. Stopped for whole-admission timing, integrated main and decoration correction; search-only costs are not final full-admission costs. |

SHA-256 of each exact archived file:
```text
9965d2b0d8f46e63bcbeff5e9e7edd73c5063ea307bafcd3107fa66bceb1809a  initial-results.json
367f0a44ee01bdd0a955f4c0a8b4d639f2a120044d06645c30f57bb364aac06c  pre-delivery-results.json
68c8a0ac6e87b22261e1ba947f84457ea36a823f39dfb3e64d7257b19d3959fb  rejected-moving-edge-results.json
5915693c696134ab21f613d24b71ad2162f80c835051a1faa444ad9d52c5a5c9  rejected-pre-hold-results.json
47967b5d3aca8a9ab1ce905c2317fc88a66b77731e47a0e6bbf6839abed38ab8  unqualified-corrected-results.json
```
All unqualified runs remain excluded, not erased. Affinity was an agent harness
mistake. Growth/shrink regressions reproduce false entry at 0.651/0.014 s; the
old/new edge test prevents it, and the host hold covers later real crossings.
Initial 20 s screens reached 70.8 living/45 intrusions (sqrt), 79.8/10 (linear), then 79.7/0 (larger strict).
A 10.535 s bot join motivated allowances.
An earlier zeroed-wait logger and repeated retained-death logger were corrected;
their apparent early pit death was an artifact. Originals and interrupted runs
remain local-only alongside raw events, full results, sources, binaries and logs.
Hashes identify those files without making them downloadable; no safety failure
is waived. [source-pins.json](source-pins.json) pins measured/instrumented/delivered
source identities and the full local manifest; duplicate file inventories are omitted.

## Feel, verification and limits

Tests retain fresh 16/32 wall, pit, band, floor/score and density: radial travel 900–1,616 units,
4.90–8.79 s at floor cruise. Gradual shrink/dead zone remain after larger waves. At 80 the normal
wall is 3.25× base, density 23.7% of 32-seat density, travel 15.92–28.58 s; queue allowance reaches
6.5× base wall (23,296). This explicit high-population space/approach tradeoff is unchanged by pruning.
Regressions cover strict queues/holds, moving-pit rules, checkpoint continuation/revision rejection,
wire bounds, fixed pit cells on 80/136/512 maps, own-marker projection and socket decorations.
Floor/food/scoring checks remain; debug food-index addition wraps like subtraction before rebuild.
The implementation's full Rust group passed after rebase onto 3a23dc9; docs checks passed.
Independent GPT 6.1 Sol source review covered policy, pit/minimap, contracts and regressions;
its sub-cell marker update finding was fixed with tests preserving physical-food lifetimes.
Review did not rerun checks/measurements. Strict timing stays on 150d4b1; delivery adds prey bounds/fidelity
reports with zero prey in Battledome and unchanged admission owners. A uses 249cdb2. Pruning added no measurements; the separate shrink follow-up is below.
Owner-relayed community spot reuse/500-unit fallback/modest growth inspired the design;
that server's human deaths/performance were not measured. No outside game server, browser, Linux/macOS,
full tools/WASM/package or human gameplay test was run. No 80-human, dense one-core, equilibrium or
superiority claim follows. Bounded work/space cannot guarantee a maximum wait if all sampled poses
violate safety; later turns, boosts and shrink encounters remain. Zero observed early failures applies only here.

## Review follow-up: shrink presentation

Queued respawns retire before unique enqueue (`alive=false`, death time cleared); occupied/waiting seats are disjoint. No accounting fix.

Five alternating release pairs after warmups replayed the full 11,648→3,694.7421875 shrink: 48,477 moving ticks, 32 pit-following synthetic views, no snakes/food.
Worker-thread fixture CPU ms/tick median (range): **0.2614 (0.2601–0.2688) → 0.0406 (0.0406–0.0409)**.
Sync/diff elapsed medians: 0.1321/0.1291→0.0125/0.0265 ms/tick. Separate profiles counted rebuilds 48,477→3,378 and retained-sector comparisons 12,348,818→864,069.
Shrink now retains decoration handles within a radial food cell (300/256 units); growth past the cached edge refreshes immediately. Extra outward offset is <1 cell, plus normal grid rounding; physics/growth constants stay unchanged.

All final world hashes match; wire differs intentionally at this presentation precision. The regression fails before/passes after, checks held handles, boundary packets, outside-edge bounds and growth; physical-food tests/full Rust pass.
This empty-world component CPU includes views, core steps, timers and packet hashing; diff elapsed includes encoding/hash. All children used verified `0x30`, no errors; shared-desktop timings are descriptive, not a host/network capacity grade.
Original admission rows/PLAN remain unchanged. Compact pairs/profiles are in results.json; full fixture, sources, binaries, attempts and pins stay local-only under outputs/spawn-capacity/shrink-phase/.
