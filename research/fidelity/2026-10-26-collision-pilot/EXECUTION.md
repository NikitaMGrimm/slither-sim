# Collision pilot execution: failed feasibility

**Answer:** official pilot one failed on a join timeout after **7 attempted
slots, 0 launched and 0 qualified**. The frozen gate stopped pilot two.
No online collision rule, fitted threshold or join-order verdict follows.
[Selected results and all retained archive hashes](execution-results.json).

## Acquisition history

1. **Initial scouting:** five Origin-less scouts closed with 1006 before any
   frame or join. Populations were null; no pilot invocation was reserved.
   The procedure omitted the required `Origin: http://slither.io`.
2. **Reproduction:** omitting Origin again gave no frames; adding it yielded
   one challenge and four outgoing frames, then a 1006 close after login.
   The remaining failure was traced to ping `0xfb` sent before the challenge.
   The scout and collision paths lacked the base collector's own-snake guard;
   #152 fixed this second acquisition defect. Challenge/login serialization
   matched the pinned functions, with default skin 0; nicknames differed.
3. **Vantage comparison:** the unchanged movement/headless collector joined
   from both authorized comparison vantages. Setup/leaderboards arrived with
   first populations 454 and 499. Their 1006 closes followed requested shutdown,
   rather than login rejection. No pre-setup ping occurred.
4. **Corrected scouting:** on fixed source, the first five active decoded list
   entries gave **447 / 309 / 201 / 94 / null**. All reached setup; the fifth
   closed before a leaderboard. First populations were observed within 10 s;
   every close completed before the next scout. No later entries were probed.
   No ping preceded setup or own appearance. The <=100 gate selected 94.
5. **Pilot one:** the preparer bound that candidate with Origin and unchanged
   scientific fields. First populations were 98/98, maxima 99/99. Five slots
   ended prelaunch for third-party proximity, one for arena margin; the final
   slot was censored by a join timeout. No own game-over packet was observed.
   No retry, replacement endpoint or second pilot followed.

## Encounter counts

Probe is the controlled role; first is actual first-join identity. Targets are
per cell across both originally planned invocations. As the plan defines,
prelaunch aborts are exclusions (T: third party, A: arena margin); the raw
ledger labels them censored outcomes. Only the last attempted slot, which has
no terminal encounter result, is censored by the invocation's join timeout.

| Cell | Attempted | Excluded | Launched | Qualified | Contact | Survival | Censored | Target |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Ladder 1.3, probe 0 | 1 | 1 T | 0 | 0 | 0 | 0 | 0 | 2 |
| Ladder 1.1, probe 0 | 1 | 1 T | 0 | 0 | 0 | 0 | 0 | 2 |
| Ladder 0.9, probe 0 | 1 | 1 A | 0 | 0 | 0 | 0 | 0 | 2 |
| Ladder 0.7, probe 0 | 1 | 1 T | 0 | 0 | 0 | 0 | 0 | 2 |
| Ladder 1.3, probe 1 | 1 | 1 T | 0 | 0 | 0 | 0 | 0 | 2 |
| Ladder 1.1, probe 1 | 1 | 1 T | 0 | 0 | 0 | 0 | 0 | 2 |
| Ladder 0.9, probe 1 | 1 | 0 | 0 | 0 | 0 | 0 | 1 | 2 |
| Ladder 0.7, probe 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| Head-on, probe 0, first 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| Head-on, probe 1, first 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| Head-on, probe 0, first 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| Head-on, probe 1, first 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2 |

Qualified totals: **0/16 ladder, 0/8 head-on**. Acquisition completion and both
nonzero-qualified gates failed. Third-party-aborted **launched** fraction is
undefined (0 launched), not 5/0 or 0%; all five exclusions preceded launch.

## Unqualified staging quality (descriptive only)

| Quantity | Entries | Minimum | p95 | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Own anchor age, ms | 464 | 1.17 | 226.21 | 267.04 |
| Peer anchor age, ms | 298 | 1.17 | 225.42 | 264.12 |
| Own/peer predicted-head disagreement, units | 298 | 0.18 | 1.28 | 32.81 |

These dependent join/staging entries are not independent trials or qualified
geometry. Protocol predictions are not simultaneous authoritative positions.
No contact offset, sustained hold, candidate inconsistency count or half/base/
double timing-bound comparison is identified on the empty qualified set.

## Integrity, source pins and limits

All five corrected scouts audited as verified. Pilot one is quarantined for
interrupted acquisition/failed child; the planned second attempt remains absent.
Both raw members have complete gzip, valid SHA-256 prefix receipts, unchanged
sources and matched result ledgers. Bundle verification preserves quarantine;
checksums do not establish scientific validity. Packet census rejected the
interrupted members. Maximum game-socket concurrency was two; closes balanced.

Pre-fix acquisition used `d21d90b84069242c2b2d7d58c98b8103dd4adf08`; corrected
scouting/pilot used `42ef95eec464cf0f3fa142ac9f572127deda8e5d`. The frozen
[plan](plan.json) was preserved. Raw archives and operational provenance remain
local-only outside Git; neutral aliases and hashes identify them without
publishing private details. Earlier failures remain retained and labelled.

Fixed-source checks: 7 unit tests passed; the first full offline cycle qualified
10/12 and failed coverage, then one unchanged full repeat qualified 12/12 and
passed. Both synthetic runs are retained; the deterministic curve passed.
The result is one dependent failed pilot, with unseen-third-party and body/
timing uncertainty limits. No p-values, held-out validation, simulator change
or new game connection was made for this publication. Verified copy-back
preceded scratch cleanup; no acquisition process or task container remains.

## No second design yet

No v2 official pilot is planned. A scratch design review found two problems
that a retry would not fix:

- **The fine comparison is unidentifiable.** The frozen position floor alone
  allows at least 4 units of relative perpendicular displacement at base scale
  (2 at half scale). H0's `0.975 * r_body` and a `1.00 * r_body` band differ by
  about 0.37 units at the observed 2–5 segment sizes and 0.63 units at 80
  segments. At newborn sizes the H1 bands are also only a few units apart.
  Capsule versus point-band contact might still separate on isolated geometry,
  but that needs an offline uncertainty screen first. Do not shrink the
  allowance to make a candidate win.
- **Acquisition needs a new method.** Pilot one started the next pair 0.6–25 ms
  after both prior sockets closed, which made 13 connection attempts in the
  first minute. The last attempt closed 0.5 s after connect, before any open or
  frame. A same-address rate limit fits that but is not established. Third-party
  exclusions happened while the own snakes were still 2.4–7.7 thousand units
  apart, so every restart discarded their approach. The one pair that staged
  cruised straight at about 181 units/s for 27.6 s until it crossed the 900-unit
  rim margin of a 6,953-unit arena.

A v2 would need bounded, persistent staging, paced admissions and a coarser
question, validated offline before any official pilot. It must keep the
600-unit guard and this failed attempt visible.
