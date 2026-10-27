# Collision latency compensation: acquisition gate failed

**Verdict: inconclusive.** No near-zero-population server was found among the
27 active entries in the retained official list. The 26 observed leaderboard
totals ranged from **61 to 483**; one connection closed before setup. The
frozen gate therefore stopped collection before any controlled encounter.
This study answers neither current-state collision testing nor latency rewind.

| Official arm | Planned | Attempted / launched / qualified | Contacts / survivals |
| --- | ---: | --- | --- |
| Control, no added delay | 12 | 0 / 0 / 0 | 0 / 0 |
| Probe delayed 450 ms each direction | 12 | 0 / 0 / 0 | 0 / 0 |

The boundary-shift estimate and uncertainty interval are **unidentified**, not
zero. There is no p-value, online move/test phasing result or simulator adoption.
`docs/server-rules.md` and simulator rules are unchanged.

## Design and offline evidence

The [plan](plan.json) was frozen at 2026-10-27 09:29:51 UTC, before the first
official game connection, with SHA-256
`bb8c71cd943a94c98c3003b4c43f6fba67fd80a14a7f6239ff11ac81cc5b7d71`.
Every scout pins that same plan hash and unchanged collector sources. The
collection source base was `20e5064`; later main changes do not relabel it.
The retained discovery response and client decoder source have separate hashes.

The owner authorized this official-server attempt specifically at near-zero
other-player counts. The [earlier pilot's failures](../2026-10-26-collision-pilot/EXECUTION.md)
were read and retained: this plan uses a much stricter population gate, paced
admissions, persistent staging, inward restaging and the same 600-unit
third-party body-edge guard. It does not retry the unidentifiable fine-radius
comparison or infer anything about 0.975 versus 1.00 body radius.

The [offline record](OFFLINE.md) documents a production loopback host, ordered
delay queues in both directions, raw replay and standard audit. All seven
development runs remain visible, including staging/coverage failures and
quarantined recorder defects. The final run has two verified archives and one
contact and one miss in each arm. Desktop CPU contamination is recorded;
localhost timing is not an official latency baseline.

At the planned **12 synthetic observations per arm**, the post-hoc production
history model detects the specified approximately 450 ms **reference-only**
rewind: predicted shift -81.324 units, identification interval [-148, -12].
The no-rewind negative gives [-68, 68]. Half/base/double position floors retain
those classifications. These synthetic intervals are not the official result.

Two design findings constrain future inference. A tandem pass alongside a
straight body has no speed-times-delay lateral shift; this design uses a
perpendicular pass behind a finite tail instead. A rewind of **both** constant
trajectories is indistinguishable from current-state testing. Fixed display
delay cancels in the difference between arms. The smaller approximately 150 ms
treatment was inconclusive across synthetic anchor phases, so the plan chose
450 ms. These findings concern experiment identifiability, not measured server
behavior or a correction to the simulator's collision model.

## Official execution

The decoded response contained 141 IPv4 entries, 27 marked active. All 27 active
entries were scouted once in decoded order from an ephemeral remote container
capped at two logical CPUs, 768 MiB and 128 processes. No existing container
was modified. The remaining 114 inactive entries were not probed. Their status
does not establish whether a near-empty official server could be reached there
or at another time.

Eligibility required a first leaderboard total at most six, including the
scout, and at most five visible other snakes. Server bots count as third parties.
All observed totals failed the population gate. In particular, one scout saw
no cached other snake while its leaderboard reported 480: an empty local view
was not substituted for a low population. There was no endpoint substitution,
threshold relaxation, paired invocation or replacement scouting pass.

Raw lifecycle reconstruction verifies **27 connections and 27 closes**, maximum
concurrency **one**, minimum connect spacing **8.404 s**, and at least **8.004 s
after the preceding close**. The early close was followed by **30.009 s** of
back-off. `Origin: http://slither.io` was configured in the pinned collector;
raw outgoing bytes contain no ping before setup. The one early close has no
identified cause; it does not establish a rate limit. The task container was
removed and no acquisition process remains.

[Selected execution results](execution-results.json) retain every scout's
population, visible-cache count, outcome, close code, audit status and archive
hash, plus every zero encounter cell. Independent raw decoding confirms the
first leaderboard totals. Twenty-six members pass the standard capture audit;
the early-close member is quarantined for `no_gameplay_setup`. Its gzip and
prefix receipt remain intact. Raw bytes, sent bytes, source exports, the frozen
plan, discovery response, private transfer archive and full audits are retained
outside Git; access is local-only. Published hashes identify those records
without providing a public download or exposing endpoints or infrastructure.

## Verification and remaining limits

The complete tools check group passed: Python tooling/fidelity suites, 265 Node
passes with two expected opt-in skips, transport/bot/C ABI smoke checks, and
35 wheel tests with one existing skip. The explicit new loopback run passed;
the production counterfactual example's two tests, Clippy and formatting passed.
After collection, the publication branch was rebased onto current main; its
tooling checks were repeated without refreshing the historical plan/source pins.
There was no independent agent review in this research sub-agent task.

Population snapshots, a recipient cache and one short scouting session cannot
prove permanent availability or global absence of other snakes. Synthetic
power checks and valid recording receipts cannot establish official mechanics.
The requested controlled comparison remains uncollected. A useful next attempt
requires identifying an authorized near-zero endpoint or time window first,
then freezing a new acquisition plan with these failed scouts retained.
