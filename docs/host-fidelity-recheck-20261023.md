# Paired visibility recheck, October 23

The completed recordings challenge the current food-interest model and expose
crossing-collector accounting defects. They do not identify a replacement online
subscription algorithm. [Host fidelity](host-fidelity.md) describes the current
engineering choices; no physical default or subscription constant changes here.

## Acquisition and source identity

Two earlier one-client preflights retained zero incoming frames. A later
12-second preflight with explicit HTTP Origin joined and recorded 1,225 incoming
frames. The historical collector supplied that Origin; the earlier attempts had
omitted it. A loopback handshake check confirmed the current runtime sends the
configured header. Success under the changed condition does not isolate the cause
of every earlier handshake failure. All failed attempts remain retained.

The frozen follow-up acquired one diagonal-positive and one cardinal-x pair,
480 seconds each, on the same official endpoint. All four archives passed gzip,
capture-end integrity, membership and source checks. They contain 206,053 incoming
frames and 529,090 logical packets across 30 command families. These are two
acquisition batches; observers, lives, entities and packet intervals are nested
and correlated.

| Run / recipient | Archive SHA-256 |
| --- | --- |
| diagonal-positive / bot0 | `eb1c73c332df2454768d6474c129f872737ccdd8cbe8914202cb038307005e5b` |
| diagonal-positive / bot1 | `4a8a542ada410a33ed4185f6c525463bb7ef2c60b758e4eb63a46f521de5d2ea` |
| x / bot0 | `2018e4b4472eb12c78b8cc0086dc6b34e462e8c3fe59b4b0a70139d762967383` |
| x / bot1 | `a8644724bc53c19bc8518bac3ac514f3623dba87da5860b38251c05efa657a26` |

The pre-acquisition amendment SHA-256 is
`2efd80b8639e9d2fcee42aa4b4d891b65e93376f7bef8f738cd155a90d7146e1`.
Collector source snapshots remained fixed during acquisition. Full recordings,
frozen plans/analyzers, raw failure witnesses and stage receipts are retained
outside Git under study `visibility-origin-preflight-20261023-r3`, in
`outputs/fidelity-pass-20261023/origin-study`. Reproducing the full analysis needs
those artifacts; the repository does not distribute the full corpus.

## Snake and prey crossings

Seven instantaneous and 48 temporal candidates were frozen before analysis, with
the retained clock alignment, source-age, incarnation and continuity screens.
The analysis adapter SHA-256 is
`5fa886e439e3f1565da5d735376ef6b4471cc2a81e8189a9befb424e69e16e51`.
Twelve synthetic checks passed. Standalone and combined evaluations reconciled;
the adapter and scientific gates were unchanged after raw opening.

The diagonal run had eight route-counter endpoint arrivals and 1,568 shared
alignment anchors; cardinal had eleven arrivals and 2,475 anchors. **Neither run
had a complete leg satisfying the frozen endpoint/reference geometry gates.**
Both classes therefore have zero eligible calibration and held-out present/absent
denominators. No candidate was selected. The 10,043 diagonal and 12,301 cardinal
incidental geometry queries outside eligible legs were not substituted for that
missing experiment.

The collector advanced on B's endpoint alone even when A was displaced or stale,
and counted the initial partial approach. For example, cardinal trial 6's
103,056.163–107,254.136 ms leg reached B's near endpoint while A's anchor was
200.9 ms old and its reference-geometry flag was false. The collector correction
now requires the study's endpoint geometry/age gates, distinguishes the initial
approach, and logs exact endpoint witnesses. Offline regressions reproduce both
old defects. No subsequent acquisition validates the corrected route's yield;
continuous geometry, avoidance, alignment and held-out coverage remain analysis
requirements.

## Food model counterexamples

The unchanged 45-sector/100-unit-lead/2,600 ms diagnostic classifier was evaluated
separately from a frame-sampled proxy of the host's 2.6-second retention and
504 ms cleanup schedule. The proxy fixes phase zero at received setup and does
not execute every physics tick. Its results are not native Rust validation.

The food adapter SHA-256 is
`d429178c9ba25252e3f19492fb0630cc24c324275fafbe1cb3c96ea7bb058389`.
Sixteen dependencies were pinned. Eight synthetic checks passed, and the old
34/3,584 disagreements reproduced unchanged. An identity-bookkeeping correction
was recorded before new raw decoding: the paired driver and imported headless
module have separate already-frozen hashes. Original code, freezes and results
were preserved. No candidate, phase or scientific gate was retuned.

| Run / recipient | Classifier disagreements / retention exposures | Entry receipts outside classifier |
| --- | ---: | ---: |
| diagonal-positive / bot0 | 42 / 3,731 | 86 / 311 |
| diagonal-positive / bot1 | 37 / 3,003 | 60 / 314 |
| x / bot0 | 24 / 3,054 | 76 / 334 |
| x / bot1 | 20 / 3,893 | 72 / 335 |

These are correlated known-sector membership/removal exposures, screened using
the original construction: live own snake, warmup, fresh anchors, at most
80 segments and same-frame reintroductions excluded. They do not supply absent
or empty-subscription denominators. The separate schedule proxy disagrees on
555/13,681 exposures. Neither that sum nor the classifier's 123/13,681 defines
whole-host accuracy or an independently validated error rate.

Sixteen selected witness entries were checked against raw frames. Examples:

- Diagonal bot0, connection 1, server 19,157 ms: sector `(109,105)` arrives with
  `F` despite never being touched by the predicted stencil in this life. Own
  size is seven segments and its head anchor is 56 ms old.
- Cardinal bot0, connection 1, server 172,711 ms: `w` removes `(109,108)` only
  157 ms after predicted last touch.
- Cardinal bot0, connection 6, server 15,275 ms: known sector `(107,119)` remains
  retained at predicted last-touch age 3,621 ms.

History-continuous screening retains counterexamples. The causes remain
unidentified: reconstruction, notification lag, scheduler phase, every-tick
geometry and size effects cannot be isolated here. Qualified food calibration/
held-out legs and hidden subscription states remain unmeasured. These results
support preserving the model limitation, not installing a new guessed threshold.

## Reporting cadence

Per-connection census summaries broadly reproduce the earlier approximately
2.5-second minimap, three-second leaderboard and 338 ms median prey cadence.
Whole-connection summaries are descriptive; they are not uniformly stratified by
own-life state. The following overlapping reports are one case, not independent
observations:

| Recipient / connection | Same-prey `j` pair, server ms | Raw packet pair |
| --- | --- | --- |
| diagonal bot0 / 1 | 103,824 → 105,438 | `6aa1a72a622af93fffff` → `6aa1a72a622b45313ed184c123421809` |
| diagonal bot1 / 2 | 25,098 → 26,712 | identical pair |

Prey ID 41383 has no intervening same-prey update or incarnation reset in either
recipient. The gap is 1,614 ms; other incoming frame gaps in that interval are at
most 37 ms. The raw endpoints/order and difference were independently checked.
Thus the earlier fixture's extrema are corpus-specific, and one third second is
not a universal reporting upper bound. Local periodic corrections remain an
engineering choice that prevents unbounded client prediction drift. No new
reporting timer or physics rule follows from this outlier.
