# Sparse-view frame cadence, 2026-10-26

**Official servers also send frames less often when a client sees few other
snakes.** In six near-server captures, the median gap between frames is
**14 ms** with 1–2 other snakes in view and **7 ms** with 6–10. Older captures
with no other snake in view have a **47 ms** median. The slow cadence of #144's
r2 matches its sparser views, and this evidence does not justify adding a fixed
heartbeat to official delivery. No runtime, delivery-profile or tolerance
changes are made.

**One gap stays open:** with 6–10 other snakes, about 68% of our frame gaps are
≤9 ms, versus 83% on official servers.

## Inputs

This is a retrospective, offline analysis of the
[near-server timing](../2026-10-26-vps-timing/REPORT.md),
[packet-scheduler](../2026-10-25-packet-scheduler/REPORT.md) and
[#144 host-transport](../2026-10-26-host-transport-500-bots/REPORT.md) captures.
Their aggregate results were already known. [PLAN.md](PLAN.md) froze the bins
and the decision rule before any conditional outcome was examined. Its unchanged
bytes have SHA-256
`609100a10fee5ae2322a45d5a3e0816cd176f3b6a244b1c6992a8b55a9b8be64`
([digest](PLAN.sha256)).

- **Primary:** the six completed near-server captures and the six valid #144
  local captures (0 and 500 bots, r1–r3). The four interrupted official attempts
  stay excluded, as originally quarantined.
- **Older cohort:** all 91 distinct official archives from the packet-scheduler
  manifest. These differ in date, route and scenario and are reported
  separately.

All 103 archives match their compressed hashes and decode. Server identity and
acquisition quality were not revalidated. [Sources](sources.json) lists neutral
archive aliases and hashes. **Raw archives, per-frame CSVs, full window traces,
the original full results and run notes stay local-only, outside Git.** No
redistribution rights or publicly retrievable raw archive are established.

## Method

- Replay uses the existing `tools/fidelity/slither_protocol.py` reader and
  `World` from complete prefixes, resetting at each connection or setup. No new
  decoder or fitted rule is introduced. Body contraction is disabled.
- **View** means recipient-cache membership, the snakes the server has told
  this client about. It is neither geometric visibility nor world population.
  Peer counts exclude the client's own snake.
- A **gap** is the server-header time between consecutive frames within one
  setup and one continuous life. Gaps across setup, appearance or death are
  excluded. Each gap is binned by the preceding frame's peer count; binning by
  the ending frame leaves the reported medians unchanged.
- A frame **changes** a snake when it carries an anchor, tail or lifecycle
  event, or changed decoded motion, mass, kills or credit. Reports that repeat
  values are counted separately. Food and prey events count even when the net
  count does not change.
- The tracker omits stock turn-direction state and display prediction. Missing
  internal ticks are unobservable.

## Official cadence by view

The primary official captures contribute **386,891 active gaps**, unfiltered.
Stable windows are whole one-second server-time windows whose peer count stays
in one bin; frames/s is their median received-frame rate, which also counts
silence between frames.

| Other snakes | Gap n | Exposure s | Median / p95 ms | Gaps ≤9 ms | Stable windows / frames/s |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 53 | 2.042 | 36 / 105 | 22.64% | 0 / — |
| 1–2 | 11,044 | 192.126 | 14 / 42 | 41.62% | 147 / 57 |
| 3–5 | 67,497 | 770.259 | 8 / 28 | 62.78% | 573 / 87 |
| 6–10 | 195,617 | 1,659.941 | 7 / 15 | 83.43% | 1,333 / 119 |
| 11–20 | 106,660 | 797.453 | 7 / 13 | 94.20% | 622 / 135 |
| 21+ | 6,020 | 42.983 | 7 / 8 | 98.77% | 32 / 140.5 |

Every near-server capture has a 1–2-peer median of **13–14 ms**. Four meet the
frozen sparse sample and exposure minimum on their own; none meets the
per-tick criterion (median ≤9 ms and at least 80% of gaps ≤9 ms). The frozen
condition for a missing heartbeat therefore fails, so no implementation PR
follows. This publication only records the result.

Near-server exposure with no other snake in view is too small. The **older**
cohort supplies **45,252** zero-peer gaps over **2,652.300 s**: median/p95
**47/142 ms**, with **11.53%** ≤9 ms. Its 2,466 stable zero-peer windows have a
median of **17 frames/s**. Fifteen older captures each have more than 1,000
zero-peer gaps, with medians of **42–57 ms**. Its 1–2-peer bin has 165,029 gaps,
median/p95 **15/63 ms**.

## What quiet frames contain

Primary official frames with no change to another snake number **41,791** (gap
median/p95 **7/23 ms**). Frames with no snake change at all, including the
client's own, number **26,399** (**7/22 ms**); **2,076** of those come from
1–2-peer views (**14/43 ms**). These condition on frames that arrived and
describe no silent internal tick. Own state changed in **52,354** frames; food
and prey activity occurred in **112,512** and **6,201**, overlapping.

**12,067** frames carry only own state and **2,555** only a pong. All six
primary official audits have zero unmatched pongs. Our host already emits a
frame for own updates, requested pongs and maintenance. None of the 103 parsed
archives contains an empty or header-only frame.

Counterexamples are retained. **1,571** frames contain only `h` mass packets
with unchanged decoded mass, and **174** only repeated decoded motion tuples.
Our value comparator would suppress such packets, but they are too sparse to
establish a per-tick heartbeat. Repeated motion tuples are not proven
semantically empty; the exact dirty, rearm, direction and prediction rules
remain unidentified.
[Selected witnesses](selected-witnesses.json) keep five complete-prefix checks,
including a **188 ms** primary gap with one peer and a **267 ms** older
zero-peer gap. Frame indices count incoming frames from zero, including
challenges.

## #144 captures compared

| 500-bot capture | Time-weighted peers | Frames with no other change | All-gap median | Medians with 1–2 / 3–5 / 6–10 peers | Time with 1–2 peers |
| --- | ---: | ---: | ---: | ---: | ---: |
| r1 | 6.49 | 11.93% | 7 ms | 14 / 7 / 7 ms | 8.337 s (2.82%) |
| r2 | 3.43 | 23.16% | 14 ms | 14 / 7 / 7 ms | 110.866 s (37.46%) |
| r3 | 5.60 | 16.44% | 7 ms | 14 / 7 / 7 ms | 19.453 s (6.48%) |

r2 spent 37% of its time with 1–2 peers, against 3–6% for r1 and r3, and all
three have the same median within each peer bin. That supports explaining r2's
cadence by its mix of views. It is not a causal proof or a fidelity pass.

**The open gap:** with 6–10 peers, **67.99%** (r1) and **68.05%** (r2) of local
gaps are ≤9 ms, versus **83.43%** official. Matching peer counts does not
control for activity or scenario, and the cause of this residual is unknown.
The local zero-peer median remains **35 ms**; same-route official zero-peer
coverage is inadequate.

## Decision and gate recommendation

Do not add an empty, ping or own-state heartbeat from this evidence. #144's r2
cadence and r3 excess-delay failures remain failures under its frozen gates;
sparse views do not excuse the delay failure. That study is unchanged.

A future gate revision should freeze **view-matched bins and whole-window
emission rates**, minimum exposure and per-capture results. It should match
time-weighted peer membership, own speed, turn and anchor activity, food and
prey, and ping load; use one declared mixture; and validate each batch
separately. Bins without enough data stay inconclusive. Keep ≤9 ms as the
original workload-dependent diagnostic. Do not waive sparse failures
automatically or fit tolerances from this table.

## Limits

Measured: bytes, cache transitions, header and receipt gaps, and associations
with frame content. Inferred: event-driven omission is a plausible shared
explanation. Source facts: our host emits only nonempty frames and compares
values before sending. Unidentified: hidden official ticks, rearm logic,
geometric subscriptions, causal CPU effects and rendered interpolation. Frames
are correlated, and header clocks identify neither one-way latency nor renderer
equivalence. Status: **exploratory and unreviewed**. The historical held-out
pair is not a new independent validation split.

## Reproduction, provenance and checks

The original analysis base is
`503ca4df8104321c7b0e5c28cc1f628c1dd6cc48`; #144 was read at
`eb92fdd2a4e61f2bef0feed079ba405e93d5a998`. Publication is based on the later
main that includes #144. [Source pins](source-pins.json) preserve the original
plan, analysis, decoder and full-output digests. The portable command-line
adaptation removes local paths and obsolete window bookkeeping; bins,
eligibility and arithmetic are unchanged. The original local notes record the
first window and own-life corrections, and a failed summarizer attempt caused by
a shadowed variable.

Copy `sources.json` to an external input manifest and set each `path` to the
archive's retained location, keeping every ID, cohort and hash unchanged.
Neutral `raw/<id>.jsonl.gz` paths resolve relative to that manifest; no raw
files are bundled. From the repository root, with a new external output path:

```sh
STUDY=research/fidelity/2026-10-26-sparse-view-cadence
uv run --locked --only-group tools python "$STUDY/analyze.py" --inputs /external/inputs.json --out /external/sparse-output
uv run --locked --only-group tools python "$STUDY/analyze.py" --inputs /external/inputs.json --out /external/sparse-output --older
uv run --locked --only-group tools python "$STUDY/summarize.py" --out /external/sparse-output
uv run --locked --only-group tools python "$STUDY/enrich_witnesses.py" --out /external/sparse-output
uv run --locked --only-group tools python "$STUDY/verify.py" --out /external/sparse-output
uv run --locked --only-group tools python "$STUDY/select_results.py" --out /external/sparse-output --check
```

The primary analysis requires a new output directory; the later steps reuse it.
The selection check compares JSON values exactly against the
[pooled](pooled.json), [per-capture](results.json),
[witness](selected-witnesses.json) and [verification](verification.json)
records.

All twelve primary integrity audits pass. The existing census independently
matches packet counts, and causal cache and clock checks pass. The existing
parity tool reproduces #144's counts and medians exactly: 0 bots
**35/35/35 ms**, 500 bots **7/14/7 ms**. Its active-only denominators
intentionally exclude boundary and post-death gaps. Synthetic window checks
cover partial windows, membership changes and fully silent seconds. The
portable scripts reproduce all selected outputs exactly. The 18 transport
regressions and seven documentation checks pass. The Rust and full tools groups
were not run for this study-only change. No game server was contacted and no
service was run; the analysis used serial standard-library Python with at most
two processes overlapping, and needed no Rust build. Integrity and computation
checks do not establish scientific acceptance.
