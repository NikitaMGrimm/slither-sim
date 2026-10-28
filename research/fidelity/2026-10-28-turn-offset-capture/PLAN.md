# Frozen small-count turn-offset capture

Frozen before any discovery request or game connection in this study. Owner
authorization: revised count-37/count-100 cruise-only experiment, at most two
simultaneous clients and four invocations of at most 1200 seconds. #236 is accepted
and owner-landed; its branch is not modified. No runtime or fixture changes.

## Question and predictions

Compare the already fixed D111 rule with D106 using one fixed integer count offset
k in **6..10**, shared across sizes, directions and invocations. Retain original-count
D106 as a diagnostic. No denominator, coefficient, count offset or uncertainty
budget is fitted to new captures.

`Q_D(n)=.033/.008*(.13+.87*((7-clamp(1+(n-2)/D,1,6))/6)^2)` rad/s.
Fixed offset k predicts `Q_106(max(2,n-k))`; D111 predicts `Q_111(n)`.
Use actual received logical count; the primary cells require exact counts 37 and
100, cruise only, both signed turn directions. Optional 174 cells do not replace
missing primary cells or gate the primary decision.

The #236 frozen uncertainty assumptions remain: angular budget
`A=2*asin(sqrt(2)/39)+2*pi/65536`; timing half-width
`B=.001+2*j/1000`, with primary j=42 ms and sensitivities 0,7,14 ms.
An arc is compatible with Q when `abs(abs(angle)-Q*T) <= A+Q*B`.
These are conditional envelopes, not established event-to-send or clock-rate bounds.

For each count, calculate the worst pairwise separation time against k=6..10:
`max_k((2*A+(Q111+Qk)*.085)/abs(Q111-Qk))`. Add 25% margin and round upward to
five seconds. The resulting minimum midpoint-time spans are **25 s at 37**,
**60 s at 100**, and **340 s at optional 174**. Controls hold four additional
seconds for initial target settling and post-interval stability. Stop at the first
complete eligible interval reaching that span; score its actual span, not nominal time.

## Selection and exclusions

- Each invocation seeks one first chronological eligible interval per primary
  count/direction cell, then optional 174 only when time remains. Odd invocations
  seek increasing first; even invocations decreasing first. Counts ascend 37,100,174.
- Use the existing headless Bot, recorder, defaults and safety threshold. A research
  steering adapter supplies actor holds and observer rendezvous; login, protocol,
  25-ms timer, >33-ms changed-heading throttle, ping, reconnect/backoff and avoidance
  defaults remain unchanged. Never request boost. Nickname is neutral: `Turn study`.
- Actor forages with the existing default policy until the target count, then
  requests latest received heading plus/minus pi/2. Avoidance wins at danger>.2.
  Retain interrupted/failed holds and count overshoots. A passed count is marked
  missing for that life; natural default reconnects remain within the same invocation.
- Observer follows the actor using only current causal receipt prefixes and stays
  in a 450..700-unit annulus when safe. Require positive same-life actor presence
  in its cache, observer/actor anchor ages <=300 ms and separation <=800 for holds.
  Stale/unsafe control resets a candidate hold, retaining its failure. No retrospectively
  refreshed earlier pose, extra observer, or negative-cache inference.
- Scored actor anchors must be one connection/incarnation, exact stable logical
  count, no logical-count event for >=1 second before/after, cruise speed <=7 wire
  units, and complete paired coverage. Retain every failed and censored cell.
- Original triplet rules: 39..45-unit chords; positive neighboring server gaps
  <=700 ms; observed signed rate>.15 rad/s; wanted-heading margin>=.55 radians
  in the requested direction across three neighboring triplets; >=5 consecutive
  triplets. No model-error exclusion, trimming or favorable alternative interval.
- Score signed chord-direction sums and midpoint durations from server elapsed
  headers through the pinned client. Keep receipt clocks and transport gaps separate.
  Actor is primary; observer positive-reference/trajectory agreement is a qualification
  diagnostic, not another independent trial.

## Invocations and stop rules

1..2 are development, 3..4 held-out. Reserve each attempt durably before process
launch; a failed process or acquisition consumes its ordinal. No replacements,
extra pilots, endpoint scouting sockets or outcome-based reruns. Archive every life,
close, failed acquisition, partial file, source identity and process result outside Git.

After both development invocations, audit and freeze analysis source hashes in a
separate held-out checkpoint before ordinal 3. Candidate formulas, durations,
eligibility, exclusions and the decision rule never change. Development can expose
implementation failures; any necessary correction is documented/frozen before
held-out acquisition, or collection stops if the original protocol cannot be honored.

If pooled development does not acquire all four primary cells, stop without spending
held-out invocations and report the acquisition gate inconclusive. Per invocation,
stop early if both default clients fail, no pair is established by 300 seconds,
no primary count is attained by 600 seconds, all primary cells are done and optional
174 cannot fit in the remaining time, or an interference/integrity/protocol limit fails.
At 1200 seconds all sockets stop. Source drift, a non-19 setup or challenge failure
stops the study; do not change collector defaults to repair official acceptance.

## Decision rule fixed before data

For each complete held-out invocation, compute the compatible candidate set from
its four primary cells at j=42. Missing primary coverage, integrity failure or
conflicting recipient evidence makes that invocation unqualified for rejection.

- **Reject D111 in favor of a fixed offset:** both held-out invocations have complete
  primary coverage; each excludes D111 in at least one primary cell; the *same*
  fixed k in 6..10 is compatible with every primary cell in both invocations.
  Report that evidence and stop; do not implement a mechanics change.
- **Reject every fixed k in 6..10 in favor of D111:** complete primary coverage in
  both held-out invocations; D111 compatible with every primary cell; each fixed k
  excluded by at least one primary cell in *each* held-out invocation.
- **Inconclusive:** missing cells, failed development gate, no qualified held-out
  replication, both explanations surviving, every candidate failing, contradictory
  directions/recipients, or a rejection not replicated by the rule above.
- Report D106 at original count separately in every cell. The retained long
  174-segment arc remains rejected under #236's j<=42 assumptions; an uninformative
  new sample does not erase it. A conflicting new result is reported as unresolved
  heterogeneity/clock uncertainty, not a silent revival or adoption.

All conclusions remain conditional on the stated quantization/timing assumptions.
Development summaries are not promoted to held-out confirmation. Optional 174
coverage, direction/mode exclusions and every failure remain visible.

## Network and resource limits

Before any official request, obtain an exclusive local capture lock and verify no
other official collector or game connection to the selected server is active.
Monitor other collector processes during acquisition and stop our sockets on conflict.
Use one fresh official discovery-list request and the repo's existing authored
decoder, without fetching/executing official JS. Prefer the retained timing endpoint
only if currently listed active; otherwise use the first active decoded entry.
Bind one endpoint privately for all four invocations; no fallback after a game attempt.
Discovery bytes, decoded list, selection and endpoint binding stay outside Git.

At most two game WebSockets, including connecting/closing sockets, globally for
this task; no other official collector may run concurrently. One Windows logical
CPU, Cargo jobs/Rayon/tests 1, serial collector/build/harness work, modest RAM.
Use the clean pinned client `8d0772c015d6b9b73b80253db8c7349aa600b201` only for
offline analysis; owner checkout stays read-only. External targets are removed
after checks. Commit only research protocol/code, bounded aggregates and hashes;
no raw captures, personal/machine/account/endpoint details, official JS or assets.
