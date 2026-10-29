# Contiguous retained-sector state after compact snake histories

**Final status: rejected for good after the separate confirmation.** The
ten-pair confirmation improves mature stock whole-window CPU in only 5/10
pairs, below its pre-registered 8/10 requirement. No further series will run.
The original five-pair report and rejection below are preserved; the
[post-result maintainer decision and confirmation](#separate-post-result-confirmation)
are reported separately, without pooling observations or changing either gate.

*Original five-pair series:*

**Rejected under the frozen gate; no runtime change.** Contiguous retained-
sector state lowers mature maintenance CPU in all ten pairs, with median paired
savings of 662 microseconds/tick stock and 543 for protocol 13. However, both
mature whole-window CPU ratio ranges cross one, and young protocol 13 costs
+39.1 microseconds/tick median against the +10 limit. No observation is excluded,
retried or tuned. The exact candidate and oracle remain in
[rejected-layout.patch](rejected-layout.patch).

## Fresh maintenance attribution

Baseline `9ad002c1` includes compact occupied snake histories. The
[plan](PLAN.md) was committed as `b409578e` before new collection; the
[diagnostic binary pins](attribution-build.json) as `00b197c8` before profiling;
the [candidate selection](SELECTION.md) as `62d0726a` before implementation.
No profile of the older sparse-history runtime selects this change.

A 4-vCPU Zen 5 EPYC KVM guest runs the existing complete-backend, mature-500
fixture without sockets or wall pacing. One full warmup per protocol reaches
tick 193,000. Each of three counter-only and three IP-sampling windows per event
forks that exact backend and advances 1,080 ticks: 1,065 ordinary and 15 actual
food-epoch transitions. Policy/admission preparation, draining and serialization
are outside the measured advance. Release/thin-LTO/one codegen unit is portable;
the diagnostic alone retains symbols. No affinity or CPU-specific runtime path.

Maintenance shares of sampled cycles, stock / protocol 13:

| Containing source scope | Stock | Protocol 13 |
| --- | ---: | ---: |
| Food overall | 34.82% | 33.25% |
| Retained-sector advancement | 11.70% | 8.72% |
| Subscription union membership updates | 5.99% | 5.85% |
| Held-listing refresh | 9.47% | 10.83% |
| Listing build/sort | 1.53% | 1.37% |
| Sector diff and listing lookup | 2.79% | 2.49% |

There are 718 / 803 maintenance cycle samples. The old `last_seen.retain`
site receives 36 in each protocol. Retained-sector advancement accounts for
10.31% / 9.54% of maintenance dTLB samples (485 / 566 total). Ordinary cycle
samples still lead with snake visibility/history/bounds (21.08% / 19.41%),
while food accounts for 10.57% / 9.96%. Food's broad containing scope is larger,
but its leading costs are ownership/hint/subscription work; the sampled record
packing scopes do not establish a clearly larger removable layout budget.
This supports candidate 2 without claiming that its whole containing budget
can disappear. Held-listing refresh remains a separate follow-up lead.
For context, the remaining leading project groups on maintenance ticks are
snake work (7.80% / 6.85%), broadcast (5.71% / 4.11%) and physics
(4.32% / 4.86%). Ordinary physics is 10.93% / 10.53%, broadcast 9.57% / 8.75%
and the food grid 5.44% / 5.23%. External/allocator and unresolved shared-library
samples remain in each denominator rather than being redistributed.

Cycle and dTLB profiles are lossless and unthrottled, with fully scheduled
counters. Both L1d sampling series throttled at the predeclared 10,007 period;
they remain diagnostic-only and were not repeated. Counter-only L1d data is
valid. IP overflow locations and static inline stacks do not identify precise
loads or data addresses. The diagnostic counter medians, separately from any
savings claim, are:

| Protocol / tick | Million cycles | L1d misses/k instructions | dTLB misses/k instructions |
| --- | ---: | ---: | ---: |
| Stock ordinary | 7.124 | 26.414 | 0.435 |
| Stock maintenance | 17.118 | 18.443 | 0.524 |
| Protocol 13 ordinary | 7.186 | 26.029 | 0.493 |
| Protocol 13 maintenance | 17.055 | 17.637 | 0.560 |

The frozen start has 30,199 retained keys/timestamps across 500 recipients.
The old timestamp-map payload is at least 301,990 bytes, plus 60,398 duplicate
membership-key bytes, before unmeasured tree-node/allocator overhead. The
vectors remove duplicate keys and pointer traversal; insertion shifts and
spare capacity are measured costs, not assumed free. Compact snake histories
already reserve only 444,864 bytes for 3,681 occupied records. Shared food
listings have 42,392 capacity slots for 28,307 live 32-byte entries.

The narrow union scope uses generated food-source lines 58–107, excluding its
final retired-listing filter. Held refresh uses 534–574, listing build/sort
352–382, and sector diff/lookup 30–37 plus 576–615. These are source-IP scopes,
not precise load attribution. [attribution-tables.py](attribution-tables.py)
recomputes the shares and counter medians without the raw sample streams.

Selected raw-derived evidence: [profiles](attribution.json),
[counter-only runs](attribution-counters.json),
[maintenance sites](maintenance-sites.json),
[footprints](attribution-footprints.json), [event controls](attribution-events.json).

## Exact runtime candidate

Candidate `5edb5b2d` keeps ascending unique two-byte sector keys and parallel
`f64` timestamps. Binary search updates an existing timestamp or inserts a new
key/time pair. A due cleanup stably compacts both arrays together. It retains
the oldest non-current witness and unchanged skip predicate, outgoing-stencil
touch rewrites, strict `now - seen > 2.6` expiry, cleanup epochs and membership
revision changes. Recipient state, cloning and reconnect resets stay independent.
Production food consumers receive the same ascending keys. No deadline addition,
reduced-precision storage, protocol or packet scheduler change is introduced.
The measured vectors reserve 127,360 key bytes and 509,440 timestamp bytes
at the mature start, 636,800 total. Tree capacity/node bytes were not measured;
the old minimum payload alone does not establish an allocator-byte saving.

## Primary method and frozen decision

Both arms use portable release builds and the unchanged full-backend fixture.
Fresh complete warm streams are compared byte-for-byte for stock and protocol
13 before any primary timing. The mature starts are tick 193,000; young-500
starts at zero; 1/10/50-seat controls warm for 600 ticks. Each row has one
discarded AB and five AB/BA/AB/BA/AB pairs, each 1,080 ticks, forked from the
same parent. Instructions, cycles, L1d and dTLB are grouped user-only events
with at least 99% scheduling per interval. CPU includes identical wrappers,
without subtraction. All counted windows finish before direct-parent proofs
or reductions. Full redacted process commands are recorded at boundaries.
Primary build/source pins were committed as `09a419bc` before timing:
[primary-build.json](primary-build.json). The two-second
[idle preflight](primary-idle-preflight.json) found no other active CPU job;
the [notice](primary-timing-notice.json) preceded timing by over 60 seconds.
Every measured interval is fully scheduled, so no counter fallback is used.

Both mature protocols start with 499 living snakes across 500 recipient slots,
27,610 anchors, mean length 55.33, p90 176 and maximum 271, and end with 500
living, 28,080 anchors, mean 56.16, p90 176 and maximum 271. The tick range is
193,000–194,080. The unchanged fixture includes deaths, respawns and reconnects;
the two arms' complete endpoint records match in every window.

The maintaining agent froze the adoption rule before any results: every mature
whole-window pair improves in both protocols; every short row's median paired
absolute CPU cost is at most +10 microseconds/tick; both mature maintenance
median paired CPU costs are nonpositive. Integer CPU totals evaluate that gate.
No outcome-based exclusion, retry, threshold change or layout tuning is allowed.
Paired ranges are descriptive, not confidence intervals. These fixtures contain
no sockets and cannot establish live official-500 p99 or capacity on other CPUs.

## Primary results and verdict

Ratios below are medians of five paired candidate/base ratios, followed by their
full range. They are not ratios of separately calculated arm medians. The
[complete tables](TABLES.md), [selected observations](results.json) and
[independent calculator](tables.py) retain instructions, cycles, CPU, L1d/dTLB
MPKI and tick p99 separately for ordinary and maintenance ticks on all ten rows.
Discarded AB observations remain in the JSON and are outside reported medians.

| Mature protocol / tick | CPU paired ratio [range] | Instructions paired ratio | Cycles paired ratio |
| --- | ---: | ---: | ---: |
| Stock all | 0.9207 [0.9138, 1.0860] | 0.9886 | 0.9215 |
| Stock ordinary | 0.9249 [0.9171, 1.0915] | 0.9902 | 0.9255 |
| Stock maintenance | 0.8143 [0.7793, 0.9263] | 0.9311 | 0.8144 |
| Protocol 13 all | 0.9515 [0.8734, 1.0289] | 0.9897 | 0.9539 |
| Protocol 13 ordinary | 0.9548 [0.8773, 1.0327] | 0.9912 | 0.9572 |
| Protocol 13 maintenance | 0.8505 [0.7552, 0.9109] | 0.9362 | 0.8520 |

Median paired whole-window savings are 115.1 microseconds/tick stock and
78.6 for protocol 13, but stock pair 0 costs 8.60% more and protocol-13 pair 2
costs 2.89% more. Both violate the requirement that every mature pair improve.
Maintenance paired savings range from 268.7 to 799.8 microseconds/tick stock,
and 321.8 to 996.5 for protocol 13. Maintenance instruction counts consistently
fall 6.89% / 6.38%. Whole-window p99 arm medians fall 3.4082→2.8624 ms stock
and 3.6281→3.1248 ms protocol 13; these are descriptive no-socket fixture tails.

| Short control | Stock median paired CPU cost, µs/tick | Protocol 13 | +10 limit |
| --- | ---: | ---: | --- |
| Young 500 | -38.1000 | +39.0792 | Protocol 13 fails |
| 1 seat | -0.1272 | -0.1138 | Pass |
| 10 seats | -0.7201 | -0.5594 | Pass |
| 50 seats | -2.4124 | -2.3731 | Pass |

Young protocol 13 ranges from -82.82 to +120.52 microseconds/tick, with median
paired ratio 1.0345. Its instruction count falls 1.68%, but the measured median
absolute CPU cost still exceeds the gate. No competing workload was observed;
the sources of the CPU/cycle variation are not identified by this series.
Lower instruction counts do not overrule the CPU observations.

Mature ordinary dTLB MPKI changes 0.4615→0.3701 stock and 0.4729→0.4673
protocol 13; maintenance changes 0.5464→0.4160 and 0.5307→0.4868. L1d MPKI
increases in all four mature classes (27.0561→27.8492, 26.8764→27.2601,
18.8069→20.0211 and 18.2054→19.0244 respectively). These are ratios per
instruction, not proof of a globally memory-bound thread or a specific load's
cause. Smaller containers and fewer tree traversals are insufficient adoption
evidence when the frozen whole-window/control criteria fail.

The study therefore rejects this exact prototype, preserving its measurements
and patch. It makes no change to the rule, layout, compiler settings or sample
selection after seeing results. The fresh profile still identifies held-listing
refresh/ownership and subscription-union work as portable follow-up budgets;
their containing shares do not promise a speedup.

## Exactness and delivery checks

- All 13 focused visibility tests pass, including the literal tree-backed
  full-sweep oracle over 48,000 random-walk steps, bit-exact timestamps,
  re-entry, reset/reconnect, clone independence, large jumps, strict rounding
  and full-domain key compaction.
- Wrong outgoing time, missing re-entry refresh, deadline addition and
  misaligned compaction each fail at runtime (exit 101). Source bytes are
  restored after each; the restored suite passes.
- `python3 tools/check.py rust` passes on the measured candidate, covering the
  complete offline Rust group. The seven documentation tests pass.
- All 27 catalog/host traces are byte-identical. Stock and protocol-13 radius
  mutations change complete traces and host-frame payloads. A stream-byte flip
  is separately detected by the full warm-stream comparer.
- Both complete warm streams match every byte through tick 193,000. All ten
  cohorts' two child continuations per arm and both direct-parent streams
  match completely, including frame order, packet bytes, cumulative counts,
  action/state records and terminal summaries.

[proofs.json](proofs.json) retains actual commands/exits and trace receipts;
the warm/continuation receipts are in `results.json`. Final delivery checks
rerun docs and independently recompute the tables/gate, then apply/reverse the
rejected patch and compare both runtime files with the measured archives.
No timing series is repeated. No separate agent review is claimed; maintainer
review remains required.

Reproduction and exact prerequisite-receipt construction:
[REPRODUCE.md](REPRODUCE.md). Full raw evidence remains local-only, outside Git;
it is not publicly downloadable. The evidence archive is streamed off the guest
without allocating another guest copy, then streamed back for member-by-member
byte comparison with the stable source files before pruning. A transferred
archive digest and verification counts are retained in `archive-receipt.json`.

## Separate post-result confirmation

After the first series failed, the maintaining agent declined an override and
authorized exactly one separate confirmation. [CONFIRMATION-PLAN.md](CONFIRMATION-PLAN.md)
was committed and pushed as `a6dfc764` before any new observations. This is a
**post-result maintainer decision**, not a revision of the original study's
verdict. Adoption rests on the confirmation alone. If it fails, the candidate
stays rejected for good; no third series, tuning or extra pairs are allowed.

The new arms are main `b750004a` (only 0.8.2 version metadata and research #275
beyond the first runtime) and candidate `9a2f3bbc`. The exact original patch is
applied without edits; both affected runtime files match `5edb5b2d` byte-for-byte.
[confirmation-build.json](confirmation-build.json) pins both fresh builds and
the Python controller/C collector aggregate, committed as `26d3a2cc` before
timing. The Rust fixture, portable code generation, four grouped counters and
advance brackets are unchanged. New Python boundary reads record background
CPU outside the measured advance.

Four rows use fresh complete-backend parents: mature-500 at tick 193,000 and
young-500 at zero, both protocols. Each has one retained discarded AB, then
ten AB/BA alternating pairs of 1,080 ticks. The previously passed 1/10/50-seat
controls are not repeated. Mature start/end censuses match the first fixture;
young worlds start with 500 living two-anchor snakes and end with 500 living,
mean length 4.728, p90 6 and maximum 13. All within-series endpoint records match.

The confirmation-only gate requires a mature median paired whole-window CPU
ratio below one and at least 8/10 improving pairs in **each** protocol, mature
maintenance median paired ratio below one in each, and young median paired
absolute CPU cost at most +10 µs/tick in each. Even medians average the middle
two values; exact rational/integer comparisons decide the boundaries.

| Mature protocol | Improving whole-window pairs | Median whole CPU ratio [range] | Median maintenance CPU ratio [range] | Gate |
| --- | ---: | ---: | ---: | --- |
| Stock | **5/10** | 0.987482 [0.878007, 1.055279] | 0.859030 [0.736831, 0.929779] | **Fail: fewer than 8 wins** |
| Protocol 13 | 8/10 | 0.957239 [0.891392, 1.061125] | 0.850463 [0.789021, 0.913717] | Pass |

| Young protocol | Median paired cost, µs/tick | Full paired cost range | +10 gate |
| --- | ---: | ---: | --- |
| Stock | -31.986611 | [-110.523897, +79.981259] | Pass |
| Protocol 13 | -2.689671 | [-70.388379, +86.954306] | Pass |

Maintenance CPU improves in all twenty reported mature pairs, with median
paired savings of 540.476 µs/tick stock and 557.440 for protocol 13. Mature
maintenance instructions fall 6.90% / 6.49%, and ordinary instructions fall
0.98% / 0.92%. All four whole-window instruction medians improve. This does
not satisfy the failed stock CPU quorum and does not change the decision.
Whole-window CPU p99 arm medians are 3.531690→3.079925 ms stock and
3.735635→3.373615 ms protocol 13, with overlapping ranges. These are descriptive
no-socket fixture tails, not a live official-500 capacity result.

[CONFIRMATION-TABLES.md](CONFIRMATION-TABLES.md) retains the new ordinary and
maintenance CPU, instructions, cycles, L1d/dTLB MPKI and p99 tables. The new
[confirmation-results.json](confirmation-results.json) retains all eleven
pairs per row and every arm's background diagnostics. The original
`results.json`, `TABLES.md`, plan and selection are unchanged. The independent
[confirmation calculator](confirmation-tables.py) verifies only this series;
the original calculator separately reproduces the original rejection.

### Background and exactness

The [idle preflight](confirmation-idle-preflight.json) passes after the
[notice](confirmation-timing-notice.json). All 88 windows, including discarded
ABs, remain retained; none are filtered or corrected. Counters are fully
scheduled in every interval. Visible non-study CPU ranges from 0 to 50 ms per
request, with row medians of 20–30 ms; aggregate busy-minus-study CPU ranges
from -30 to 170 ms, with medians of 55–90 ms. The 10 ms quantum and boundary
alignment permit negative residuals, which are preserved. Aggregate steal totals
220 ms across all request spans; no new/exited non-study process is observed at
the boundaries. The request spans include untimed policy/drains and cover all
guest CPUs. These diagnostics do not establish the cause of CPU variation and
never enter the gate.

On the fresh arms, the thirteen-test visibility suite, four detected runtime
mutations and byte-checked restoration, full offline Rust group, all 27 traces,
both wire controls, stream-byte control and seven documentation tests pass.
Both complete mature warm streams and all four cohorts' six continuation
streams match. Nine synthetic gate/accounting controls pass. Actual commands,
exits and receipts are in [confirmation-proofs.json](confirmation-proofs.json).
No conditional runtime adoption or post-adoption rebuild/spot check is claimed:
that branch was not entered because confirmation failed.

**Decision: keep the runtime unchanged and retain the original prototype patch.
This candidate is rejected for good under the maintaining agent's confirmation
rule. No observations are pooled, excluded or retried; no further series.**

[CONFIRMATION-REPRODUCE.md](CONFIRMATION-REPRODUCE.md) gives the separate recipe.
Final research-only checks recompute both series independently, rerun docs and
compare the retained patch in both directions against the measured archives;
their receipt is `confirmation-postprocessing-checks.json`. Full raw evidence
is local-only. The confirmation archive is bounded in guest tmpfs, compared
member-by-member before SFTP transfer, and checked by SHA-256 after transfer;
`confirmation-archive-receipt.json` records its identity and verification.
