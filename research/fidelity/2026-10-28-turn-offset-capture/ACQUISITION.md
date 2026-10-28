# Acquisition implementation frozen before requests

The original [plan](PLAN.md) and [candidate predictions](CAPTURE-PROTOCOL.json)
remain unchanged. This note makes implementation choices explicit before discovery
or game traffic; it does not authorize a future run.

- `guard.ps1` serializes acquisition under an exclusive file handle, screens other
  possible Node/Python collectors and existing connections to the bound server,
  and repeats those checks with three-second polling sleeps. It reserves each ordinal
  with a flushed append **before launching** the capture process. A launch failure
  consumes that ordinal. It never stops somebody else's process.
- `discover.mjs` makes one inert list request and calls the existing authored
  decoder. All discovery/endpoint inputs and outputs are external. There are no
  discovery-time game sockets or fallback endpoints.
- `capture.mjs` subclasses the existing headless Bot only for acquisition steering
  and packet metadata. Received `World.packet` applications, 25-ms control pacing,
  outgoing heading throttle, login, reconnect, recorder and avoidance stay with
  the original collector. A socket-set admission guard includes connecting and
  closing sockets. Cruise-only intent applies to both clients, including forage.
- Receipt-age/positive-cache conditions are checked at every control tick. A
  changed count, new life, safety diversion, control older than 75 ms or stale pair resets the current
  interval. Counts passed in a life are retained as missed. Acquisition completion
  is provisional until pinned-client reconstruction; it is never itself a score.
- The active capture stops ten seconds before its 1200-second outer deadline to
  reserve time for closing sockets. The outer guard can terminate only our child
  at the hard deadline. Failure to close stops the study and prevents another run.
- The original triplet gate admits three consecutive neighboring triplets before
  accumulating its center triplet. At least five accumulated centers are needed.
  Duration is the sum of half-neighbor-gap durations, matching the original study's
  midpoint convention. No curve fit or candidate error participates in selection.

## Native reconstruction and positive observer agreement

Offline analysis reuses the committed [count-phase native harness](../2026-10-28-turn-count-phase/harness/src/main.rs)
and its locked Git dependency on client `8d0772c015d6b9b73b80253db8c7349aa600b201`.
Every complete incoming frame is validated in original order. Server elapsed
headers define durations; receipt time is only for causal pairing/control gates.
Logical count, physical anchors, dying points, fam and displayed length are retained
separately. Full exported rows remain external.

The offline selector uses only the most recent preceding control, at most 75 ms
old, and conservatively ages its cached poses forward to the anchor receipt.
It also requires one second of count stability before/after the candidate arc.
Interruptions reset selection. A first full eligible arc is retained even when
observer agreement fails; a later arc cannot replace it for a better model fit.

For each selected actor anchor and its two neighbors on either side, require one
positive observer anchor for that actor ID within 300 ms receipt time and distance
`sqrt(8)` world units, with identical wire logical count. The spatial allowance
covers separate absolute/grid rebasing; it is an engineering qualification screen,
not a fitted spatial-noise model. Ambiguous/missing matches or incompatible counts
fail agreement. The observer is not a replicate. Actual received count is scored.

The archive audit checks exact JSONL-prefix integrity, both direction counts,
source stability, common batch/time origin and absence of boost requests.
Interrupted or incomplete archives cannot qualify a held-out invocation. Primary
development coverage counts only cells with positive observer agreement and intact
completed archives. No held-out launch is possible without an external gate that
names the committed analysis checkpoint; the decision must still be reproduced
from all retained archives afterward.

Source hashes are recorded in `SOURCE-FREEZE.json` before any request. The
development/held-out boundary permits documented implementation corrections only
as the original plan allows; no candidate, duration, gate or decision-rule tuning.
All failures, stdout/stderr, process outcomes and raw traffic stay external.
