# Offline validation and limitations

All results here are **synthetic engineering evidence**. No official collision
law follows from them. [All attempts and archive hashes](offline-results.json)
are retained, including the failed recordings.

## What the geometry can identify

A long straight body translated along its own axis has the same lateral
contact band. Rewinding it by speed times delay does not produce the proposed
lateral shift. The study instead passes perpendicularly behind a finite tail;
the offset along the reference's axis is lateral to the probe's trajectory.
For the specified peer-only rewind, that end moves backward by approximately
`reference_speed * added_one_way_delay`. A fixed display delay cancels between
arms; it is not an additional measured arm difference.

The production-history scorer exposed a discrete-anchor limitation. At 147 ms
(21 simulator ticks), one anchor phase showed a shift and another hid it.
At the frozen 12 observations per arm and four-unit floor, the shift envelope
was **[-88, 48] units**: inconclusive. The study therefore chose 450 ms in each
wire direction. Its synthetic approximation is 448 ms (64 ticks), differing by
0.36 units at the synthetic 181.527 units/s cruise speed.

With the frozen fixed ladder and both anchor phases:

| Synthetic model | Samples per arm | Predicted shift | Shift identification interval, base floor | Detector result |
| --- | ---: | ---: | --- | --- |
| No rewind | 12 | 0 | [-68, 68] | Current state compatible; specified peer rewind excluded |
| Reference-only rewind, 448 ms | 12 | -81.324 | [-148, -12] | Specified peer rewind detected; zero excluded |
| Both states rewound, 448 ms | 12 | 0 | [-68, 68] | Indistinguishable from current state |

The same conclusions hold at two- and eight-unit floors. These are deterministic
identification intervals over a synthetic model, not confidence intervals or
an empirical power guarantee. The no-rewind negative control reuses the same
simulated histories with the rewind scorer disabled; it is not an independent
acquisition. Whole-state rewind remains a counterexample to a universal
"no compensation" inference from an unchanged boundary.

## Wire acquisition, failed attempts and audit

Seven complete development runs are retained, four scheduled encounters each:

| Run | Collector outcome | Standard audit / selection |
| --- | --- | --- |
| local-01 | Four staging timeouts; rim interruptions retained | Quarantined; source changed during development and record clocks regressed |
| local-02 | Four contacts, no misses | Quarantined; clock regression and failed coverage |
| local-03 | At 150 ms, one contact and one miss per arm | Quarantined; clock regression |
| local-04 | At 450 ms, delayed arm had two contacts | Quarantined; clock regression and failed coverage |
| local-05 | At 450 ms, one contact and one miss per arm | Quarantined; clock regression |
| local-06 | Same required coverage | Quarantined; final raw frame count omitted queued undecoded ingress |
| local-07 | Same required coverage | Both members verified; selected engineering run |

The staging fixes let the probe shed excess lead and wait for a straight
reference body before choosing the finite-end approach. A transient rim-margin
episode in the final delayed contact was recorded and restaged with the same
pair; four interruption messages describe that episode, not four independent
encounters. The 600-unit body-edge guard has a synthetic persistence regression.

The recorder fixes preserve actual socket ingress/egress times: applying queued
ingress does not record it twice; egress timestamps are taken when sending;
terminal raw counts include frames still waiting for delayed decoding.
Focused regressions cover monotonic record clocks and raw counts. Old bytes and
quarantine statuses were preserved. Historical collector source bytes were
reconstructed after development and verified against each collection-time
SHA-256; this does not repair an invalid recording.

An early nominal-offset comparison would have been a false positive: at a
requested -120-unit offset, the current-state localhost host produced a control
miss and delayed contact. Independent raw replay put their achieved intercept
ranges around [-112, -73] and [-19, 15] units. The controller's delay changed
the trajectory; it did not show a shifted server boundary. The frozen analysis
uses achieved packet-derived ranges and preserves their uncertainty.

Raw replay consumes only each stream's prefix at a shared monotonic time.
It extrapolates each head from its latest anchor, then projects the remaining
straight crossing relative to the observed tail. It retains the full intercept
range over the -100 to -20 transverse-unit window. Reported anchors,
extrapolated heads and simultaneous authoritative positions remain different
objects. The empirical plan additionally expands by wire-RTT uncertainty;
synthetic controls have exact histories and no network RTT.

The local process trees were restricted to two logical CPUs, with two Cargo
jobs and Rayon threads. This was a shared desktop: a 10% total-CPU sample and
concurrent compilation contaminated timing. The final run's wire RTT p95 was
50.506 ms. It validates plumbing and coverage, not an idle-machine latency
baseline. Official scouting used the separately capped remote container.
