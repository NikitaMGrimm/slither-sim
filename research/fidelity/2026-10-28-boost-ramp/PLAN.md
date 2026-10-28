# Frozen offline boost-ramp investigation

This plan is committed before new scoring or raw-recording inspection. Prior
exposure: #233's formula map/ranked gap, #236's protocol/report, the historical
`speed-equations.md` summary and analyzer, and the beginning of the existing
speed-ramp fixture (sample values visible during provenance inspection). The
historical outcomes are already known. This is retrospective development work,
not a blinded replication or a new validation set.

## Pins, cohort and split

- Simulator base: `ffa6e032` (resolve full hash in results).
- Introducing Arena commit: `91c52fa5396c5a0afba632e60ca8bb8c9a84aa28`,
  `docs/speed-equations.md`, `tools/analyze-speed-equations.py`,
  `tests/fixtures/speed-ramp-validation.json`, `src/physics.rs`.
- Retained historical source/analyzer pin:
  `c2fb7285febb4273034e30699dc880715a334423`.
- Use every ramp in the existing fixture and the complete three named raw
  archives: `run3-large-radius-20260927/bot0.jsonl.gz`
  (`0317f6bbfb7075e019295727e41071e816799d4adbb97d4f6a06ce51748c2a94`),
  `run3-single-20260927/bot0.jsonl.gz`
  (`6343bca18a9dbf9a4a105e5d6968087d007a3a3eb198c9a8eb53d095ddcdf06f`),
  `visibility-covariates-20260928/bot0.jsonl.gz`
  (`46490bb6a7be12d5450b054c040c9266d55f3e1e0cc53dbc8d8baeec24762101`).
- All are development. They were historical separate-source validation, but
  neither their published results nor previously selected fixtures are fresh
  held-out evidence. Do not designate within-source tails as independent holdout.
  No other archive will be searched. `run1-boost` is historical discovery only:
  its incomplete archive is excluded from new analysis.
- Use the current repository's offline protocol decoder, pin its hash, and
  reconstruct the historical clock when checking exact fixture reproduction.
  No Rust client port is needed: physical speed bytes/counts suffice. Do not
  inspect, modify or build the owner's client checkout.

## Exact competing predictions

Use wire speed `v` (world velocity is `v/.032`) and seconds on accumulated
incoming elapsed-ms headers. Let `s(n)=clamp(1+(n-2)/106,1,6)`,
`b(n)=5.39+.4*s(n)`, `g(n)=14-b(n)`, `h_beta(n)=1+beta*(s(n)-1)`.
For direction u=rise/fall let a=+.015/.008 or -.02/.008.

1. P305: retain normalized progress q; `dq/dt=a/h_.305(n)`;
   `v=b(n)+g(n)*q` while q moves, q clamped [0,1]. Idle q=0 retains
   stored speed; full q=1 remains 14. Initialize q once from the first retained
   speed/count. Piecewise constant count uses exact linear integration.
2. V305: retain direct speed, `dv/dt=g(n)*a/h_.305(n)`, clipped to the
   current target (14 rising, b falling). No size-induced instantaneous refresh.
3. P0 and P390: same normalized-progress rule with beta=0 or .39.
4. P305_equal: use -.015/.008 falling (ratio 1 rather than 4/3).
5. P305_offline: multiply both rates by 1.3, an explicit negative control.
6. Historical empirical: `dv/dt=15.45/(1+.39*(s-1))` rising and -4/3
   of that falling. Score constant-size controls only.
7. For P305 and V305, compare size effective immediately at its received
   event versus delayed by one 7-ms loop. Also compute the full event-time
   sensitivity. These are conditional phase candidates, not identified ticks.

P305 and V305 are algebraically identical at fixed size. Report that identity
explicitly; do not attribute a fixed-size win to hidden progress retention.
Do not refit beta, ratio, initial state per later point, or timing to favor a law.

## Selection and exclusions

First recompute OLS slopes on every fixture interval. Independently reproduce
all intervals through the complete raw sources using the historical maximal
same-sign changed-speed runs: incoming speed differences nonzero, gaps in
(0,70] ms, trim first/last report, require >=4 retained points; split at count
change, appearance, removal or connection. Counts are decoded live anchors
after each packet, with fam separate. Require 5.39/.4/14 setup, protocol 19,
complete capture-end marker and matching archive hashes. Report any failure
before using an alternate estimator; do not silently replace the cohort.

For growth/loss, apply the same monotonicity/gap/trim/minimum rules, but do not
split at count changes. Require at least one decoded count change strictly
between first/last retained reports, and retain every qualifying interval,
including mixed growth/loss. Split on appearance/removal/reconnect/setup.
Count events include intervening anchor/tail packets, not just report counts.
Mass-only fam changes are not count changes. Unknown opponent controls remain
unknown: sign is an observational label. Do not treat a reversal, endpoint,
appearance speed or held unchanged speed as a clean interior acceleration.
Publish direction, count/event ranges, track/source counts and all failures.
Keep full derived rows external; bounded per-source/model aggregates and
sanitized witness indices/hashes enter Git.

## Scores, uncertainty and decision

Constant-size primary score: unweighted ramp OLS-slope RMSE, MAE and signed
residual in wire speed/s, separately by source, direction and size band
2..9,10..29,30..79,80..159,160+. Also report duration-weighted RMSE.
For OLS weights w_i, original quantizer half-cell slope budget is
`sum(abs(w_i))/36`; additionally use conservative full-cell budget
`sum(abs(w_i))/18` because the true encoder rounding policy is not established.
For a constant predicted rate r, add `abs(r)*sum(abs(w_i))*(.0005+j/1000)`
for per-report timing sensitivities j=0,7,14,42 ms. These are conditional bounds,
not confidence intervals or demonstrated event-to-send age limits.

Cross-size primary score: forward predicted speed from the first retained
point, no reinitialization at count events; RMSE/max error and per-interval
compatibility at j=0,7,14,42 ms. Difference-of-two-observation speed budget is
`2/18`; timing budget is maximum absolute slope across involved counts times
`(.001+2*j/1000)`. Add event-time sensitivity by shifting all count events
earlier/later by `j+7` ms, preserve order, and exclude scored reports within
`j+8` ms of any event (report missing exposure, never substitute intervals).
Use the envelope of nominal/early/late predictions plus the speed/time budget.
This envelope is a sensitivity check, not an exhaustive independent per-event
timing bound; do not claim a rejection of the full hidden-phase family from it.
Retain source/track aggregation: packets and ramps on one track are correlated.

Mechanics correction requires robust separation and causal attribution, not a
lower retrospective RMSE or compatibility with an arbitrary age bound. Stop
and report any supported correction; do not implement it.

## Power gate (authored, not captured)

Compute fixed-size ramp durations and pairwise separation for n=2,9,37,106,
214,343,430,500. For changing size, use one growth per 42-unit anchor at
instantaneous travel v/.032, full pending nutrition as a best-case ceiling;
normal loss is constrained by .155-s drops and the existing nutrition/mass
equations. Also report a deliberately generous upper bound of one segment
lost per drop. Never model a giant immediate segment-count jump as legal growth.
Evaluate full monotone rises/falls and partial cycling q between .2/.4, .4/.6
and .6/.8, durations capped at 30 s. Cycling cannot evade unknown input-phase
error: report transition count and accumulated worst-case slope/time error,
not only ideal-state separation. For old/new-size timing, bound one-segment
speed refresh and slope difference over 7 ms against speed bytes.

A proposed capture needs pairwise separation larger than the combined speed,
timing and input-transition budgets under a stated feasible control schedule.
Repeat intervals do not average away bounded systematic uncertainty. If no
legal discriminating pattern passes, state negative power and propose no
capture. If a scoped comparison passes, freeze its sizes, legal growth/loss
schedule, durations, interval counts and development/held-out allocation in
the report, marked **not approved**. Approval/collection is the owner's decision.

## Boundaries and delivery

Offline only, no game-server connection, simulator/fixture/contract edits,
official JS/assets/raw capture in Git, machine/account/endpoint/nickname data,
SSH/AWS, or client workspace dependency. One CPU; Python analysis serial and
affinity-limited, any Rust jobs/Rayon/tests=1 with external disposable target.
One research PR; this plan committed first, then report, scripts/focused tests,
bounded selected JSON, plain research-index row and one-line Added changelog.
Record this plan's commit hash and every source/tool hash in the report/results.
