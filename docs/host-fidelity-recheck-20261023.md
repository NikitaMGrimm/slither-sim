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
old defects. The separately frozen R4 follow-up below achieved one route leg;
continuous geometry, avoidance, alignment and held-out coverage remain analysis
requirements, and it recovered no numerical subscription rule.

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

## Independent minimap recheck

A later offline recheck decoded every minimap in these same four completed R3
archives using the two retained independent Python decoders. They agreed on all
737 maps: 451 full `M` and 286 XOR `V`, each 136×136. All 20 connection/setup
sequences started with `M`; no delta lacked a base. The 717 within-connection
server-clock gaps span 2,499–2,509 ms, with median 2,504 ms. No maps followed the
16 observed deaths; the final connection in each stream is right-censored.
These are correlated recipient observations from two acquisition batches.

The analyzer SHA-256 is
`f205186e9da856e11645083cd34c0037a628df7c4895ed651de76d815b7e6a34`.
Its dependencies were pinned before raw opening. Six Python smoke checks,
all-map dual decoding, archive hashes and the frame/packet/epoch census
reconciliations passed. The report and original packet/bitmap-hash witnesses are
retained under `outputs/qualified-crossings-20261023-r4/minimap-agent`.

Source inspection found native support for every received map form and correct
connection-base resets. This recheck ran no native code. It supplies no functional
minimap correction and does not identify full-world raster filtering, tiny-snake
treatment, boundary pixels, first-report phase or the official full/delta choice.
The original 134-map comparison remains a separate preserved result.

## Corrected-collector follow-up (R4)

The next study, `visibility-qualified-endpoints-20261023-r4`, froze its collector,
analyzers, seven instantaneous/48 temporal candidates and scientific gates before
opening new inputs. Its plan SHA-256 is
`0494909aadb6cdc2665701e5cbd172ca457311598622c41e0a706cb766b81e50`.
The changed acquisition condition was the endpoint-accounting correction, rather
than a different visibility threshold.

One diagonal-positive pair completed 480 seconds and passed gzip, capture-end,
source/provenance and batch-membership checks. Its two archive SHA-256 values are
`764972a11f825a908a8c5250cb2ac4d8b7d9391c706fcdbebcc9d8df676b0ac2`
and `1a2d81117ac0a674829ce7897ad5a102c1f042cc5823c7d054fb5270a5ab7260`.
The pair contains 96,418 incoming frames and 218,309 logical packets across
30 command families. Both observers belong to one acquisition batch.

The cardinal invocation exited unsuccessfully immediately afterward, leaving two
empty archives and an empty log. Neither contains a join or usable observation;
the cause remains unidentified. Both are quarantined. The frozen stop rule ended
acquisition, so neither extension pair was invoked and no replacement was made.
The offline paired-CLI signal/drain regression passed afterward; it does not
explain that failed invocation.

Four completed endpoint witnesses were recorded, separately from four initial
approaches. Three completed legs were excluded for recorded avoidance. One
5,317.3629 ms far-to-near leg passed the sampled geometry/continuity screen.
The frozen chronological split consequently has zero calibration legs and one
held-out leg. Of its 50 query opportunities, 17 fail own-pose freshness and 33
fail the original packet-based clock-alignment window. Both entity classes have
zero eligible query/transition denominators; no candidate was selected. Cardinal,
size and boost validation are also absent. A completed route does not itself
validate an entry distance or retention law.

The unchanged food analysis gives these separate recipient results:

| Recipient | Classifier disagreements / exposures | Schedule-proxy disagreements / exposures | Screened entries outside classifier |
| --- | ---: | ---: | ---: |
| bot0 | 6 / 3,504 | 91 / 3,504 | 54 / 351 |
| bot1 | 59 / 4,304 | 129 / 4,304 | 49 / 372 |

Seven selected witness entries across five raw frames were verified. Examples
include sector removal at predicted last-touch ages 0 ms and 24 ms, and retention
at 2,605 ms. Counterexamples persist in the history-continuous stratum. These are
conditional discrepancies under the received-pose reconstruction, not native
execution failures, independent trials or a recovered cleanup timer. Empty/absent
subscription denominators and food calibration/held-out coverage remain missing.

The endpoint adapter SHA-256 is
`f9c29f80e1218ab49062becaf62db2d9008217323b27a25483745bc8c2504aa9`;
24 synthetic checks passed before raw opening. The food adapter SHA-256 is
`2fcccc22a7b2aad3dd57dafb6e2dc04af7d9cd2f2667aea2ed7cbeea854bd297`;
11 checks passed before input inspection. Original R3 and older counterexamples
remain unchanged. Full sources, archives, main audit/stop receipts, exclusions,
packet summaries and both analysis reports remain outside Git under
`outputs/qualified-crossings-20261023-r4`.

No numerical production correction follows from this stopped phase. In
particular, a 300-unit wire-sector grid or the simulator's provisional 75-unit
food-admission cells does not identify the recipient's subscription geometry,
cleanup scheduler or size scaling.

## Exploratory common receipt-time replay

A separate retrospective derivative checked whether the valid R4 leg contained
useful received-cache comparisons without requiring nearby byte-identical packets
to align the two connection clocks. Source and provenance establish that both
clients shared one Node process, capture batch and `performance.timeOrigin`.
The original 33 alignment windows lacked shared-packet brackets; no populated
window showed offset disagreement. The original frozen zero-query result remains
unchanged; this derivative uses a different, explicitly exploratory query grid.

At 27 grid times, spaced 200 ms across the same half-open leg, each stream was
replayed only through packets already received at that time. Position, body and
motion field ages remain separate. Fixed freshness, reference history, identity,
transition and retrospective survival screens leave 154 correlated object/query
exposures at 11 grid times: 113 snake and 41 prey rows, with 21 present and
133 absent local-cache labels. All qualified prey rows are local-absence cases.
Three snake transition receipts qualify, including one nondeath exit/reentry;
no prey transition qualifies. These are received-cache observations against a
positive peer reference, not simultaneous authoritative world states.

The plan acknowledges earlier raw inspection. It was frozen with the executable,
tests and dependencies before additional raw replay. Nineteen synthetic tests
passed, and saved-output verification checked 5,693 field-prefix/age assertions.
The parent independently reran those tests and reproduced the saved checks without
reopening the recordings. The plan SHA-256 is
`f35e7f5cff77c9514e4d22b81816a72eb8c26109f739e7c9e9bdd5748205eb68`;
the replay SHA-256 is
`5d3467146ae78fd36a391436d929573bf89d42695115475f3c64d557841bef3a`.
Full source, pins, rows, exclusions and packet witnesses remain outside Git in
`outputs/qualified-crossings-20261023-r4/analysis-agent/clock-design/receipt-exploration`.

Later event/continuity receipts supply guards and survival selection only; they
never fill earlier coordinates or omitted fields. The selection can be biased
toward retained episodes. Body geometry uses the pinned historical reconstruction;
receipt age does not bound hidden serialization delay. Calibration remains empty,
cardinal acquisition absent, and repeated-transition, size and boost coverage
inadequate. No threshold or timer was scored, fitted or selected, and no numerical
subscription correction or production validation follows.
