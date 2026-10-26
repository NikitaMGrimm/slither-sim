# Sparse-view frame cadence, 2026-10-26

**Official cadence also slows in sparse views.** The six near-server captures
have a **14 ms** median gap with 1–2 other snakes and **7 ms** with 6–10.
Older zero-peer captures have **47 ms**. These observations do not support
adding a content-independent heartbeat to official delivery. No runtime,
delivery-profile or tolerance changes are made.

## Method and evidence

This is retrospective exploration of the completed
[near-server timing](../2026-10-26-vps-timing/REPORT.md),
[packet-scheduler](../2026-10-25-packet-scheduler/REPORT.md) and
[#144 host-transport](../2026-10-26-host-transport-500-bots/REPORT.md) inputs.
The aggregate outcomes in those studies were already known.
[PLAN.md](PLAN.md) froze bins and the decision rule before inspecting conditional
outcomes; its unchanged bytes have SHA-256
`609100a10fee5ae2322a45d5a3e0816cd176f3b6a244b1c6992a8b55a9b8be64`
([digest](PLAN.sha256)).

All six completed near-server and six valid local captures are primary inputs;
the four interrupted official attempts remain excluded. All 91 distinct
official-labelled scheduler archives form a supporting cohort. All 103 match
compressed hashes and decode; historical server identity/acquisition quality
was not independently revalidated. [Sources](sources.json) gives neutral archive
aliases and hashes. **Raw archives, per-frame CSVs, full window traces, original
full results and run notes remain local-only, outside Git.** No redistribution
rights or publicly retrievable raw archive are established.

Replay uses the existing `tools/fidelity/slither_protocol.py` reader and `World`,
starting from complete prefixes and resetting at connection/setup. View means
recipient-cache membership, not geometric visibility or world population. Own
snake is excluded from peer counts. Eligible gaps join consecutive frames in
one setup and continuous active life; setup/appearance/death boundaries are
excluded. Bins use the preceding frame's cache; ending-bin sensitivity preserves
the reported medians. Body contraction is disabled. Per-frame local output
retains visible/reported/changed IDs, own decoded state/anchor age, food/prey
counts/activity and both clocks.

A change means an anchor/tail/lifecycle event or changed decoded
motion/mass/kill/credit values. Reports are counted separately from changes;
food/prey events count even without net count changes. The tracker omits stock
turn-direction state and display prediction. Missing internal ticks are
unobservable. No new decoder or fitted rule is introduced.

## Official conditional cadence

Primary official: **386,891 active intervals**, unfiltered. Stable-window column
gives whole 1-second server-time windows remaining in one peer bin / median
received frames per second; it also captures silence between received frames.

| Other snakes | Gap n | Exposure s | Median / p95 ms | Gaps ≤9 ms | Stable windows / frames/s |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 53 | 2.042 | 36 / 105 | 22.64% | 0 / — |
| 1–2 | 11,044 | 192.126 | 14 / 42 | 41.62% | 147 / 57 |
| 3–5 | 67,497 | 770.259 | 8 / 28 | 62.78% | 573 / 87 |
| 6–10 | 195,617 | 1,659.941 | 7 / 15 | 83.43% | 1,333 / 119 |
| 11–20 | 106,660 | 797.453 | 7 / 13 | 94.20% | 622 / 135 |
| 21+ | 6,020 | 42.983 | 7 / 8 | 98.77% | 32 / 140.5 |

Every near-server batch's 1–2-peer median is **13–14 ms**. Four individually
meet the frozen sparse sample/exposure minimum, and none meets the near-per-tick
criterion (median ≤9 ms and ≥80% gaps ≤9 ms). The frozen condition for a missing
heartbeat therefore fails. The original decision against an implementation PR
is preserved; this publication is a research-only record.

Near-server zero-peer exposure is insufficient. The **older** cohort supplies
**45,252** zero-peer intervals over **2,652.300 s**: median/p95 **47/142 ms**,
**11.53% ≤9 ms**. Its 2,466 stable zero-peer windows have median **17 frames/s**.
Fifteen older captures individually have >1,000 zero-peer intervals and medians
**42–57 ms**. Its 1–2-peer stratum has 165,029 intervals, median/p95 **15/63 ms**.
Different dates/routes/scenarios remain separate cohorts.

## Quiet contents and counterexamples

Primary official frames with no other-snake change number **41,791** (gap
median/p95 **7/23 ms**); no snake change, including own, **26,399** (**7/22 ms**).
Restricting the latter to 1–2-peer views gives **2,076**, **14/43 ms**. These
are conditions on received frames, not observations of silent internal ticks.
Own state changed in **52,354** frames; food/prey activity occurred in
**112,512 / 6,201**, with overlap.

There are **12,067** own-only and **2,555** pong-only primary frames. All six
primary official audits have zero unmatched pongs. Own updates, requested pongs
and maintenance already trigger our host's emission. No empty/header-only frame
was found in any of the 103 parsed archives.

Counterexamples are retained: **1,571** frames contain only unchanged decoded
mass `h` packets, and **174** only repeated decoded motion tuples. Our value
comparator can suppress such traffic, but these sparse events do not establish
a per-tick heartbeat. Repeated motion tuples are not proven semantically empty;
exact dirty/rearm/direction/prediction semantics remain unidentified.
[Selected witnesses](selected-witnesses.json) retain five complete-prefix checks,
including a **188 ms** primary gap with one peer and **267 ms** older zero-peer
gap. Frame indices count incoming frames from zero, including challenges.

## #144 r1/r2/r3 comparison and remaining gap

| 500-bot capture | Time-weighted peers | Frames with no other change | All-gap median | Medians with 1–2 / 3–5 / 6–10 peers | Time with 1–2 peers |
| --- | ---: | ---: | ---: | ---: | ---: |
| r1 | 6.49 | 11.93% | 7 ms | 14 / 7 / 7 ms | 8.337 s (2.82%) |
| r2 | 3.43 | 23.16% | 14 ms | 14 / 7 / 7 ms | 110.866 s (37.46%) |
| r3 | 5.60 | 16.44% | 7 ms | 14 / 7 / 7 ms | 19.453 s (6.48%) |

r2 is much sparser, and these matched-bin medians agree across local
repetitions. This supports a view-mixture explanation, not a causal proof or
fidelity pass. **An open fidelity gap remains:** with 6–10 peers, local r1/r2
have **67.99% / 68.05%** gaps ≤9 ms versus official **83.43%**. Peer-count matching
alone does not resolve activity/scenario differences or establish why this
residual exists. The local zero-peer pool remains **35 ms**; same-route official
zero-peer coverage is inadequate.

## Decision, gate recommendation and limits

Do not add an empty/ping/own-state heartbeat from this evidence. #144's r2
cadence and r3 excess-delay failures remain failures under their frozen gates;
sparse cadence does not excuse the delay failure. That study is unchanged.

A future gate revision should freeze **view-matched strata and whole-window
emission rates**, minimum exposure and per-capture results. Match time-weighted
peer membership, own speed/turn/anchor activity, food/prey and ping load; use a
common declared mixture and separate batch-level validation. Keep unsupported
strata inconclusive. Retain ≤9 ms as its original workload-dependent diagnostic;
do not automatically waive sparse failures or fit tolerances from this table.

Measured: bytes, cache transitions, header/receipt gaps and content associations.
Inferred: event-driven omission is a plausible shared explanation; our
nonempty-only emission/value comparisons are source facts. Hidden official
ticks, rearm logic, geometric subscriptions, causal CPU effects and rendered
interpolation remain unidentified. Frames are correlated; header clocks identify
neither one-way latency nor renderer equivalence. Scientific status:
**exploratory, unreviewed**; the historical held-out pair is not a new independent
validation split.

## Reproduction, provenance and checks

The original analysis base is
`503ca4df8104321c7b0e5c28cc1f628c1dd6cc48`; #144 was read at
`eb92fdd2a4e61f2bef0feed079ba405e93d5a998`. Publication is based on the subsequent
main including #144. [Source pins](source-pins.json) preserve the original plan,
analysis/decoder and full-output digests. The portable CLI adaptation removes
local paths and obsolete window bookkeeping; bins, eligibility and arithmetic
are unchanged. Original local notes retain the initial window/own-life
corrections and a failed summarizer attempt caused by a shadowed variable.

Copy `sources.json` to an external input manifest and set each `path` to its
retained archive location, keeping every ID, cohort and hash unchanged. Neutral
`raw/<id>.jsonl.gz` paths resolve relative to that manifest; they do not imply
bundled raw files. From the repository root, with an external new output path:

```sh
STUDY=research/fidelity/2026-10-26-sparse-view-cadence
uv run --locked --only-group tools python "$STUDY/analyze.py" --inputs /external/inputs.json --out /external/sparse-output
uv run --locked --only-group tools python "$STUDY/analyze.py" --inputs /external/inputs.json --out /external/sparse-output --older
uv run --locked --only-group tools python "$STUDY/summarize.py" --out /external/sparse-output
uv run --locked --only-group tools python "$STUDY/enrich_witnesses.py" --out /external/sparse-output
uv run --locked --only-group tools python "$STUDY/verify.py" --out /external/sparse-output
uv run --locked --only-group tools python "$STUDY/select_results.py" --out /external/sparse-output --check
```

Primary analysis requires a new output directory; later steps use that external
directory. Full generated CSVs/results remain local-only. Selection checks compare
JSON values exactly against [pooled](pooled.json), [per-capture](results.json),
[witness](selected-witnesses.json) and [verification](verification.json) records.

All twelve primary integrity audits pass. The existing census independently
matches packet counts; causal cache/clock checks pass. Existing parity-tool
counts/medians reproduce #144 exactly: 0 bots **35/35/35 ms**, 500 bots
**7/14/7 ms**; active-only denominators intentionally exclude boundary/post-death
gaps. Synthetic window checks cover partial windows, membership changes and
fully silent seconds. The portable scripts reproduce all selected outputs
exactly. The 18 transport regressions and seven documentation checks pass.
Rust/full tools groups were not run for this study-only change. No game-server
connections or services; serial standard-library analysis, with at most two
processes overlapping. No Rust build was needed. Integrity/computation checks
do not establish scientific acceptance.
