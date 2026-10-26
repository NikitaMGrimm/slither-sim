# Official movement scheduler, 2026-10-26

Six frozen 600-second invocations completed with one client on one official
protocol-19 endpoint. All six archives verified, both coverage gates passed
(three development, three held-out), and all 24 unfiltered transport gates passed.
There were no replacements, quarantined analyzed archives or unexpected lifecycle
events. These are temporal replicates on one endpoint, not independent servers.
Status: **exploratory and unreviewed**.

The observed wire gaps contradict literal report floors: **791 motion gaps
below 35 ms and 193 standalone `h` gaps below 100 ms**, including **451 and 107**
in held-out ordinals 4–6. **No replacement scheduler was selected**: authoritative
dirty and input-application instants are unobserved, so the frozen prediction
metrics are undefined. This is missing identification, not rejection of all
internal schedulers. No rule, release model or runtime code changes.

Collection used revision `7580d53baffa35d776af4fc22a9926220d33c672` and the
unchanged standard movement controls. The prior
[visibility pilot](../2026-10-26-visibility-pilot/REPORT.md) failed its frozen gate;
the protocol then switched to this separately authorized P2 study. Publication
was entirely offline, without new captures or game-server connections.

## Per-invocation coverage

Each full eligible batch requires at least 300 joined seconds, 20 isolated changed-heading holds, four qualifying boost transitions and 50 credited pickup/mass pairs.

| Ordinal | Split | Member audit | Connections / joined lives | Joined seconds | Isolated holds | Qualifying boost responses | Pickup/mass pairs | Coverage |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | development | verified | 8 / 8 | 571.215 | 939 | 28 | 1566 | PASS |
| 2 | development | verified | 5 / 5 | 583.676 | 1079 | 28 | 1947 | PASS |
| 3 | development | verified | 6 / 6 | 579.334 | 1037 | 30 | 1804 | PASS |
| 4 | heldout | verified | 8 / 8 | 570.828 | 932 | 47 | 1653 | PASS |
| 5 | heldout | verified | 8 / 8 | 571.264 | 997 | 36 | 1664 | PASS |
| 6 | heldout | verified | 9 / 9 | 567.038 | 898 | 34 | 1710 | PASS |

Joined exposure runs from setup to own death or capture end; reconnect/post-death waits are excluded. All lives remain in their original invocation. Isolated holds require at least 150 ms, prior wanted target over 2.1 angle-byte units away, and the first exact `floor(byte*256/251)` wanted reflection before any later heading command. Every command, including unchanged, superseded and censored commands, remains in the full ledger. Qualifying boost responses require directional speed reflection without intervening avoidance or pickup/growth/tail confounds. Credit pairs follow explicit own-eater identity and raw packet order to the next own mass-bearing update; multiple correlated credits may share an update, and censored credits are retained.

| Split | Started | Audited summaries | Fully eligible | Motion eligible | Mass eligible | Required two eligible |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| development | 3 | 3 | 3 | 3 | 3 | PASS |
| heldout | 3 | 3 | 3 | 3 | 3 | PASS |

Actual start-to-start intervals (s): 1801.347, 1801.354, 1801.563, 1801.548, 1801.417. All observed intervals meet 1,800 s. No game socket was opened during the spacing waits. The original audit reports `planned_attempts_missing` while fewer than six ordinals have started; this does not quarantine an individually verified archive.

## Frozen mandatory diagnostics

Counts use cumulative server elapsed headers within each setup/connection and reset per snake incarnation. Packet order is retained. Coarse receipt masks remain separate and do not certify clean 5-40 ms timing.

| Ordinal | Motion gaps <35 ms | Standalone h gaps <100 ms | Reverse same-frame mass/motion | Unchanged decoded tuple | Tuple and encoded direction unchanged |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 123 | 27 | 2 | 165 | 133 |
| 2 | 137 | 24 | 0 | 89 | 64 |
| 3 | 80 | 35 | 1 | 71 | 62 |
| 4 | 106 | 31 | 0 | 96 | 69 |
| 5 | 142 | 28 | 0 | 120 | 91 |
| 6 | 203 | 48 | 0 | 89 | 71 |

Full per-field masks, turn direction, prior heading command/hold, boost state, phase, growth/tail/credited-pickup age and exception context are preserved in each `ordinal-NN-p2-analysis-final.json`. Raw witness references include archive SHA-256, JSONL line, connection/life/incarnation, server/receipt clocks, packet index and bytes. The earliest witnesses were independently cross-checked against raw lines, and each motion denominator was checked against the standard packet census. Unchanged decoded tuples do not identify an unchanged higher-precision authoritative state or a hidden dirty latch.

Server gap cells below are **count / minimum / nearest-rank p50**, in milliseconds.

| Ordinal | Own motion | Other motion | Own standalone h | Other standalone h |
| --- | --- | --- | --- | --- |
| 1 | 5728 / 26 / 51 | 56269 / 25 / 43 | 1557 / 84 / 155 | 15397 / 83 / 108 |
| 2 | 5821 / 25 / 56 | 50826 / 19 / 43 | 1769 / 90 / 155 | 14093 / 88 / 108 |
| 3 | 5801 / 26 / 55 | 43573 / 18 / 43 | 1722 / 14 / 156 | 12755 / 82 / 107 |
| 4 | 5695 / 27 / 51 | 48914 / 24 / 43 | 1586 / 0 / 155 | 13167 / 88 / 108 |
| 5 | 5891 / 32 / 51 | 53970 / 25 / 43 | 1663 / 88 / 156 | 14766 / 83 / 108 |
| 6 | 5748 / 26 / 51 | 74304 / 9 / 43 | 1596 / 89 / 150 | 20730 / 83 / 107 |

Prey cells are **count / minimum / p50 / maximum** in milliseconds. Entry-to-first intervals are kept separate from repeated corrections and never used as repeated-report gates.

| Ordinal | Prey entry to first correction | Repeat prey corrections |
| --- | --- | --- |
| 1 | 54 / 0 / 34 / 325 | 1296 / 210 / 338 / 355 |
| 2 | 37 / 0 / 62 / 305 | 814 / 209 / 338 / 354 |
| 3 | 41 / 0 / 98 / 325 | 976 / 211 / 338 / 357 |
| 4 | 53 / 0 / 161 / 338 | 1272 / 210 / 338 / 357 |
| 5 | 42 / 0 / 85 / 336 | 783 / 210 / 338 / 356 |
| 6 | 53 / 0 / 76 / 333 | 852 / 211 / 338 / 375 |

Equal-weight means of per-batch quantities are descriptive and do not create independent packet trials or population confidence intervals.

| Split | Batches | Mean joined seconds | Mean motion <35 count | Mean h <100 count |
| --- | ---: | ---: | ---: | ---: |
| development | 3 | 578.075 | 113.333 | 28.667 |
| heldout | 3 | 569.710 | 150.333 | 35.667 |

## Hypotheses and candidate scoring

The literal observed-wire motion floors of 40/42 ms and mass floors of 100/105 ms are checked with retained short-gap witnesses; they do not establish a hidden physics timer or exact tick count. The visible decoded-tuple comparator cannot be treated as a complete account of every emitted report. Field/direction context, mass exceptions and same-frame order are reported rather than replaced with a universal sort or a fitted exception law.

**No replacement scheduler is selected.** Authoritative dirty instants and input application times between reports are unobserved. For each of candidates (a)-(e), the fixed analyzer records zero scored prediction windows and the full count of excluded motion/mass reports; packet-state mismatch and absolute next-report error are undefined. It does not substitute receipt time, a future field value or a ping midpoint for a dirty instant. Thus the frozen 20% mismatch and 2 ms next-report improvement criteria cannot be evaluated, even when acquisition coverage passes. Fixed-phase versus report-relative rearm, the authoritative dirty comparator and the exact mass-event bypass remain inconclusive. This is missing identification, not zero prediction error or rejection of every internal scheduler.

All predeclared 42/105 ms phase residual profiles are retained as descriptive diagnostics; no phase was selected from held-out data. The development selection is explicitly null. Development seal: present. It pins code, parameters/selection status, development predictions, outputs and hashes before held-out opening; the main analyzer was checked against 20 synthetic cases and retained source/byte witnesses. No held-out summary was used to edit its logic.

## Input reflection bounds

The following are conditional bounds on observed wanted-angle reflection, not input application latency. The clock fit uses five-second lower-envelope bins independently per setup/connection, and requires at least two bins; insufficient bounds stay missing. Within a +/-5-second offline window, receipts bound the offset above and ping send/reply inequalities bound it below. No RTT/2 subtraction is used. Integer-clock sensitivity of one millisecond is retained separately in each command ledger; drift-fit and callback timing remain assumptions/uncertainty. Future receipts enter this offline clock estimate, not a causal earlier-state reconstruction.

| Ordinal | Qualified commands with bounds | Reflection p50 lower to upper (ms) | Reflection p95 lower to upper (ms) |
| --- | ---: | --- | --- |
| 1 | 939 | 4.123 to 7.405 | 20.955 to 24.118 |
| 2 | 1079 | 3.808 to 7.107 | 19.222 to 22.651 |
| 3 | 1037 | 3.807 to 7.188 | 19.796 to 23.151 |
| 4 | 932 | 4.082 to 7.725 | 21.979 to 25.512 |
| 5 | 997 | 3.960 to 7.519 | 21.177 to 24.822 |
| 6 | 898 | 3.973 to 7.501 | 21.886 to 25.421 |

## Standard offline diagnostics

Packet census and unfiltered transport measurement/comparison run on every individually audited archive. The comparison uses the frozen near-server fixture and its +/-600-frame gauge, with no changed tolerance, masking substitution or favorable reference selection. A failed comparison remains a result, not a reason to recollect. Values do not certify renderer parity, absolute one-way latency or the collector's fine timing cleanliness.

| Ordinal | Anchor receipt p50 | Anchor receipt p95 | Frame server p50 | Excess delivery p90 |
| --- | --- | --- | --- | --- |
| 1 | 0.724 (PASS) | 2.279 (PASS) | 7.000 (PASS) | 1.024 (PASS) |
| 2 | 0.730 (PASS) | 2.292 (PASS) | 7.000 (PASS) | 1.016 (PASS) |
| 3 | 0.810 (PASS) | 2.361 (PASS) | 7.000 (PASS) | 1.027 (PASS) |
| 4 | 0.737 (PASS) | 2.297 (PASS) | 7.000 (PASS) | 1.047 (PASS) |
| 5 | 0.712 (PASS) | 2.349 (PASS) | 7.000 (PASS) | 1.050 (PASS) |
| 6 | 0.820 (PASS) | 2.384 (PASS) | 7.000 (PASS) | 1.047 (PASS) |

Units are world units for anchor correction and milliseconds for frame/delivery metrics. Command logs and exact outputs are retained under each `ordinal-NN-diagnostics/` directory. RTT measurements are deliberately absent from this report.

## Comparison with the host model

Source facts at publication base `d21d90b84069242c2b2d7d58c98b8103dd4adf08`:
[`SnakeReports`](../../../src/client/simulation/snakes.rs) shares each snake's
report opportunities across recipients. Under official delivery it compares
quantized heading/wanted/speed, including recipient-stale state, and rearms at
`tick + MOTION_REPORT_TICKS` (six ticks); standalone mass uses 15 ticks.
At the default 7 ms tick these are 42/105 ms model choices. Responsive delivery
bypasses motion gating for changed wanted/speed and mass gating for changed mass;
heading-only changes retain six-tick spacing. The
[delivery profiles](../../../docs/server-rules.md#packet-timing) are explicit
host designs, not two inferred official regimes.

Measured P2 other-motion medians of 43 ms, other-mass medians of 107–108 ms,
and repeat-prey medians of 338 ms are consistent with the model's nominal
scales. They do not validate its exact gates or cross-recipient phase: P2 has
one recipient. Short gaps and unchanged decoded tuple/direction reports
contradict a literal wire-floor or simple visible-value-only account. Recipient
history and latent precision remain possible explanations; P2 does not
identify them or establish that every repeated tuple would be suppressed by
our recipient-stale mechanism. The three reverse same-frame mass/motion cases
are also exceptions to the host's usual incremental order.

In [`server.rs`](../../../src/client/server.rs), the simulation drains commands
before each tick, uses timestamped command arrival to place them in world time,
then advances and hands over frames. P2's wanted-angle reflection bounds are
compatible with prompt reporting opportunities, but cannot test that input
application policy or distinguish official from responsive delivery. The hidden
application instant, dirty comparator, phase versus relative rearm, event bypass
and precise receive-to-send deadline remain unidentified. Agreement in medians
is descriptive; no host fidelity pass or new scheduler follows.

## Publication and reproduction

[Selected results](selected-results.json) retain split gates and totals;
`ordinal-NN-summary.json` retains every batch's coverage, diagnostics, reflection
endpoints, undefined candidate scores and bounded earliest witnesses. Neutral
archive aliases plus SHA-256 identify local-only recordings. Selected witness
references omit packet bytes; raw archives and
full analysis outputs are unavailable in this repository. No new sharing rights
are asserted. Endpoint, route round-trip measurements and operational provenance
are withheld. The standard collector's coarse mask cannot certify clean fine
receipt timing, and transport passes establish neither renderer parity nor
scientific acceptance.

The [plan](02-scheduler-movement.json) is a non-executable publication copy of
the frozen unbound template. Only `capture.server` and the documentation-address
placeholder in `method` were removed; scientific text (including original
encoding artifacts) is preserved. Its `.source.sha256` identifies the unchanged
local original and `.sha256` verifies the published bytes. The endpoint-bound
execution plan is retained locally. [Source pins](source-pins.json), the
[code freeze](analysis-code-freeze.json), [development seal](development-seal.json),
[null selection](development-selection.json), [predictions](development-predictions.json)
and [revision history](analysis-revision-preservation.json) preserve the pre-held-out
boundary and earlier development attempts. Historical seal paths identify
local-only originals, not files bundled here. Digests establish identity, not
prospective registration, server authenticity or scientific validity.

`analyze_p2.py` and `check_analysis.py` are byte-identical portable originals.
`test_analyze_p2.py` changes only decoder location and temporary synthetic-output
storage. Earlier development analyzer/output revisions and synthetic witnesses
remain local with their digests; they were not substituted for the frozen final
analyzer. The source report records 20 synthetic cases before held-out opening;
publication reruns those cases against the checkout decoder.

Supply the retained archive and a trusted checkout/export of the pinned source
revision; use a new external output path. These commands only read local data:

```sh
STUDY=research/fidelity/2026-10-26-movement-scheduler
uv run --locked --only-group tools python -B "$STUDY/analyze_p2.py" /external/archive.jsonl.gz --source /external/source --output /external/new-analysis.json
uv run --locked --only-group tools python -B tools/fidelity/packet_census.py /external/archive.jsonl.gz --output /external/new-census.json
uv run --locked --only-group tools python -B "$STUDY/check_analysis.py" /external/new-analysis.json /external/archive.jsonl.gz /external/new-census.json --source /external/source --output /external/new-check.json
uv run --locked --only-group tools python -B -m unittest discover -s "$STUDY" -p test_analyze_p2.py
uv run --locked --only-group tools python -B "$STUDY/select_results.py" --source-root /external/retained-task --check
```

The selector checks published plan identities and selected JSON against original
summaries and the final verification record; it does not rerun scientific
analysis. All report table values were checked against those summaries. Full
raw analysis and transport reruns are separate from publication checks.

Publication checks passed: seven documentation checks, 20 synthetic analyzer
cases, 18 transport regressions and exact selection checks for 23 published
plan/result files. All 36 per-ordinal table rows match summary JSON. All eight
retained archive hashes (including P1) and 20 selected original witness byte
references were checked locally; decoder/tool pins and published-plan digests
also match. Full scientific analysis, census and transport measurements were
not rerun for publication; their original checks remain identified separately.
No Rust build or full tools group was run for this documentation/research change.
