# Official food-interest pilot: failed feasibility, 2026-10-26

**P3 failed its frozen pilot gate: three completed slots against a minimum of
four.** The single-client route completed A/positive, A/negative and B/positive,
then aborted B/negative for avoidance. The frozen route allows no new site after
an abort, so the collector ended there: one avoidance event used up the pilot
after 33 of its 480 seconds. Slots 5–8 remain missing. The stop rule prohibited
pilot 2 and all development/held-out invocations: one of six planned invocations
ran, with no retry, replacement site or endpoint substitution. A later attempt
would need a new frozen plan that restages after interference, as the
[collision pilot](../2026-10-26-collision-pilot/EXECUTION.md#no-second-design-yet)
also concluded.
Status: **exploratory and unreviewed**. No food-interest model, retention rule or
local-replacement effect was fitted; **no simulator change is recommended**.

## Measured acquisition and gate

The unchanged protocol-19 collector used source revision
`7580d53baffa35d776af4fc22a9926220d33c672`, with a 480-second ceiling and one socket.
An ordinary home route is allowed by the plan. Selection used the first active
port-444 entry in a fresh official list fetched with the required HTTP Origin;
no game probes preceded selection. All 515 files in the planning freeze matched.
Only `capture.server` changed during private binding; `capture.origin` remained
`http://slither.io`. The normal evidence initializer froze those bound bytes
before the sole invocation.

| Frozen pilot condition | Result |
| --- | --- |
| ≥4 completed slots, including both lanes/directions | **Fail: 3**; B/negative unfinished |
| ≥10 qualified known-sector listings/clears | Not evaluated after decisive slot failure |
| ≥5 unambiguous clear→positive-reintroduction episodes | Not evaluated after decisive slot failure |
| ≥20 eligible 200 ms queries | Not evaluated after decisive slot failure |
| Development/main coverage and held-out improvement | Not reached; no invocations or scored candidates |

The three completed passes lasted 7.472, 7.736 and 7.464 seconds. Their received
endpoint distances were 34.42, 41.30 and 22.24 units, within the collector's
55-unit tolerance. Slot 4 aborted 4.844 seconds after starting, with 593.31 units
remaining to its endpoint. These are acquisition diagnostics; this publication
does not certify the additional causal freshness, continuity and identity screens
needed for qualified food-model exposures.

The archive contains one connection, one join, 2,117 incoming frames and
188 outgoing frames. Across **all phases**, it contains 154 `F` packets with
1,328 food entries, 120 `f`, 113 `b` and 97 `w` packets. These measured delivery
counts do not establish qualified subscriptions, physical births or refill.
The remaining gate denominators and candidate errors are **unassessed, not zero**.
Pilot data never become fitting or validation data. All 32 frozen candidates,
including 45/100/2.6/504, remain unscored. Maintenance phase and uncertainty
sensitivity were not estimated; the replacement contrast is undefined.

Joined receipt time through route stop was 32.798 seconds. The connect event to
route-stop interval was 33.052 seconds. The entire durable invocation lasted
33.440911 seconds, a conservative upper bound on total official connection time
including connecting/closing; the collector does not record exact socket-close
time. Its 300 ms close drain and child exit completed. A subsequent operating
system check found no task process or endpoint TCP socket. No container or
remote process was created.

## Integrity, preservation and reproduction

[Selected results](selected-results.json) retain the slot ledger, phase witnesses
by JSONL line, neutral archive alias, source pins and exact archive hash. The
member audit is **verified**, with no issues or warnings. The study and packaged
bundle retain **incomplete: planned_attempts_missing**, because five invocations
are deliberately unrun. Child exit zero and archive integrity do not pass the
scientific gate. The collection wrapper returned 2 for that incomplete study.
Standard packet census and transport measurement completed; the unfiltered
transport-reference comparison failed. It neither changes the feasibility
decision nor identifies a server timing law. No transport tolerance was relaxed.

[Retained hashes](retained-hashes.json) identify the local-only raw archive and
normal evidence bundle. Access is restricted: neither is downloadable here, and
no additional sharing rights are asserted. Endpoint-bound plans, official-list
bytes, operational provenance and full packets remain outside Git. Hashes check
consistency, not server authenticity, prospective registration or scientific truth.

The [published plan](03-food-lanes.json) is non-executable: `capture.server` is
removed and its documentation-address placeholder is withheld from `method`.
All other scientific fields are preserved. Its `.source.sha256` pins the original
frozen bytes; `.sha256` pins the publication copy. [Source pins](source-pins.json)
identify the retained tools and locked environment.

With access to the retained study and pinned export, reproduce the selection
without network access (run in the locked tools Python environment):

```sh
python -B analyze_pilot.py --study /retained/study --source-root /retained/pinned-source --check selected-results.json
python -B -m unittest discover -s . -p test_analyze_pilot.py
```

The selector supports this decisive failed-slot outcome only; it refuses to
declare a pilot passing when four slots complete. Synthetic tests check that
boundary and preserve missing slots and unassessed denominators. This pilot
does not contradict or refine a documented food law, so server rules and
simulator behavior remain unchanged. A later acquisition would need a separately
authorized protocol; the stopped schedule cannot be resumed as replacement data.
