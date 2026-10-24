# Performance comparison statistics contract v1

2026-10-24. Handoff to the coordinating implementation agent. This contract supersedes ambiguous implementation details in STATISTICAL-RECOMMENDATION.md; its model and error budget are unchanged. Scope: ten advisory performance workloads, source snapshot 76a3151945b3fd5c47890ead58eeb319a171ecce. All work here is offline and scratch-only. No repository edits, benchmarks or CI dispatches were performed for this handoff.

## Decision and budget

Agreed: operation wall time for all ten workloads; paired log ratios with a fitted order term; looks at 8/12/26 pairs; eta = 1/600 per look/workload; practical band [0.95, 1.05]; 15-minute hard job timeout and at most 620 seconds for the measurement stage. Process CPU remains a raw diagnostic. Preserve the comment's table and sections.

The 620 seconds include discarded outer warmups, child startup/setup, untimed reset/control work, and measurement orchestration, not just operation timers. Reserve **60 seconds** before the hard job timeout for finalizing and uploading data. Thus the stage deadline can be earlier than its nominal 620-second cap. The implementation may reserve more if observed upload latency warrants it.

Using the retained Windows run's old batch lengths, full-suite 26-pair cost is about 602.33 seconds INCLUDING its 20.77 seconds of discarded outer warmups. Whole-job projection is 880.46 seconds, leaving only 19.54 seconds. To finish all 26 pairs and leave the recommended 60 seconds, fixed work must fall by at least **40.46 seconds**. Otherwise the deadline legitimately retains an earlier look. These are projections from one saved run, not a latency guarantee for new runners or new batch sizes.

## 1. Frozen experiment and inputs

One statistical observation is a complete adjacent baseline/candidate **fresh-process pair** for a single workload. The two processes run serially, with no other benchmark, build, test, trace or cache extraction running concurrently. Inner operations and repeated within-process blocks do not increase the statistical pair count.

Freeze a versioned plan before sampling: workload ID/definition hash, input/seed, operation scope, operation count K per block, block count m, state/reset/warmup policy, executable identities, catalog order, order sequence and deadlines. Both sides use the same plan. A PR can change implementation but cannot silently change which operation or inputs a row measures. A changed workload definition must be recorded; mismatched plans are unavailable.

For each valid pair, provide:

```
workload_id
pair_index                 # 1-based, contiguous within workload, 1..26
order                      # +1 = BASE then PR; -1 = PR then BASE
baseline_wall_seconds      # finite real > 0; booleans/strings are invalid
candidate_wall_seconds     # finite real > 0
operations_per_block       # positive integer K, identical both sides/all pairs
blocks                     # positive integer m, identical both sides/all pairs
```

Raw records additionally retain both child exit statuses, observed execution order, monotonic start/end times, complete block timings and operation counts, CPU diagnostic, binary/plan hashes and failure logs. The aggregate pair fields must be derived from successful, fully validated raw records; they cannot conceal a failed side. In A/A mode both labels use the exact same hashed executable, with label-neutral paths/arguments/environment except the output destination.

**Primary time:** a block's wall time W is the sum of the harness's monotonic operation intervals. Setup, control generation, reset, output destruction and serialization remain excluded from this primary metric, but included in elapsed CI cost. This is instrumented operation cost, not whole-process or end-to-end latency. Keep the timer scope matched across sides. Do not substitute CPU or automatically switch metrics for any row.

For the initial m=1 implementation, `*_wall_seconds = W`. If calibration selects m>1, use equal-K, identically reset/warmed blocks in each process and define `*_wall_seconds = exp(fsum(log(W_k))/m)`. This is a **geometric mean block time**, not the sum or arithmetic mean. It makes the pair log ratio exactly the mean of matched block log ratios and matches the variance-components calibration model. Store every W_k and the separate total operation time/count. This extension requires explicit harness support and a plan version; do not reinterpret existing aggregate records as if they contained blocks. Never treat the m blocks as m independent process pairs.

For fixed K, using per-operation times instead of block totals gives the same d. A shorter/longer evolving trajectory is a different workload; repeating reset-identical blocks is the approved way to extend an already capped trajectory. No per-side run-until-fast-enough or timed stopping inside a block.

## 2. Exact order and sampling schedule

Use this literal order sequence for **every workload**, indexed by that workload's pair_index. No RNG or data-dependent order selection:

```
pair:   1  2  3  4  5  6  7  8 | 9 10 11 12 |13 14 15 16 17 18 19 20 21 22 23 24 25 26
order: +1 -1 -1 +1 +1 -1 +1 -1 |-1 +1 +1 -1 |-1 +1 +1 -1 -1 +1 -1 +1 +1 -1 +1 -1 -1 +1
```

The sequence balances each adjacent two-pair block, hence all scheduled prefixes: 4/4 at n=8, 6/6 at n=12, 13/13 at n=26. +1 means baseline executes first, not that a positive effect is desirable. The schedule is fixed independently of measured data; balancing alone does not establish independent errors or eliminate arbitrary time drift.

Before counted measurements, run one fixed discarded baseline-then-PR outer warmup pair per workload using its measurement plan. Preserve these records with `warmup=true`; never include them in n. Each fresh process must also have the same fixed internal warmup/reset policy. Do not adapt warmup after seeing an interesting result or add extra discarded runs between looks. Calibration can change the frozen policy for a future plan.

Run rounds in the frozen catalog order, one adjacent pair per active workload per round. Initial catalog:

1. step-default
2. step-food
3. step-body
4. observe
5. portable
6. protocol
7. official-100-step
8. official-100-bot
9. dome-proxy-32-step
10. dome-proxy-32-bot

Analyze a workload only immediately after complete prefixes of 8, 12 or 26 pairs. Remove resolved/unavailable rows from further rounds. An unresolved workload keeps its own next pair index when another row stops. Allocate no extra alpha or pairs to other rows when a row resolves. Missing workload slots still consume their original allocation; more than ten workloads or different looks requires a new error budget/constants.

## 3. Model and calculations at a scheduled look

For a workload and i=1..n:

```
d_i = log(candidate_wall_seconds_i) - log(baseline_wall_seconds_i)
d_i = mu + gamma * order_i + epsilon_i
epsilon_i independent N(0, sigma^2), sigma > 0
```

Subtract logs to avoid overflowing an intermediate ratio. The assumption is within-workload, with constant mu, gamma and variance over its measurement window. Different workloads need not be independent. The target `exp(mu)` is the geometric mean paired multiplicative time ratio, averaged over balanced orders, conditional on this runner/binary/input protocol. It is not a ratio of arithmetic mean latencies, a tail metric, or a guarantee over all hosts/build layouts.

Validate the prefix is complete and balanced. Use `math.fsum`:

```
mu_hat    = fsum(d_i) / n
gamma_hat = fsum(order_i * d_i) / n
e_i       = d_i - mu_hat - gamma_hat * order_i
SSE       = fsum(e_i * e_i)
df        = n - 2
s         = sqrt(SSE / df)             # residual log-ratio SD, NOT sample SD of d
SE        = s / sqrt(n)
eta       = 1 / 600
critical  = T_CRITICAL[df]
L, U      = mu_hat - critical*SE, mu_hat + critical*SE
```

Do not use df=n-1, drop the fitted order effect from residuals, use standard deviation / n, use a z critical value, or add a sign/unanimity veto. `gamma_hat` is a diagnostic nuisance estimate; negative/positive pair changes remain fully included.

Hard-code the following binary64 values (17 significant digit representations):

| n | df | t_(1 - 1/1200, df) |
|---:|---:|---:|
| 8 | 6 | 5.3982324751112900 |
| 12 | 10 | 4.2586177085984529 |
| 26 | 24 | 3.5404690421382261 |

Generated by SciPy 1.18.1 `scipy.stats.t.isf(1/1200, df)`; these are floating-point reference constants, not a claim of arbitrary-precision accuracy. No SciPy dependency is needed in production or its tests. The stdlib checker verifies `.5 * I_(df/(df+t*t))(df/2, 1/2) = 1/1200` using lgamma and an incomplete-beta continued fraction, to relative tolerance 2e-10. Compute the tiny tail directly; do not lose accuracy by subtracting it from a CDF near one.

## 4. Exact labels and display transformation

Use unrounded log endpoints and `a=math.log(0.95)`, `b=math.log(1.05)`:

| Key | Condition | Display text |
|---|---|---|
| Faster | U < a | 🟢 Faster |
| Slower | L > b | 🔴 Slower |
| Within | L >= a AND U <= b | ⚪ Within ±5% |
| Undecided | otherwise, or degenerate residuals | 🟡 Undecided |
| Unavailable | invalid/failed data, or no completed scheduled look | ⚠️ Unavailable |

Within asserts the effect lies in the practical band. Undecided can have a positive or negative point estimate and can be all-positive, all-negative, or mixed pairs. It asserts neither sameness nor no regression. The +5% and -5% factors refer to time, not speed/throughput. For example, a time reduction of 10% is not a throughput increase of exactly 10%.

Display `change_percent=100*math.expm1(mu_hat)` and transform L and U the same way. No symmetric plus/minus interval on the percentage scale. Do not center an interval using median pair values.

Base and PR columns use geometric means over **the same analyzed prefix**:

```
baseline_ms_per_operation = exp(log(1000) + fsum(log(base_time_i))/n - log(K))
candidate_ms_per_operation = exp(log(1000) + fsum(log(PR_time_i))/n - log(K))
```

Their ratio is exp(mu_hat), up to numerical/formatting error. With m>1, time_i is already the geometric mean block time defined above. Do not divide it by m*K. Preserve the existing five decimal places in the Base/PR columns. Change: signed estimate and endpoints to two decimal places, e.g. `−8.30% [−10.10%, −6.40%]`; normalize displayed negative zero. Rounding NEVER changes labels. A rounded endpoint at exactly ±5.00% can coexist with a strict full-precision decision; Details states classification uses unrounded bounds.

For degeneracy display the point estimate plus `[interval unavailable]`; for Unavailable display em dashes in performance-number cells. Raw history remains available separately. If any required derived/display field overflows or is nonfinite, report Unavailable with `numeric_failure`; never serialize NaN/Infinity or silently clamp an endpoint. This is conservative suppression of a claim.

## 5. Zero variance and numerical degeneracy

Set `residual_floor = 64 * math.ulp(max(1.0, max(abs(d_i))))` in log-ratio units. If `s <= residual_floor`, keep mu_hat/gamma_hat/s as diagnostic outputs, but set both intervals to null, label Undecided and reason `degenerate_residuals`. Continue to the next scheduled look if budget permits. The multiplier is a conservative numerical policy, not an estimated noise floor or a theorem about the runner.

This catches exactly equal times, an exact two-level order-only fit, and roundoff pretending to be residual variation. Do not emit a zero-width confidence interval, add artificial noise, drop the order term, or rerun until variance appears. The rule can only withhold a claim relative to the t procedure; it cannot increase its model-based error probability. At n=26, or the cap after a degenerate scheduled look, remain Undecided. Very small but genuine runner noise can be suppressed by this policy; that is intentional.

## 6. Stopping, deadlines and failures

After a valid scheduled look:

- Faster / Slower / Within: stop this workload immediately. Keep that prefix, df, critical value, means and interval unchanged. A row resolved at n=8 never needs a 12/26-pair interval. Do not shrink its interval using a future n or borrow another workload's variance.
- Undecided at 8 or 12: continue to the next scheduled count.
- Undecided at 26: stop with `termination=max_pairs`.

Let t_job be the monotonic job-start anchor supplied by the workflow and t_stage the measurement-stage start before outer warmups. Deadline is `D=min(t_stage+620, t_job+900-60)`. Keep the workflow hard timeout at 15 minutes. Anchor the collector as early as possible; an anchor recorded only late in setup must not pretend to be the actual job start. The hard workflow timeout is the outer enforcement and cannot itself guarantee successful artifact upload.

Check remaining time before each child; cap its timeout to the remaining stage budget. A pair is complete only if both children finish successfully by D and its matched raw records validate. Never start another child at/after D. Kill/reap a child still running at D; preserve its partial stdout/stderr and the completed side as an incomplete attempt. Process-exit status and deadline reason must be distinguishable.

**Normal deadline:** retain the last completed scheduled look, even with later complete but unscheduled pairs. For example, a cap after 10 or 11 valid pairs retains n=8; after 14 retains n=12; after 7 produces Unavailable (`insufficient_pairs`). Extra valid pairs remain in raw data, not in displayed means or intervals. A pair interrupted by the global deadline is `incomplete_due_to_cap`, not an outlier, zero time, replacement observation, or measurement failure. If the deadline expires during warmup, affected unmeasured rows have no eligible look.

**Measurement/data failure:** a non-deadline crash, failed warmup, missing side, invalid/nonpositive/nonfinite time, noncontiguous/duplicate pair index, wrong order, mismatched counts/hash/plan, or failed measurement-validity condition makes that workload Unavailable, permanently for this job. Even if a prior look exists, it stays in raw `history` only; do not present a potentially selected surviving interval as the final row. Stop that workload, preserve the failed attempt, and continue other independent rows when possible. Global plan/binary corruption invalidates every affected row. Do not replace, silently skip, trim or winsorize observations; slow successful runs are valid data. No same-job restart/alpha reset after an unfavorable failure or label.

Validation failures outside performance, such as changed fidelity/traces, remain visible in their existing sections; do not turn a fast but behaviorally changed workload into a correctness claim.

At the time cap, write an ordinary report and raw artifacts while the reserved time remains; the benchmark's undecided status is not a required-check failure. Infrastructure/report-publication failures remain report failures, not performance evidence.

## 7. Required result fields

Per row retain `n_complete`, `n_used`, `last_look_n`, `termination` (`resolved`, `max_pairs`, `time_cap`, `measurement_failure`, `not_started`), label/reason, and a list `history` of every actually evaluated look. At a valid look retain n, df, mu_hat, gamma_hat, SSE, s, SE, critical, residual_floor, log interval, transformed interval/point, and both geometric per-operation means. Degeneracy uses null intervals. Unavailable has no final claim/interval; previous history can remain diagnostic.

For cap-at-10, n_complete=10, n_used=8, last_look_n=8. For failure-after-10, n_complete=10, n_used=0, last_look_n=8, final label=Unavailable and prior n=8 result appears only in history. For cap-before-8, n_used=0, last_look_n=null. `n_complete` counts validated in-budget measured pairs, never warmups or one-sided attempts. Record `n_attempted` separately if useful. Do not fabricate history for unobserved looks.

## 8. Guarantee, limits, cache interaction

Under the fixed-design independent-normal constant-variance model, `(mu_hat-mu)/(s/sqrt(n))` has a Student t_(n-2) distribution. Each of 30 possible intervals has noncoverage probability 1/600. The union bound gives simultaneous coverage at least 1-30/600=0.95, without cross-workload independence. Any incorrect Faster/Slower/Within claim requires noncoverage of at least one corresponding interval. Planned-look early stopping and selecting the last completed prefix at a time cap do not create an interval outside this finite union. Degenerate/failure suppression only withholds claims.

This is **model-based**, not distribution-free and not a promise of measured hosted-runner coverage. Non-normal tails, dependent errors, heteroscedasticity, warmup drift, changing treatment effects or label-dependent censoring can break the model. Calibration can reveal problems; a small calibration cannot prove its assumptions. No result is a guarantee across future commits, runner images or physical hosts. A materially changed measurement plan needs fresh calibration.

Build caching is welcome. Cache dependency/toolchain/lock/feature/flag inputs correctly; preserve accurate package rebuilding, PR-pinned measurement overlays and immutable hashed executable copies. Complete cache downloads/extraction, builds, tests and trace generation before timing. Symmetric fixed warmup applies to both binaries. Never leave a background build/upload active during samples, preferentially warm one side, or silently reuse the wrong binary. Cache speed itself does not alter the error allocation. Code/OS-page cache state during measurements does affect timings and must be controlled/calibrated. Same-binary A/A does not test variability across independently linked builds; this contract conditions on the two retained executables.

## 9. Regression artifacts and verification

- `statistics-reference.json`: constants; 15 synthetic SciPy OLS datasets with expected estimates/intervals/labels; 8 endpoint-boundary cases; 14 invalid-input mutations; 8 stopping histories; 3 block aggregation vectors. Individual math datasets are independent analyzer inputs, not necessarily legal continued-sampling histories; only state_vectors encode the runner's early-stop behavior.
- `generate_statistics_reference.py` plus its uv lock: Python 3.12+, NumPy 2.5.4, SciPy 1.18.1; independent least-squares oracle via `scipy.linalg.lstsq` and t.isf.
- `check_statistics_reference.py`: stdlib-only incomplete-beta constant check and balanced-regression/reference checks. Production tests should feed the JSON to the implementation under test; do not merely re-test this checker.
- `CALIBRATION-PLAN.md`: pilot/confirmation plan, variance components, m optimization and platform selection.
- `COMMENT-WORDING.md`: exact strings, with the existing layout preserved.

Reproduce offline:

```
uv run --locked --script generate_statistics_reference.py
uv run --no-project --python 3.12 check_statistics_reference.py
```

Numeric oracle tolerance: relative 2e-10, absolute 5e-12. Log endpoints/labels must use unrounded values; explicit nextafter boundary cases test strict/inclusive comparisons directly. Near-zero diagnostic SSE/s from degenerate SciPy fits need not agree relatively. JSON contains no literal NaN/Infinity; invalid mutations describe how tests materialize those values.

Sources: [NIST Bonferroni simultaneous intervals](https://www.itl.nist.gov/div898/handbook/prc/section4/prc473.htm), [SciPy Student t API](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.t.html), [Kalibera and Jones, rigorous benchmarking and repetition allocation](https://kar.kent.ac.uk/33611/45/p63-kaliber.pdf). The numerical degeneracy policy, workload/order literal, 60-second reserve and comment wording are engineering choices made in this contract.
