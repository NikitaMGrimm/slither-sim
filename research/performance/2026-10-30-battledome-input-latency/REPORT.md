# Battledome input-to-effect latency

The independent protocol-19 probe measured steering send-to-explicit-own-wanted
reflection p99 of **7.847 ms alone and 12.044 ms at 320 total seats**. These are
single 360-second windows, not repeated capacity tests. All selected windows had
zero deferred ticks: the requested deferred-versus-normal latency comparison is
**not observed**, rather than evidence that overload has no latency cost.

## Method and provenance

The frozen [plan](PLAN.md) pins runtime
`9cd6ba6922c0a63b525ba88b69f86c8558142596`, default portable release, Rust 1.99,
unchanged `src/`, shipped Battledome defaults and no modeled player latency.
One c7a.medium target shares its one core between simulation and sockets; a
same-AZ c8a.4xlarge generator has 16 vCPUs, separate decoding workers and a
CPU-pinned independent probe. Private real sockets, no authoritative observations,
no thin clients and no server hook. The 80/160/320 totals include one probe.
Load bots retain shared received-frame A4 steering and rule-gated constant boost.

Each randomized steering send has one outstanding target. Start is immediately
before WebSocket send; end is immediately after reading the first explicit own
wanted-byte echo. E/4, 3/5 and d/7 own motion forms carry it, with the appropriate
total packet lengths (2/3, 3, 4 bytes respectively, including the tag). Command byte `c`
expects `(c*256+125)//251 mod 256`; tests check all 251 commands. Current-heading
movement and appearance do not qualify. This measures a received target echo,
not rendered turning or pixel latency. Deaths censor one attempt at 160 and one
at 320; no timeouts or silent unsuccessful samples are converted to successes.

The primary solo/80/160 archives come from collection attempt 2; 320 comes from
attempt 3 after a controller-file race invalidated attempt 2's 320. Selection
changed prospectively by completeness/gates, never by latency. All failed and
supplementary runs remain in [ATTEMPTS.md](ATTEMPTS.md). Every primary passes the
>=300 echo quota, six-worker whole-window coverage and unchanged generator gates.
At 320 the worst worker CPU p99 is 19.1% of its core, worst input-p99 report
14.434 ms; all loaded workers report 100% boost actions among eligible actions.
This is action duty, not a claim that a dead/pending snake is boosting.

[Selected data](selected-results.json) binds each row to its own closed archive,
member manifest and complete receipt, and binds all rows to one native-source
archive and build pins. [Reproduction](REPRODUCE.md) names each producer. Native
archive SHA256 is
`407b60d66b5f39299cf45a0402451d97c21650c0922920766b64757e1542d851`.
Raw archives, captures, native sources/binaries and receipts are retained
local-only outside Git, without a public retrieval URL. Hashes establish integrity,
not measurement correctness. Scripts and bounded selected values are public.

## Latency, RTT and delivery tables

All times are milliseconds, nearest-rank quantiles. ICMP is a network/kernel RTT
proxy. WebSocket control RTT includes gateway scheduling; game RTT additionally
includes tick/broadcast work and must not be subtracted as pure network time.
Subtraction pairs each echo with its nearest RTT within two seconds, rather than
subtracting marginal percentiles. It is descriptive, not an isolated server timer.

| Total seats | Confirmed / attempts | Input-to-effect p50 / p95 / p99 / max ms | Deferred ticks |
| ---: | ---: | ---: | ---: |
| 1 | 805 / 805 | 4.286 / 7.229 / 7.847 / 8.005 | 0 |
| 80 | 748 / 748 | 5.377 / 8.449 / 8.757 / 8.937 | 0 |
| 160 | 769 / 770 | 5.688 / 8.850 / 9.382 / 9.693 | 0 |
| 320 | 772 / 773 | 7.162 / 10.318 / 12.044 / 14.623 | 0 |

| Seats | ICMP RTT p50 / p95 / p99 / max ms | Control RTT p50 / p95 / p99 / max ms | Game RTT p50 / p95 / p99 / max ms |
| ---: | ---: | ---: | ---: |
| 1 | 0.161 / 0.179 / 0.188 / 0.224 | 0.630 / 1.217 / 1.251 / 1.254 | 4.157 / 7.229 / 7.693 / 8.156 |
| 80 | 0.156 / 0.232 / 0.320 / 0.424 | 0.783 / 1.374 / 1.411 / 1.568 | 5.221 / 8.317 / 8.753 / 8.911 |
| 160 | 0.157 / 0.273 / 0.377 / 0.435 | 0.787 / 1.570 / 1.851 / 2.285 | 5.685 / 8.846 / 9.535 / 11.849 |
| 320 | 0.159 / 0.277 / 0.373 / 0.409 | 0.913 / 2.318 / 2.603 / 3.070 | 7.093 / 10.467 / 12.161 / 15.097 |

| Seats | Latency minus ICMP RTT p50 / p95 / p99 / max ms | Latency minus control RTT p50 / p95 / p99 / max ms |
| ---: | ---: | ---: |
| 1 | 4.124 / 7.083 / 7.689 / 7.856 | 3.526 / 6.747 / 7.214 / 7.526 |
| 80 | 5.227 / 8.295 / 8.595 / 8.793 | 4.592 / 7.815 / 8.254 / 8.441 |
| 160 | 5.527 / 8.677 / 9.232 / 9.544 | 4.775 / 8.152 / 8.771 / 9.219 |
| 320 | 6.962 / 10.173 / 11.860 / 14.488 | 6.127 / 9.531 / 11.374 / 14.027 |

| Seats | Stats bucket | Samples | Input-to-effect p50 / p95 / p99 / max ms |
| ---: | --- | ---: | ---: |
| 1 | normal-bucket | 805 | 4.286 / 7.229 / 7.847 / 8.005 |
| 1 | deferred-bucket | 0 | Not observed |
| 1 | reporting-gap | 0 | Not observed |
| 1 | uncovered | 0 | Not observed |
| 80 | normal-bucket | 748 | 5.377 / 8.449 / 8.757 / 8.937 |
| 80 | deferred-bucket | 0 | Not observed |
| 80 | reporting-gap | 0 | Not observed |
| 80 | uncovered | 0 | Not observed |
| 160 | normal-bucket | 769 | 5.688 / 8.850 / 9.382 / 9.693 |
| 160 | deferred-bucket | 0 | Not observed |
| 160 | reporting-gap | 0 | Not observed |
| 160 | uncovered | 0 | Not observed |
| 320 | normal-bucket | 772 | 7.162 / 10.318 / 12.044 / 14.623 |
| 320 | deferred-bucket | 0 | Not observed |
| 320 | reporting-gap | 0 | Not observed |
| 320 | uncovered | 0 | Not observed |

| Seats | Kernel capture effect p50 / p95 / p99 / max ms | Echo read age p50 / p95 / p99 / max ms | Send call p99 / max ms |
| ---: | ---: | ---: | ---: |
| 1 | 4.170 / 7.190 / 7.792 / 7.971 | 0.081 / 0.128 / 0.140 / 0.159 | 0.007 / 0.011 |
| 80 | 5.300 / 8.358 / 8.650 / 8.919 | 0.083 / 0.154 / 0.159 / 0.163 | 0.007 / 0.009 |
| 160 | 5.577 / 8.777 / 9.340 / 9.640 | 0.085 / 0.156 / 0.163 / 0.204 | 0.007 / 0.011 |
| 320 | 7.069 / 10.293 / 11.938 / 13.316 | 0.082 / 0.154 / 0.161 / 0.215 | 0.008 / 0.013 |

| Seats | Userspace gap p99 / max ms | Kernel gap p99 / max ms | Userspace excess p99 / max ms | Kernel excess p99 / max ms | Read age p99 / max ms |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 55.995 / 98.001 | 55.995 / 98.024 | 0.249 / 1.771 | 0.148 / 0.445 | 0.157 / 1.727 |
| 80 | 21.086 / 98.156 | 21.048 / 98.225 | 0.597 / 1.798 | 0.541 / 1.675 | 0.163 / 1.561 |
| 160 | 28.063 / 91.371 | 28.057 / 91.431 | 1.000 / 5.064 | 0.959 / 5.060 | 0.162 / 1.579 |
| 320 | 35.559 / 98.156 | 34.914 / 92.638 | 3.067 / 5.407 | 3.089 / 5.302 | 0.162 / 0.382 |

| Seats | Raw gaps >20 / >100 / >250 ms | Kernel gaps >20 / >100 / >250 ms | Residual stalls >20 / >100 / >250 ms | Kernel residual stalls >20 / >100 / >250 ms |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 8626 / 0 / 0 | 8626 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| 80 | 711 / 0 / 0 | 711 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| 160 | 1125 / 0 / 0 | 1013 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| 320 | 1592 / 0 / 0 | 804 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |

| Seats | Load-worker excess p99 / max ms | Independent probe kernel excess p99 / max ms | Load-worker residual stalls >20 ms | Probe kernel residual stalls >20 ms | Worker pass p99 / max ms |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | Not observed | 0.148 / 0.445 | 0 | 0 | Not observed |
| 80 | 0.827 / 11.853 | 0.541 / 1.675 | 0 | 0 | 4.983 / 8.233 |
| 160 | 1.717 / 11.131 | 0.959 / 5.060 | 0 | 0 | 7.849 / 13.389 |
| 320 | 4.491 / 23.922 | 3.089 / 5.302 | 69 | 0 | 9.933 / 14.132 |

Kernel columns use only the verified capture subset. Coverage is 17,493/17,493,
47,015/47,015, 42,634/45,485 and 28,740/42,976 playing frames at 1/80/160/320;
verified echo matches are 805/805, 748/748, 716/769 and 502/772. Captures with
reported kernel drops are excluded from packet diagnostics for that connection;
all monotonic client echoes and all client frame metrics remain included. This
subset may be biased. The packet/read-age results cannot bound unverified echoes.

## Why #281 could show 80 ms excess with low tick CPU

[Historical audit](historical-results.json) verifies each of #281's four original
320 archives and uses its unchanged canonical tick interval. No historical row
was reselected. The host progress diagnostic samples existing census wall time
minus `tick*7 ms`, normalized by its interval minimum; it is coarse (100 ticks),
includes census observation scheduling, and is not an individual packet timer.

| #281 row | Deferred | Excess p99 / max ms | Host wall-minus-tick p99 / max ms | Worker pass max ms | Stall reports / with pass >20 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| 01-base-320 | 0 | 80.618 / 160.693 | 81.064 / 144.900 | 12.221 | 47 / 0 |
| 02-candidate-320 | 27 | 76.751 / 199.884 | 73.876 / 148.060 | 13.006 | 12 / 0 |
| 05-candidate-320 | 0 | 4.499 / 19.959 | 4.385 / 4.820 | 10.822 | 0 / 0 |
| 06-base-320 | 73 | 669.177 / 772.911 | 674.897 / 695.904 | 10.842 | 65 / 0 |

The first base window's excess p99 of 80.618 ms closely matches host-progress
lag p99 81.064 ms despite zero deferrals. Its worker pass max is only 12.221 ms,
and none of its 47 stall-bearing reports has a pass above 20 ms. Across all four
rows, **no** stall-bearing worker report has such a pass. Conversely, the second
candidate window has low host lag (max 4.820 ms), low excess and no residual
stalls. This supports uneven host progress as a major historical source, rather
than a generator pass pause accounting for the 80-ms tail. It does not identify
whether host sleep/wakeup, OS preemption, catch-up scheduling or socket cohort
work produced each old event: old packet timestamps were not recorded.
The first candidate window actually deferred 27 ticks; the later base window
73. The premise that all those high-excess windows had zero deferrals is too broad.

Three distinctions explain the apparent contradiction:

1. Tick CPU excludes time off CPU. Busy-wall p99 can exceed CPU p99, while a
   brief host scheduling stall can leave accumulated clock debt after otherwise
   cheap ticks. The scheduler catches up at most eight ticks per pass and defers
   only backlog beyond its configured threshold (about 252 ms at 7-ms ticks).
   Zero deferrals therefore does not certify steady pacing or zero backlog.
2. Frame excess is accumulated receive wall time minus summed virtual frame-header
   time, relative to a running minimum. It is not a per-input or per-gap latency.
   Permanent deferred time can remain in this accumulated metric. Header rounding
   carries fractional time and is bounded at about half a millisecond; it cannot
   alone manufacture an 80-ms drift. Raw gaps also include intentional sparse
   emission: this study's solo probe has 8,626 raw gaps >20 ms but zero
   header-adjusted residual stalls. Do not label every raw gap a stutter.
3. A shared load worker can add read scheduling delay without failing average
   headroom gates. Here, 320 load clients show 69 residual stalls >20 ms, while
   the independent probe shows zero. Its verified echo read-age p99 is 0.161 ms,
   maximum 0.215 ms. This demonstrates generator/read-cohort contributions in the
   new run; it does not retrospectively assign every historical gap to them.

Thus the historical 80-ms value is supported as a host-progress/accumulated-clock
phenomenon, with exact old socket-versus-scheduler attribution unresolved. It is
not evidence of 80-ms steering response. This run does not reproduce that old
host-lag episode and cannot claim it fixed it.

## Source verification and ranked latency leads

Inspected the pinned `src/client/server.rs`, host broadcast and protocol paths:
zero defaults include modeled latency variations/pauses. The gateway timestamps
commands at poll start; simulation drains commands before a tick and ingests
through tick end. Arrival during a running tick does not literally mutate it.
The early delivery callback runs after **all socket recipient slots**, not after
each player's encoding. With no internal bots, it is effectively the end of that
broadcast cohort. Gateway writes buffer 16 KiB and explicitly flush sessions after
servicing the session loop; large writes may flush sooner. Linux waits can wake
on the delivery channel. `MAX_IDLE=1 ms` is a requested fallback wait, not a fixed
added delay or a guaranteed wall-time upper bound.

1. **Tick phase plus whole socket-recipient broadcast path.** The solo median
   4.286 ms is already much larger than 0.161-ms ICMP RTT; the loaded median rises
   to 7.162 ms. Game ping tracks the echo path, while control RTT stays much
   lower. This ranks tick/broadcast scheduling above network time, but does not
   split tick wait from work. Investigate earlier cohort handoff/flush as an
   exact-payload candidate; prove ordering and byte identity before adoption.
2. **Host wall-clock pacing and wakeups.** #281's common progress lag is the
   strongest evidence for its large excess. Profile wait/wakeup and off-CPU time
   before proposing a change. No claim that the 1-ms constant itself causes the
   80-ms tail. Tick cadence or spreading work across ticks would change feel/rules
   and remains a maintaining-agent proposal requiring the requested play approval.
3. **Generator read scheduling and socket flush path.** New load-worker stalls
   exceed the independent probe's. Keep probe CPU isolation in future studies;
   optimize the worker only as harness work. Control RTT at 320 has p99 2.603 ms,
   leaving room to examine gateway batching, but no internal breakdown was measured.
4. **Network.** ICMP p99 stays below 0.38 ms. It offers much less room than the
   tick/broadcast path on this same-AZ route; no Internet-route inference follows.

No fix or runtime measurement hook is implemented. A future hook would need
maintaining-agent approval; these data do not justify inventing a server split.

## Validation, limits and decision

Three native probe tests pass, the wrong-wanted-byte mutation fails as intended,
and the restored probe passes. The inherited constant-boost regression passes.
Six reducer tests pass, including capture reassembly/out-of-order completion and
the pre-playing challenge/game epoch. Documentation checks run remotely. Full
Rust group is not rerun: this changes only research helpers and documentation,
and every runtime source is pinned unchanged. No independent review is claimed.

The four selected windows are descriptive. Load and ecology grow in a fixed
ascending order, the repeated 320 uses a different staged world, capture loss
limits packet conclusions, and no deferred-latency sample exists. Death-censored
attempts and unqualified first-attempt results are explicit. This is an echo
measurement, not browser-rendered response, an input-to-physical-turn guarantee,
or an official-server latency comparison. The practical result is a reproducible
probe and a distinction between input latency, sparse gaps and accumulated debt.

## Cost and cleanup

Combined conservative allocation bound **$2.352333**, including both Spot pairs,
the interrupted generator and $0.65 noncompute allowance, below the $2.50 cap.
This is a bound, not an invoice. No timer was extended. Each instance had an
in-guest shutdown timer, terminate-on-shutdown and DeleteOnTermination storage.
The [final cleanup](cleanup.json), completed at 04:43 UTC, scanned all 17 enabled
regions: zero nonterminated instances, all volumes, active/open Spot requests and
Elastic IPs. Both current instances were confirmed terminated; the two retired
instances were no longer returned after earlier retirement. The task key pair and
security group were deleted, and protected resources were untouched. The local
private key was deleted. [Cost](cost.json) includes all attempts;
[verification](verification.json) records proof and archive-retrieval checks.
