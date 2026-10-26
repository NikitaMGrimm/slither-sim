# Own wanted/speed changes: keep the motion gate

**Official servers hold own steering and boost changes behind the six-tick
motion gate, as our `official` profile does.** **549/581** qualified steering
commands sent while the gate was armed were reflected **41–44 ms after the
previous own motion report**. Reflection only looked fast overall because
**78%** of held commands found the gate already open. After confound screens,
**3/575 wanted** and **1/86 boost** observations contradict a literal 42 ms
floor; those exceptions remain. No rule or runtime change follows.

## Method and denominators

This retrospective offline diagnostic reuses all six frozen P2 invocations
(development 1–3, held-out 4–6), unchanged isolated-hold/boost qualification,
and the [movement study](../2026-10-26-movement-scheduler/REPORT.md)'s clock fit.
Clock inequalities locate send on the server clock, not ingress; conditional
arrival is only bracketed by earliest feasible send and reflection. Thus the
buckets describe the gate at **mapped send**, not a measured arrival tick.
Let send be `[L,H]`, reflection R, and P the last own motion report before both
L and H: armed means `H < P+42`, open means `L >= P+42`; crossing intervals and
missing prior history remain separate. The tick label is
`ceil((P+42-(L+H)/2)/7)`; only 328/581 wanted tick bins are unambiguous.
The conservative contradiction test is **`R-L < P+42-H`**: observed delay's
upper endpoint precedes the earliest model delay's lower endpoint.

| Cohort | Field | Qualified / bounded | Armed | Contradictions / armed | After screens |
| --- | --- | ---: | ---: | --- | --- |
| Development | Wanted | 3,055 | 274 | 1/274 | 1/272 |
| Held-out | Wanted | 2,827 | 307 | 3/307 | 2/303 |
| Pooled | Wanted | 5,882 | 581 | 4/581 (0.688%) | 3/575 (0.522%) |
| Development | Boost | 86 | 35 | 0/35 | 0/35 |
| Held-out | Boost | 117 | 52 | 1/52 | 1/51 |
| Pooled | Boost | 203 | 87 | 1/87 (1.149%) | 1/86 (1.163%) |

## Armed by nominal ticks remaining

Nearest-rank send-to-reflection p50 lower–upper bounds, in ms; pooling weights
commands, not batches. [TABLES.md](TABLES.md) gives every ordinal and split's
counts and p50/p90/p95; [selected JSON](selected-results.json) retains ambiguity.

| Ticks | Wanted n | Wanted p50 | Boost n | Boost p50 |
| --- | ---: | --- | ---: | --- |
| 1 | 86 | 2.797–6.114 | 6 | 3.421–7.095 |
| 2 | 67 | 9.848–13.216 | 6 | 15.535–19.209 |
| 3 | 136 | 15.832–19.264 | 24 | 20.421–23.870 |
| 4 | 201 | 23.385–26.824 | 35 | 22.577–25.856 |
| 5 | 59 | 28.005–31.373 | 9 | 29.039–32.352 |
| 6 | 32 | 35.926–39.213 | 7 | 36.219–39.359 |

**4,605/5,882** wanted commands already find the gate open. This explains the
low overall median compared with the synthetic delivery-profiles phase sweep
(wanted/boost-on p50 21 ms, p95 35 ms): the conditioning differs.

## Counterexamples and limits

Screens exclude complete prior/reflection frames containing growth or tail
changes, direction-only reports and unchanged encoded targets; all P2 clock
widths are ≤10 ms. One early wanted case coincides with growth, leaving 3/575;
the early boost packet changes both wanted and speed. Speed changes without
simultaneous wanted changes give **0/42** armed boost contradictions.
The five primary counterexamples have prior-to-reflection gaps 32/27/32/33/32 ms
and survive a further 2 ms margin; applying a 35 ms floor after screens gives
0/575 wanted and 0/86 boost. Header/service jitter and unobserved physical ticks
prevent interpreting a literal wire-floor failure as an exact tick-law failure.
Bytes/order and decoded fields are measured/reconstructed; the drift fit,
send bounds, causal attribution and nominal tick bins are conditional inference.
Ingress/application, dirty state and exact rearm/bypass rules stay unidentified.
P2's historical held-out label is not a new preregistered test; temporal batches
and commands are correlated. Speed is not an echoed boost bit.

The already-computed older corpus is supplementary: after no-op screening,
17,404 wanted matches give **3/2,255** armed contradictions; 519 boost matches
give **0/52**. Its 70 archives occupy 44 batch folders, not independent servers;
conditioning differs, so it is not pooled with P2.

## Decision and reproduction

Keep `MOTION_REPORT_TICKS` and both delivery profiles unchanged. The observed
delay staircase supports the existing gate; an unconditional bypass would erase
those waits. No rule, default, runtime behavior or historical table changes.
[Source pins](source-pins.json) identify P2's merged commit, decoder and neutral
archive hashes; originals/full ledgers remain local-only, unavailable here.
Portable `analyze.py --p2 /external/study --decoder-dir tools/fidelity --older
/external/scheduler-study --output /external/new-output` recomputes bounded
results/tables serially; omit `--older` to skip that supplementary corpus.
The output directory must be new; no per-command ledger or raw bytes are emitted.
`test_analysis.py` covers synthetic boundary/uncertainty cases. The original
offline parser/encoding failures were corrected before results; original notes
and failed attempts remain local. No new capture or game-server connection.
Scientific status: **exploratory, retrospective and unreviewed**.
Publication checks: 7 documentation, 80 simulation and 11 synthetic tests pass;
bounded JSON and all 20 bucket tables reproduce from the retained inputs.
