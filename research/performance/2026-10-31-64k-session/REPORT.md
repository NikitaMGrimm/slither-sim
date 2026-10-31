# What the 64,000-bot session taught us

On 2026-10-31 a public SlitherSim session ran 64,000 center-steered bots on
a c8a.2xlarge in Ohio. This records selected measurements from that session and
its supporting experiments. It is not a 64k capacity claim for the shipped host:
the session used a private prototype, approximate bot collisions and few actual
viewers. Replay timings, synthetic loads and live observations are separated below.

## Setting and evidence

The last captured public build was `c91bd13064329e3e5885b63ce2deed068c542568`:
64,000 bots, collision stride 3, adaptive 80–100 ms ticks, prey 0, radius 32,000,
hard boundary, no pit or radius following, and no food cap. Normal food
replenishment and death food remained enabled. Bots aimed at the center on birth,
then updated a staggered sixth each tick. Births used the normal official-position
sampler and random headings, without clearance search; a bounded newborn-bot
exclusion protected living players. Player heads still received collision tests
every tick. This deliberately changed bot-vs-bot outcomes.

The deployed binary used fat LTO, native CPU targeting, matching-source PGO and
`GLIBC_TUNABLES=glibc.malloc.hugetlb=1`. World and sockets used separate cores;
system transparent hugepage (THP) settings stayed at `madvise`. Builds and checks
ran separately from the public game. The binary SHA-256 was
`202bc2ad6904e4b2c40caa1ec1b0b152ca2771f32fe960c98ac7cc49ab389818`.

Two earlier zero-player live checkpoints supplied offline replays. L64-240 was
captured at 240.192 wall seconds / 56.36 game seconds with 55,733 pellets;
L64-600 at 600.037 / 223.84 seconds with 186,601 pellets and a longest snake of
235 segments. Both preserve 64,000 bots plus 128 dormant socket seats, controller
tick and spawn RNG. Their 40 ms fixed-dt, uniform-disk respawn controller predates
the final public spawn policy. They are live-derived simulator fixtures, not
official-server captures or recordings of human play. Fresh restores start with
cold derived caches. The L64-600 checkpoint SHA-256 is
`df9966ac93a7130d865bb949570e8042a9b9e804d458dbddc09ae7db97c84b20`.

Raw data, checkpoints, full traces, source/build bindings and failed attempts are
**retained outside the repository**; access is restricted, with no public download.
The source records used here are:

| Retained record | Use in this report |
| --- | --- |
| `host-64k-20261031/SESSION-REPORT.md` | Final public setting, co-location and closure; supersedes the earlier handoff |
| `SESSION-REPORT-fixed80-preserved.md`, `SESSION.md`, `REPORT.md`, `ohio-final-20261031/`, `frankfurt-owner-20261031/`, `L64/` under that session directory | Earlier settings, aged failures, admission checks and checkpoint provenance |
| `storm-scoreboard.md`, `storm-scoreboard-20261031/` including `PGO-RECIPE.md`, `PGO-BUILD-REPORT.md`, `REPORT-b36c8835-THP.md` | Frozen D64/L64 controls, build and allocator experiments |
| `storm-collision-stride-20261031/REPORT.md`; `storm-stride-lod-20261031/REPORT.md`, `REPORT-tight.md`, `REPORT-grid.md` | Approximation timings and counterexamples |
| `storm-strikes-20261031/session-distance-lt-report.md`, `ATTEMPTS.md`; `storm-collision-grid-20261031/RESULTS.md` | Guarded distances, collision indexing and rejected variants |
| `storm-live-food-20261031/REPORT.md`, `storm-live-movement-20261031/REPORT.md` | Food fallback, exact movement candidates and rejected screens |
| `storm-adaptive-tick-20261031/REPORT.md`; `presentation-thread-design-20261031/DESIGN.md` | Separate adaptive load test and source-based concurrency study |

The latest retained public capture is 17:54:16 UTC, not the planned 18:35 capture.
The latter failed because scheduled shutdown blocked new logins; an alternate
management lookup was denied. No later telemetry is claimed. The retained archive
SHA-256 is `4c384aa8581d55a4eb05dc6133ef4963c10a8ff5d78eb76e666cac8292d63cd9`.

## Inexact shortcuts: faster, with changed collisions

Stride N checks each bot head only on its join-number phase every N ticks; N is
a tick count, not milliseconds. Bodies remain collision targets every tick.
On L64-600, the initial stride experiment used a c8a.xlarge core, Rust 1.99,
locked portable release (`bot,parallel`), hugetlb advice and 40 ms dt. Each row
pools three fresh 512-tick continuations, including the cold first tick; controller
and admission work are excluded from world-step timing.

| Stride N | World-step wall p50 / p99 (ms) | Measurement cohort |
| ---: | ---: | --- |
| 1 | 68.020 / 68.999 | Initial stride study |
| 2 | 51.221 / 52.260 | Initial stride study |
| 3 | 44.448 / 45.577 | Initial stride study |
| 4 | 42.85 / 46.10 | Later grid-LOD study, three 512-tick windows, 50 synthetic player-role marks |

The N=4 row is a separate cohort, not a fourth arm of the initial comparison.
None establishes a 40 ms live-host budget. N=1 retained byte-identical checkpoint
and ordered-event output over 512 ticks; N>1 intentionally did not. Exact
checkpoint export was disabled at N>1 because phase and sweep history were not
serialized.

Phasing permits X-crossing and curved-path misses: a chord between test tips is
not the intervening path, targets move, and a target can die before the deferred
test. In a 64-tick paired continuation, 199 N=1 body-death victims had no death
within the N=2 window, and 417 had none at N=3. These worlds diverge after the
first changed death, so those counts are bounded outcome differences, not a causal
contact-delay distribution. A per-tick exact fork found 828 lives still alive
after their next scheduled N=2 test; a separate synthetic two-tick crossing proved
a lost contact when the target died before the deferred test. Delay is therefore
not generally bounded by N−1 ticks.

Player-aware LOD also cost more than plain stride (world-step wall timing).
With 50 synthetic player-role
surrogates, the original 3,285-unit protected radius measured p50 59.06 ms at N=3
and 55.69 ms at N=4, versus plain N=3 at 44.96 ms in that cohort. Tightening the
radius to 1,570 units gave 52.85 / 48.98 ms (N=3/4), with plain N=4 at 42.81 ms.
The final grid classifier gave 52.40 / 48.39 ms, versus plain N=4 at 42.85 ms.
Each variant used three fresh 512-tick windows on one c8a.xlarge core. These were
role-marked bots, not 50 socket clients. Protecting nearby heads also cannot undo
changed offscreen history or protect every visible tail whose head is distant.

**Decision:** collision stride, player-aware LOD and their session integration
stay out of the product. Their code is **kept outside the repository**. Disabling
prey was another session behavior choice, not an exact optimization.

## Exact work and product status

The following results belong to their own pins and windows. They are not additive
and do not predict the final strided public world's tick cost. Product status was
checked on 2026-10-31 against main `9bd14939`. D64 is the earlier synthetic,
unsettled 64k fixture at 7 ms dt, distinct from live-derived L64 at 40 ms.

| Work | Measured scope and result | Where it landed |
| --- | --- | --- |
| Expanded collision grid | Frozen synthetic D64, ten adjacent alternating 512-tick pairs on one c8a core: median run CPU p50 81.382 → 59.801 ms; median paired saving 26.45%. Separate L64 scoreboard: 107.989 → 76.829 ms pooled p50, three 512-tick windows per build. | Merged [#368](https://github.com/NikitaMGrimm/slither-sim/pull/368), `c4c185e8` |
| Unchanged steering cache | Frozen D64, ten 256-tick pairs on one c8a core: median paired saving 1.67%. The `official-200-bot` control exceeded its 50 µs allowance by 5.51 µs; that exception was explicitly accepted. | Merged [#369](https://github.com/NikitaMGrimm/slither-sim/pull/369), `7f4cae82` |
| Compact food index threshold | Frozen D64, ten 256-tick pairs on one c8a core: p50 81.538 → 80.344 ms; median paired saving 1.19%. | Merged [#370](https://github.com/NikitaMGrimm/slither-sim/pull/370), `2f34a000` |
| Retained collision join order | Frozen 64k storm, ten 512-tick pairs on one core of a 4-vCPU Zen 5 EPYC KVM guest: median paired world-plus-admission saving 1.85 ms (2.28%), 9/10 faster. | Merged [#376](https://github.com/NikitaMGrimm/slither-sim/pull/376), `b36c8835` |
| Dense pickup after depletion | L64-240, ten fresh first-64-tick pairs on one c8a core: world-step p50 227.363 → 55.647 ms, 10/10 faster. L64-600: 76.923 → 77.016 ms, 4/10 faster. | Merged [#377](https://github.com/NikitaMGrimm/slither-sim/pull/377), `0ead2664` |
| Grown-snake grid reach cap | Against `b36c8835`, ten adjacent 512-tick L64 pairs on one c8a core: world CPU p50 77.393 → 73.578 ms, 10/10 faster. | Open [#378](https://github.com/NikitaMGrimm/slither-sim/pull/378); integrated locally for the session, not merged |
| Guarded `distance_lt` endpoint test | L64-600 at stride 1, ten 512-tick pairs on one core of a 4-vCPU Zen 5 EPYC KVM guest: median paired world-plus-admission saving 4.072 ms (3.55%), 9/10 faster. | Session integration `0a47cf84`; upstream [#386](https://github.com/NikitaMGrimm/slither-sim/pull/386) is open with fresh catalog qualification pending |
| Presentation bounds reuse | Last five minutes of separate 15-minute, roughly two-client 60 ms runs: shared setup 18.05 → 6.40 ms. Prey also changed to 0, so this is not an isolated whole-world A/B. | Session-only exact reuse candidate, combined in `0a47cf84`; not in checked main |

These candidates retained their stated checkpoint/event or trace controls; the
presentation integration matched a 512-tick L64 world/controller/event replay.
That endpoint proof alone is not a full wire-equivalence or visual acceptance test.
The food fix addresses query fanout: the captured slowest tick spent 214.427 ms
in food queries, without cold index creation. L64-240 returns to the dense path
after 99 ticks, so warming it past that point would hide the failure.

Failed attempts remain part of the result. Packed collision candidates saved
time on L64 but failed small-world catalog limits; the session nevertheless
included a local packed variant, which is not upstream acceptance. The unadapted
integration failed the stride-sweep regression and was corrected before use.
Food cell-order changes regressed both live fixtures; head-inline was inconclusive.
Movement compact/preparation/settled screens lost or were inconclusive. A separate
exact head-tip reuse candidate saved 1.126% in ten L64 pairs on its pinned session
base, but is not established as deployed. Earlier guarded-distance variants failed
the 10k control; their rejected results are not replaced by the later session gate.

## Build and allocator measurements

These are separate experiments on `b36c8835`, one dedicated c8a.xlarge core,
Rust 1.99.0 / LLVM 23.1.1, restoring L64-600 for every continuation. Each arm had
one discarded 512-tick window and three counted 512-tick windows, including cold
first ticks, in balanced orders. CPU quantiles measure only `Simulator::step`.

| Build (all with hugetlb advice) | CPU p50 / p99 (ms) |
| --- | ---: |
| Thin LTO, one codegen unit, portable CPU, unwind | 70.662 / 75.409 |
| Fat LTO, one codegen unit, native CPU, abort | 69.092 / 74.333 |
| Same fat/native/abort build plus D64-trained PGO | 62.969 / 68.884 |

The complete fat/native/PGO recipe reduced p50 by **10.89% (about 10.9%)** versus
portable release; PGO alone relative to the fat/native arm reduced it by 8.86%.
Training used D64 only, not held-out L64. Final world/controller bytes and per-tick
lifecycle/decision counts matched across the builds. This is replay evidence,
not a networking-host speedup guarantee: earlier live 48k/stride-2, zero-player
120-second rows after 30 seconds warmup measured native 33.678 versus PGO
33.312 ms mean, only about 1%, with evolving worlds rather than a frozen A/B.

The same-binary allocator experiment measured p50 **75.790 → 70.555 ms (−6.91%)**
with `GLIBC_TUNABLES=glibc.malloc.hugetlb=1`, retaining THP `madvise`. System-wide
THP `always` instead gave 74.158 ms (−2.15%); original settings were restored.
Each setting had the same discarded/counting policy in ABC/BCA/CAB order, and
continuation results matched. Value 1 requests transparent hugepage advice from
glibc; it does not reserve an explicit HugeTLB pool. These savings must not be
added to the PGO percentage, whose arms already used hugetlb advice. See the
[host recipe](../../../docs/hosting.md#large-bot-worlds).

## Aging, backlog and co-location

Short fresh-world windows underestimated sustained cost. At fixed 60 ms,
stride 3 with prey enabled, a 15-minute run after 60 seconds warmup ended with
last-five-minute mean/p99 work of **82.55/97.56 ms** and **1,366 deferred ticks**.
A later bounds/distance/prey-0 candidate also failed 60 ms: **69.28/84.59 ms** and
**670 deferred ticks** in its final five minutes, after about 210 seconds warmup
and a 15-minute measured run. The separate private fixed-80-ms validator, after
60 seconds warmup and 15 minutes with two mixed-protocol clients, measured
**71.80/86.59 ms**, zero late/deferred ticks in the final five minutes. That is
89.8% mean utilization, not an 85%-headroom pass or 50-player public acceptance.
Size the tick from at least a 15-minute aged run with the intended player load.

Frankfurt's earlier 48k/40 ms world showed the latency consequence. During
14:45–14:46 UTC, work p99/max was 53.6/62.5 ms, with 172 deferred ticks and
start lateness p99/max **342.1/355.4 ms**, alongside reported roughly 350 ms ping.
A separate two-minute TCP sample measured RTT 19–89 ms and peak kernel send
queue 3,095 bytes. Pongs wait for world ingestion and ordered output. **Inference:**
world backlog was sufficient to explain the reported ping; historical application
queue telemetry was unavailable, so this does not exclude other contributors.

On the same eight-core Ohio VM, a second 45k world used different world/socket
cores. Main-world observations were:

| Window | Main work mean / p99 (ms) | Selected dt p50 / p99 (ms) |
| --- | ---: | ---: |
| Before 45k, three minutes | 66.76 / 80.27 | Not reported here / 80.27 |
| With 45k, three minutes | 86.28 / 103.42 | Not reported here / 100.00 |
| With 45k, later two minutes | 84.06 / 96.50 | Not reported here / 96.52 |
| After stopping 45k, three minutes | 75.86 / 91.34 | 80.06 / 91.35 |

This comparison is **confounded by aging and changing player counts**: pellets
grew from 117,703 to 234,805 across the baseline and later reached roughly 256k.
After stopping the second world, main's process stayed unchanged; mean players
was 0.88, cap loss zero and game/wall ratio 1.000. **Inference:** recovery supports
shared-resource contention contributing, but is not a controlled estimate of its
size. Separate CPU affinities did not provide machine-level isolation. Run one
large world per machine.

## Adaptive scheduling, admission and presentation

A separate native, non-PGO adaptive study used stride 3, normal prey and 40–100 ms
limits with 50 mixed-protocol clients generated elsewhere. Over 900 seconds after
123.2 seconds warmup, mean selected dt was 80.751 ms, game/wall ratio 0.999986
and cap loss 0.012560 seconds. The driver had zero failures, but strict outside
lifecycle rounds passed only 43/45; two missed their lifecycle deadline. Earlier
stride-2/80-ms attempts lost 131.320 and 105.279 seconds and failed; a planned
120 ms trial was cancelled. None of these settings replaces the final public
80–100 ms/prey-0/PGO provenance above.

Adaptive scheduling has an upstream successor, **proposed in
[#384](https://github.com/NikitaMGrimm/slither-sim/pull/384)**: optional host
`maxTick` stretches dt up to a cap instead of building a fixed-tick backlog.
It remains open at the status check. Work beyond the cap still loses simulation
time; it does not create CPU capacity. Monitor cap loss and game/wall progress.

Vlither was rejected with `Invalid challenge reply` until
[#383](https://github.com/NikitaMGrimm/slither-sim/pull/383), now merged as
`9bd14939`. Its solver uses a +2 rotation increment; the stock variant uses +4.
The old protocol matrix had claimed coverage from source reading only. A real
unchanged client probe failed on the session baseline and joined the staged
candidate, with and without a custom skin. That fix was never deployed to the
closing public session. Upstream synthetic admission replay covers the exchange;
it does not establish desktop rendering or complete gameplay compatibility.
**Lesson:** replay real client admission before claiming client coverage.

The presentation-thread design study made no implementation or new timing claim.
Its frozen aged two-viewer row attributed **18.046 ms** to shared setup and only
**0.619 ms** to the recipient loop, including occasional lazy shared HUD work.
The later candidate already reduced setup to 6.40 ms. **Conclusion from that
scope:** reduce shared geometry work first; recipient workers cannot recover the
shared cost, and the old setup saving must not be counted twice. Keep presentation
inline by default. Snapshot/pipeline gains remain unmeasured proposals.

Outside protocol-19 elapsed/bare and protocol-13 lifecycle checks passed on the
final public build. They establish admission and packet lifecycle, not human
smoothness: uneven pacing was reported during co-location. No visual acceptance
or exact official-mechanics claim follows from this session.
