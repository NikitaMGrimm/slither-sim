# Constant-boost Battledome live recheck after #265/#266

At 160 seats neither arm deferred ticks. At 320 both fell behind: the candidate
observed 434 deferred ticks versus 749 for the base, but still spent **63.8% of
its slowest 1% of ticks in spawn search**. Its respawn p95 remained 1.123 seconds.
These are descriptive single-window comparisons, not an isolated causal gain.
**480 seats were not measured. Final cleanup and the all-region audit are pending
renewed AWS login; no cleanup success is claimed.**

This is a real-socket follow-up to
[#263](../2026-10-29-battledome-boost/REPORT.md), comparing the parent of #265's
merge with the revision containing both exact optimizations. No production
code changes are made here. [Plan](PLAN.md), [first interruption amendment](PLAN-ADDENDUM.md),
[same-target recovery](PLAN-RECOVERY.md), [physical-core repeat](PLAN-FINAL-REPEAT.md),
[Zen 5 pool amendment](PLAN-ZEN5-POOL.md),
[same-class recovery](PLAN-ZEN5-RECOVERY.md), [approved generator exception](PLAN-ONDEMAND-GENERATOR.md),
[attempts](ATTEMPTS.md),
[reproduction](REPRODUCE.md), [selected values and pins](selected-results.json).

## Compared windows

| Seats | Arm (order) | CPU p50 / p99 / max ms | Deferred | Gap jitter p99 ms | Excess p99 / max ms | Stalls >20 / >100 / >250 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 160 | base (1) | 2.163 / 3.434 / 10.317 | 0 | 2.217 | 2.841 / 17.409 | 0 / 0 / 0 |
| 160 | candidate (2) | 2.194 / 3.209 / 8.679 | 0 | 1.640 | 2.167 / 15.213 | 0 / 0 / 0 |
| 320 | base (4) | 4.201 / 15.005 / 17.239 | 749 | 10.795 | 4550.121 / 5502.563 | 3799 / 0 / 0 |
| 320 | candidate (3) | 4.129 / 14.451 / 16.908 | 434 | 9.162 | 3037.964 / 3305.448 | 2617 / 0 / 0 |
| 480 | both arms | Not measured | — | — | — | — |

| Seats | Arm | Respawn p50 / p95 / max ms (n) | Pending last / max age ms | Alive mean / min | Boost % / births | Length first → last | Food first → last |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 160 | base | 7.021 / 10.883 / 319.236 (655) | 0 / 239.329 | 159.61 / 157 | 99.87 / 36842 | 56.18 → 60.05 | 9461 → 9181 |
| 160 | candidate | 6.622 / 10.689 / 35.995 (626) | 0 / 9.335 | 159.58 / 157 | 99.88 / 38208 | 55.20 → 59.06 | 9589 → 10127 |
| 320 | base | 14.166 / 1294.621 / 1815.609 (1007) | 8 / 1558.758 | 316.69 / 299 | 99.58 / 66713 | 54.94 → 55.00 | 16708 → 19722 |
| 320 | candidate | 12.077 / 1122.827 / 1794.981 (993) | 0 / 1346.073 | 317.31 / 300 | 99.67 / 69338 | 55.39 → 56.37 | 15433 → 16559 |

| Seats | Arm | Tail n / mean CPU ms | Spawn ms / % | Sockets+broadcast ms / % | Minimap ms / % | Other ms / % | Search / exhausted ticks |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 160 | base | 128 / 5.158 | 2.010 / 38.98 | 2.291 / 44.42 | 0.414 / 8.03 | 0.442 / 8.57 | 68 / 31 |
| 160 | candidate | 128 / 3.838 | 0.704 / 18.35 | 2.468 / 64.30 | 0.194 / 5.06 | 0.472 / 12.29 | 44 / 5 |
| 320 | base | 120 / 15.617 | 9.689 / 62.04 | 4.773 / 30.56 | 0.369 / 2.36 | 0.787 / 5.04 | 120 / 117 |
| 320 | candidate | 123 / 15.141 | 9.660 / 63.80 | 4.422 / 29.21 | 0.309 / 2.04 | 0.749 / 4.95 | 123 / 120 |

| Order | Arm / seats | Canonical ticks / seconds | Full-run deferred | Calibration deferred | Outside all marked windows |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1 | base / 160 | 12800 / 89.601 | 0 | 0 | 0 |
| 2 | candidate / 160 | 12800 / 89.603 | 0 | 0 | 0 |
| 3 | candidate / 320 | 12300 / 89.140 | 566 | 0 | 132 |
| 4 | base / 320 | 12000 / 89.512 | 884 | 0 | 135 |

Supplementary windows, excluded from the primary comparison:

| Attempt | Seats | Arm | CPU p99 / max ms | Deferred | Respawn p95 ms | Slow-tail exhausted ticks / n |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| Interrupted pair | 160 | base | 7.951 / 9.613 | 0 | 12.084 | 127 / 128 |
| Interrupted pair | 160 | candidate | 3.421 / 9.199 | 0 | 10.540 | 20 / 127 |
| Earlier repeated pair | 160 | base | 3.508 / 9.303 | 0 | 10.662 | 5 / 127 |
| Earlier repeated pair | 160 | candidate | 3.551 / 8.676 | 0 | 10.851 | 14 / 127 |
| Earlier repeated pair | 320 | candidate | 13.962 / 16.369 | 330 | 1380.102 | 120 / 125 |

CPU is the aligned sum of simulation and socket thread CPU, followed by
quantiles, not a sum of separately computed quantiles. Each primary window is
90 scheduled wall seconds; complete 100-tick interior blocks determine the
reported tick count and duration. Clients contribute only whole reports inside
that CPU interval. Jitter subtracts protocol elapsed-header time from each
receipt gap; excess subtracts cumulative header time relative to a reset running
minimum. Residual stalls count individual jitter gaps, so accumulated excess
can grow without a gap exceeding 100 ms. These are application delivery
diagnostics, not absolute one-way or input-to-effect latency.

Respawn waits cover completed login-enqueue-to-Playing intervals, excluding
the fixed reconnect delay and that connection's preceding TCP/upgrade handshake.
Pending requests remain censored; their last count and maximum observed age
are reported separately. Length means body segments. Boost births count actual
physical boost pellets; free boosting at the permanent floor produces none.
Alive minima are periodic census samples, not a per-tick lower bound.

The tail table selects each window's own slowest aligned-CPU 1%. Its four
categories are exclusive means over those same ticks. Minimap is subtracted
from broadcast before combining the latter with sockets. The minimap scope
includes recipient gating/shared work, not just outline rasterization. Search
and exhausted columns count ticks with those events. Full-run deferrals include
warmup, transitions and artifact flush intervals; none are silently discarded.

## Interpretation and remaining limits

At 160 the candidate's tail is led by sockets/broadcast (64.3%); spawn searches
take 18.3% and minimap 5.1%. The base's tail is split between sockets/broadcast
(44.4%) and search (39.0%). Different search/exhaustion counts show why the
observed tail change cannot be assigned entirely to the two exact fixes.

At 320 the ranking is unchanged: **spawn search first** (candidate 9.660 ms,
63.8% of a 15.141 ms mean slow tick), **sockets/broadcast second** (4.422 ms,
29.2%), other CPU 4.9%, minimap 2.0%. All 123 candidate tail ticks perform
searches and 120 exhaust; base has 117 exhausted ticks out of 120. Exact
acceleration of the existing search is the next measured target. No rule,
candidate, RNG or retry change is proposed here. The broad search phase does
not establish which inner function dominates; profile the exhausted-search
fixture before selecting another implementation.

Candidate-320 jitter p99 is 9.162 ms versus 10.795 ms for base; respawn p95 is
1122.827 versus 1294.621 ms. Accumulated frame excess still reaches seconds,
despite no individual residual gap above 100 ms. Both rows remain overloaded.
Actual boost duty is 99.58% base and 99.67% candidate at 320, below the earlier
study's approximate 99.7–99.9% range but above the frozen 99% qualification
floor. The boost override is identical. **There is no evidence about what
dominates at 480**, because neither primary row completed.

One primary window per arm/count establishes an observed comparison, without
replicated uncertainty or an isolated causal speed ratio. Worlds start fresh
and grow naturally; matched scheduled wall-time prehistory does not mean equal
virtual age, body lengths, food, deaths or spawn work. Under overload, the
slower world advances fewer ticks, changing later received inputs and ecology.
The retained census, actual boost duty, pending waits and full-run deferrals
bound interpretation. No capacity maximum, long-uptime guarantee, Broadwell
ratio or human-play acceptance is established. The run does not stress a full
15-minute same-connection kill history. Physical-food bot delivery omits
decorative pellets, although shared marker work remains.

The exactness evidence for the two production changes lives with the
[minimap cache](../2026-10-29-pit-minimap-cache/REPORT.md) and
[spawn-ring prefilter](../2026-10-29-spawn-ring-prefilter/REPORT.md) studies:
both protocols' byte traces, mutation controls and alternating fixed-work
instruction pairs. This live check does not repeat or replace those proofs.
Changing admission candidates, RNG, retry cadence, clearance, floor, population
sizing or delivery remains outside this study. Any feel/rule proposals still
need the maintaining agent's approval on a local play server first.

## Method and provenance

Base `cc1bb1e9935094a00cf513413d7922d858dd5c1e` is the parent of #265's merge
`78de98b8`; candidate `539b37f3be969827b5eac67c2978e604f9e814e8` adds #265 and
#266. Neither arm includes marker reuse. Both use identical study-only clocks,
phase instrumentation and portable x86-64 Rust 1.99 release flags, with
`debug=1`. Native source/binary digests are pinned once per arm. One
candidate-built client executable is shared by both arms; its source and A4
policy bytes match across the two archived trees. The native treatment
regression preserves exact heading bits and the received-rule boost gate.

The server is one c7a.medium core; every host task has affinity to that core.
Six fully decoding protocol-19 WebSocket client workers use logical CPUs 0–5
of one c8a.4xlarge, with control on CPU 7. Its 16 vCPUs are 16 physical
cores with one thread each; extra CPUs are unused by clients. This supplies
primary rows 1–3. After two further Spot interruptions, the maintaining agent
approved one On-Demand c8a.2xlarge (eight physical cores), supplying row 4.
The same six workers, native client,
policy and target server remain. The 320 comparison spans this generator
instance/size/market transition; it contributes to the unreplicated run/network
variation and prevents an isolated causal percentage claim. Earlier failed
generator rows supply no primary comparison.
The target and each generator are in the same Frankfurt AZ and bot traffic uses private
addresses. No TLS,
added network delay, in-process bots, thin observations or authoritative policy
inputs. Configuration is shipped responsive Battledome, seed 7, 641 reserved
remote slots, without a delivery override. A4 chooses steering, then the
research harness forces its boost bit when received rules permit free floor
boost. Every accepted calibration/primary window must have at least 99%
living-tick boost duty.

Predeclared order: base-160, candidate-160, candidate-320, base-320, base-480,
candidate-480. Each row starts a fresh world and clients, warms 300 seconds at
80, then settles 60 and measures 90 seconds at each 80/160/320/480 stage through
its target. Earlier stages are retained calibration/history, not extra primary
replicates. Only one world/cohort runs at a time on the retained target. No checkpoint
restore, world edits, forced churn, descending stage or human hold.

A placement first failed without launching an instance. The next pair completed
two 160-seat rows, then Spot reclaimed its generator during candidate-320
warmup. After recovering all surviving host sources and verifying cleanup,
the six-row sequence restarted on a fresh pair. Its generator was also reclaimed
after two 160 rows and candidate-320, during the base-320 ladder. The target was
initially retained and only the generator replaced: a c7a request failed for
capacity without launching, then c7i launched. That replacement's candidate-320
failed the unchanged input-lateness gate (59.197 ms against 42 ms), despite
worker CPU p99 below 34%, stable queues and zero socket errors. Collection
stopped and cleaned up rather than accepting that row.

An eight-physical-core c7a generator then also failed the input deadline at
160 (46.605 ms, worker CPU p99 23.34%, passing queues/errors), and was cleaned
up. Core count and average utilization alone did not qualify either alternative.
The final sequence returned to the original Zen 5 family in a larger c8a pool,
keeping six workers. Its first placement failed for capacity without an instance;
the alternate placement supplies the retained target and first three primary
rows. Two more generator reclamations interrupted base-320 warmup and then
finalization. Their surviving host sources are retained; deleted client sources
remain unavailable. The approved On-Demand exception restarts only the unfinished
base-320 ladder. That archive verified. The AWS CLI session expired at 14:09 UTC
during the following ladder's initial 80-seat calibration. Collection stopped
immediately, without retrying credentials. No 480 primary was measured.
Every rebuilt source/binary
pin matches, including the shared client. All five earlier qualified windows
remain supplementary, and both unqualified rows remain failure diagnostics;
none supplies a final primary. The two interrupted ladders' client receipts were
lost with their Spot root volumes and remain explicitly unavailable; surviving
host sources were verified before release. The original plan and every amendment
remain in the archive.

The selector exactly reproduces original online tick ranges, then independently
selects complete interior blocks from final host logs. Aligned CPU means must
reconcile with separately sampled process/thread counters within the inherited
max(0.03 ms/tick, 2%) tolerance. Generator CPU, input lateness, decoding progress,
socket errors and queue growth have the inherited acceptance gates. Host
overload alone never rejects a row. Boundary process inventories are retained.

## Verification, cost and cleanup

All four primary archives and five qualified supplementary archives were
size/hash/source-verified. Exact online replay and independent aligned-task
clock reconciliation pass; all primary generator, treatment and queue gates
pass. Both alternative-generator failures remain excluded. The seven remote
generator builds each passed the heading/rule regression for both arms.
See [verification](verification.json). No local Cargo or SSH bench work was used
for this study. Documentation and privacy checks are recorded there; no
independent review or human play acceptance is claimed.

The approved conservative full-timer bound is **$2.973667 against a $3 cap**,
covering all eleven paid instances across all attempts, including the four Spot
interruptions and single On-Demand generator. It is a bound, not an invoice:
$0.75 prior compute + $0.182 retained target + $0.625 two session Spot generators
+ $0.766667 On-Demand generator + $0.65 all-attempt storage/IPv4/transfer/control
and miscellaneous allowance. The verified On-Demand retail rate was $0.49196/h;
the bound uses $0.50/h. [Cost calculation](cost.json).

**Cleanup remains pending.** Earlier attempts' all-enabled-region scans passed,
but the final current-session scan and task key/group deletion could not run
after credential expiry. Target poweroff was verified for 15:10:44 UTC and
generator poweroff no later than 15:19 UTC, with terminate-on-shutdown and
DeleteOnTermination. These safeguards do not prove termination happened.
After the maintaining agent announces login restoration, confirm both states,
remove own remaining resources and append the final all-region receipt to
[cleanup](cleanup.json). No renewal, retry, timer extension or further collection
is authorized while waiting.

Archives become ready only after close, complete gzip verification and hashing.
All required raw sources must be retrieved and locally size/hash/source-verified
before source teardown. Full metric logs, binaries, source manifests, cloud
receipts and commands remain local-only, not publicly downloadable. Identifiers,
endpoints and private access information are excluded from published text.
The [table generator](tables.py) recomputes every numerical table from selected
values; the reproduction recipe links donor harnesses rather than copying them.

## Parked marker attempt

`perf/pit-marker-reuse` remains pushed at
`58145ae5ed33b912efe4151eb9e4d3414d78c235`, as requested. The maintaining agent
parked it as too marginal to time: **no PR and no guest timing**.
It is excluded from both arms. This task uses no SSH bench guest time.
