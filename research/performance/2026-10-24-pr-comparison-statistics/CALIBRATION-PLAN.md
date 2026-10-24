# Manual hosted-runner A/A calibration plan

2026-10-24. Proposed runs only; none have been dispatched. This plan supplements STATISTICS-CONTRACT.md. A/A means both roles invoke the **same hashed executable**, not separately compiled copies of the same source. Both roles have equal inputs, block counts, arguments and internal warmup. The label must not alter code paths, paths that affect loading, or environment. All child runs are serial within a job.

## What must be instrumented first

The existing harness gives one summed operation timer per process and per-operation quantiles. It does NOT expose nested, reset-identical repeated blocks from fresh processes. That is insufficient to identify both between-process-pair and within-process variance from one aggregate batch length. Do not infer those components from the number of deterministic physics ticks or fit a two-component formula to three aggregate timings.

Add an explicit block mode. A block reproduces the current workload trajectory with the same seed, operation count and fixed internal warmup, starting from an equivalent restored state. State preparation/reset and output destruction are excluded from operation timing but INCLUDED in elapsed cost. A process performs m blocks and reports all m wall totals W_k, their operation counts, and the total process elapsed time. A snapshot of the warmed initial state is acceptable only if it recreates the same workload and has a fixed policy on both sides; repeated warmup/setup costs must not be silently ignored when computing optimal batching.

For m>1 production, the primary per-process scalar is the geometric mean W_k, so the pair observation is the average block log ratio. This is an explicit extension of the measurement plan, not just increasing the old iterations parameter. Keep the old m=1 behavior. Per-operation observations are correlated by simulator state; never count them as independent process repetitions. If block/reset equivalence cannot be implemented, keep m=1 and compare aggregate batch plans empirically; do not claim an identified m_opt decomposition.

Freeze timer scope/clock and retain instrumented operation times without subtracting a guessed timer overhead. Record the timer implementation/resolution. A separate empty-timer diagnostic can reveal an instrumentation floor, but does not measure the simulator's uninstrumented speed or justify automatic subtraction.

## Pilot: all ten workloads, three batch lengths, both OSes

**Yes, compare Linux and Windows.** Use the images currently configured by ubuntu-latest and windows-latest, recording their resolved image/version, CPU information and Rust toolchain/target/flags. Prefer explicit image names for the final plan when available, while recognizing hosted physical CPUs can still vary. Rust and harness revisions, feature set, inputs and seeds match across platforms; the executable hash naturally differs across OSes. Never combine cross-OS observations into one t interval. This is a runner/protocol selection experiment, not a claim that Linux users see the same effects as Windows users.

Keep each block's original catalog operation count K; vary **m = 1, 2, 4 reset-identical blocks/process**. Thus timed work is K, 2K or 4K operations per process, without extending an evolving trajectory or exceeding the 100000-iteration limit in any individual block:

| Workload | K per block | candidate operations/process |
|---|---:|---:|
| step-default | 100000 | 100000 / 200000 / 400000 |
| step-food | 10000 | 10000 / 20000 / 40000 |
| step-body | 50000 | 50000 / 100000 / 200000 |
| observe | 1000 | 1000 / 2000 / 4000 |
| portable | 5000 | 5000 / 10000 / 20000 |
| protocol | 5000 | 5000 / 10000 / 20000 |
| official-100-step | 1000 | 1000 / 2000 / 4000 |
| official-100-bot | 150 | 150 / 300 / 600 |
| dome-proxy-32-step | 20000 | 20000 / 40000 / 80000 |
| dome-proxy-32-bot | 400 | 400 / 800 / 1600 |

One pilot job handles one workload group, all three m values, and six fresh A/A pairs per workload/m. Use these groups, balanced approximately by the saved Windows process costs:

- Group A, in this order: step-default, portable, official-100-bot, dome-proxy-32-step.
- Group B, in this order: step-food, step-body, observe, protocol, official-100-step, dome-proxy-32-bot.

Run **two independently provisioned jobs per OS/group**, for eight pilot jobs total: 2 OSes x 2 groups x 2 independent replicates. Reusing a VM for both replicates is not an independent host replication. Every workload/m/OS cell has 12 fresh pairs across two jobs, with nested m block timings per side. These are calibration counts, not the production look schedule.

In each job, run and retain a discarded warmup pair for each workload/m before counted sampling. Then for pair round r=1..6, visit workloads in group order; for each workload run its three m settings in the following exact order:

```
r=1: 1,2,4       r=2: 1,4,2       r=3: 4,2,1
r=4: 2,4,1       r=5: 4,1,2       r=6: 2,1,4
```

This uses all six permutations and balances position of m within a workload's round, including within each execution-order sign. For each m use the first six pair orders from the contract, `+,-,-,+,+,-`, giving three of each order. Every A/A pair still runs adjacent serial processes. No early stopping or production classification during the calibration pilot. Do not sort away high-noise jobs, discard stalls, or restart a job because its plots look bad.

Keep the same 620-second stage cap and 15-minute hard timeout/reserve for each job. Under a conservative linear scaling from the saved run, the pilot costs roughly 1085 sampling seconds per complete ten-workload replicate across the three m values, or about 18.1 minutes split over the two groups. Two OSes x two replicates is roughly **72 runner minutes of measurement**, plus eight jobs' setup/build/upload time. The groups project at about 8-10 measurement minutes each. These are planning estimates, not a reason to exceed the cap; new block/reset overhead or hardware can cause an incomplete pilot. Preserve incomplete cells and report them; do not fit a fully populated experiment that did not run. No dispatch is authorized by this document.

## Model, estimators and optimal batching

For a workload, OS, fixed m and job g, pair r, block k, let

```
x_grk = log(W_PR,grk) - log(W_base,grk)
x_grk = mu_g + gamma_g * order_gr + b_gr + e_grk
Var(b_gr)=v_b, Var(e_grk)=v_w
y_gr  = mean_k(x_grk)
Var(y_gr | job, order)=v_b + v_w/m
```

This is a **paired** variance model: common host drift shared by the adjacent sides can cancel. Do not estimate v_b/v_w from unpaired per-side times and ignore covariance. b is independent across fresh process pairs and e across blocks only under this nested model. mu_g/gamma_g allow job-specific A/A bias/order effects; they do not make an arbitrary dependent sequence valid. Report job-to-job changes separately; two hosts cannot establish a distribution of every future hosted runner.

For each job/m, with r=6 balanced pairs, estimate y's intercept/order regression and its residual mean square `S_y^2=RSS/(r-2)`. Also compute

```
WMS = sum_r sum_k (x_rk-y_r)^2 / (r*(m-1))       # m>1
v_w_hat = WMS
v_b_hat_untruncated = S_y^2 - WMS/m
v_b_hat = max(0, v_b_hat_untruncated)
```

Pool sums of squares and their degrees of freedom across the two jobs for a cell, retaining individual-job estimates. Use m=4 as the primary decomposition and m=2 as a consistency check; m=1 directly estimates total paired residual variance. Retain negative untruncated component estimates as diagnostics: zero truncation is a modeling convention, not evidence that between-process variance is truly zero. Obtain uncertainty by simulating/refitting the nested model, or by an offline bootstrap respecting process-pair clusters/order and job strata; two jobs do not support a precise host-population CI. Report estimates/ranges, not a falsely precise universal m_opt.

Fit pair elapsed cost `C(m)=h+c*m` within each job/workload using all three m settings, then summarize jobs. h is fixed pair-level process/setup overhead actually amortized by more blocks; c is **full elapsed** marginal matched-block cost, including reset/setup not in W. Check this linear cost model against observed costs. If it fits poorly or gives a negative h/c, use the observed grid costs directly and do not present the analytic optimum as measured truth.

For measurement budget B, n approximately B/(h+cm), so

```
Var(mu_hat) approx (h+cm)*(v_b+v_w/m)/B
f(m) = h*v_b + c*v_w + c*v_b*m + h*v_w/m
f'(m) = c*v_b - h*v_w/m^2
m_opt = sqrt(h*v_w/(c*v_b))
```

For positive components, `f''(m)=2*h*v_w/m^3>0`: this is the unique continuous optimum **under that model/objective**. Round by comparing feasible integer/grid values, not by blindly rounding the square root. If v_b=0 and v_w>0, the model has no finite optimum and favors more blocks; use the tested cap, not infinity. If v_w=0 and v_b>0, choose m=1. With both near zero, insufficient resolution/model information prevents a meaningful optimization.

Compute `Q(m)=estimated Var(y|job,order) * observed seconds_per_pair` for m=1/2/4. Smaller Q means greater information per elapsed second: with budget B, mean-estimator variance is approximately Q/B. Also inspect m-dependent order effects, block-position drift, tails and serial dependence. A low Q does not excuse bias or bad assumptions.

**m_opt is a dimensioning guide, not a proof of optimal sequential CI latency.** Eight pairs is the minimum production look and fixed build cost is outside its objective. A long batch can reduce variance while still delaying every comment. As candidate production maps compare (a) all m=1, and (b) the per-workload tested-grid Q minima. Do not interpolate to an unmeasured m or change K without a separate pilot. Model/simulate both maps with the actual 8/12/26 stops, serial round-robin allocation and 620-second/deadline caps, retaining bootstrap/host uncertainty. Reject any workload/m that cannot plausibly complete a single 26-pair confirmation series within a 540-second target including its warmup; this avoids proposing an unconfirmable long-batch protocol.

## Diagnostics and independent confirmation

Select the OS/plan provisionally from the pilot, then freeze it and collect **two new independent confirmation jobs per fixed workload group, with 26 pairs per workload and no early stopping**, at its selected m. Make confirmation groups from pilot costs *before seeing confirmation results*: require `26*sum(pair_costs)+sum(warmup_costs) <= 540 seconds`, using the larger observed job cost estimates for planning. A group can be one workload; do not split one 26-pair series across hosts and treat it as a constant-intercept run. Both confirmation jobs get the contract's full 26-element order sequence. If the budget stops one, retain incomplete data and report that the planned confirmation was incomplete.

This yields 52 fresh held-out pairs per workload on the selected platform, plus nested blocks, without selecting a protocol on its own confirmation noise. At m=1, whole-suite confirmation measurement cost projects about **20.1 runner minutes** for two 26-pair series including warmup, divided into groups. Larger m can increase this; cap each job and report the finite manual budget before dispatch. This is a one-time calibration proposal, not recurring CI work.

For pilot and confirmation, report:

- mu_hat (A/A role bias) and gamma_hat, with ordinary explicitly diagnostic intervals and job identity. For A/A, the target treatment effect is zero; a reproducible nonzero intercept indicates bias or model failure. In the log model, `2*gamma` is the difference between mean log ratios for base-first and PR-first orders; gamma is not directly a per-side first-run percentage.
- v_b, v_w, paired residual SD and Q by workload/m/OS/job, h/c and the feasible batch decision. Compare Windows/Linux using this information and full expected comment latency, including separately observed build/test/trace costs. Raw A/A speed alone is not the selection criterion.
- Residual time-series plots, Q-Q plots, tail/stall counts, and residual scale by order, pair index, and block position; lag 1..3 correlations and time trend. Plot all data, not a cleaned subset. Compare diagnostics with simulation envelopes that refit the same intercept/order design: fitted residuals themselves are correlated by regression, so naive IID residual p-values can mislead. Block autocorrelation invalidates `v_w/m` scaling; group-level variance changes/order interactions invalidate the simple t regression. The first/last portion of each job also checks fixed warmup adequacy.
- Using held-out full series, replay the production scheduled looks and early stops offline. Record interval noncoverage of zero, any wrong directional A/A label, pair count at stopping and modeled/capped comment latency. A/A alone measures neither power for real regressions nor treatment-dependent variance. Validate the statistical code separately by synthetic normal paired data with order effects and known injected log shifts (0, log(.90), log(1.10), and threshold-near cases); multiplying logged times by a known factor tests analysis code, not the simulator's physical response or real-runner power.

Do not use a single normality test as a pass/fail proof. Fifty-two pairs can reveal gross drift/tails but cannot verify IID Gaussian errors or a 5% rare-error bound. Even with zero failures in R independent full comparisons, a one-sided 95% binomial upper bound is `1 - .05^(1/R)`; getting it below 5% needs **59 independent full comparisons**, not 59 overlapping windows from one trace. This pilot/confirmation is not sized to empirically certify 95% family coverage. The formal claim remains conditional on the model. If important diagnostics fail, report that and repair/recalibrate the protocol; do not silently choose a robust-looking subset, switch tests after seeing the result, or call the labels calibrated.

## Selecting the production platform/plan

Use this predeclared ranking, with estimated uncertainty reported:

1. Exclude protocols with an unresolved systematic A/A role bias or clear residual/variance-scaling problem. This is a reason to inspect/repair, not to pick a better-looking replicate.
2. From each OS's two candidate maps, evaluate predicted capped resolution probabilities for true 0% and +10% time changes, expected full comment latency, and the observed job/host range. Use conditional Gaussian simulation with fitted variance/order terms, using both conservative and central estimates; those are forecasts, not measured power.
3. Prefer candidates reaching the 80% per-workload resolution target for both scenarios under the capped forecast; among them minimize expected whole-comment latency averaged equally over the two scenarios. If none reaches the target, maximize the minimum per-workload resolution across the two scenarios, then minimize latency. State that the target was missed instead of quietly increasing alpha, the practical band or time budget.
4. Freeze the provisional winner and confirm it independently as above. Do not re-optimize on confirmation without declaring a new pilot/confirmation round. If differences are small compared with host uncertainty, prefer the simpler m=1 map and faster-build platform; do not assert a statistically established platform winner.

Linux is a sensible candidate because operation wall time is portable and historical CPU comparability is no longer the objective. It should become the default only after checking these measurements and any feature/platform support. Cross-OS calibration observations and build-cache behavior do not change a PR row's arithmetic, alpha allocation or label boundaries.

Record a calibration manifest with source/harness/plan/binary hashes, toolchain/flags, OS image, runner/CPU metadata, exact m/K/reset/warmup/order schedule, raw records including failed attempts, cost model and diagnostics, selection rule/output, held-out confirmation and remaining caveats. Recalibrate after material harness/workload/warmup/toolchain/runner changes or recurring drift; never silently retune using the PR being classified.

Theory: [Kalibera and Jones, 2013](https://kar.kent.ac.uk/33611/45/p63-kaliber.pdf) develops nested variance/repetition allocation and time-efficient dimensioning. The paired log-block specialization, finite grid, exact pilot matrix, cost caps and platform ranking here are proposed engineering choices; they are not claims that the paper prescribes these numbers.
