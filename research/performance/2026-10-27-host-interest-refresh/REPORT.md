# One food-interest refresh per tick, 2026-10-27

Normal ticks refreshed subscriptions twice. Keep the first pass before any
broadcast error; standalone broadcasts still refresh same-time edited poses.
Subscription rules stay unchanged; only broadcast.rs changes.

Measured `4cd6ce5` -> `99e84b7` after #224/#226; [pins/results](results.json).
WSL2 Linux x86-64, Rust1.99 locked
release/debug=1, default target. Jobs/Rayon/tests one,
at most two pinned CPUs; offline official-500, seed7, no bots.

## Selection and profile

Coarse legacy emission is 0.91M instructions/tick, conversion 0.36M. Fine probes
put anchors/food together at 0.115M; 623/906 messages pass unchanged. A borrowed
framing screen saved 0.01/0.67/1.18% stock/half13/all13, below the 2% threshold.
Pre-#224/#226 runtime59e075e matched terminal summaries, without full traces.
Rejected patch/logs remain local-only; no conversion cache kept.

All13 residual split on the measured arms: million user instructions/tick, probes
included; redundant row is nested. Same 853 ticks: >=120, 500 alive, reconnect720
excluded, PMU>=99%. Diagnostic/control traces match.

| Residual component | Base | Candidate |
| --- | ---: | ---: |
| Interest, both calls | 0.457 | 0.278 |
| Of which redundant | 0.153 | 0.000 |
| Recipient plumbing | 0.365 | 0.365 |
| Update preparation | 0.206 | 0.206 |
| Input/actions | 0.152 | 0.151 |

Other shifts include compiler/probe effects. All13 fallback is slightly below 2%.

## Matched evidence

| Official500 | Reduction | Pair range |
| --- | ---: | ---: |
| stock | 2.111% | 2.109-2.111% |
| half13 | 2.010% | 2.008-2.011% |
| all13 | 1.932% | 1.930-1.932% |

One warmup then five alternating whole-process instructions:u pairs, >=99% PMU,
none excluded; setup,1080 ticks and drains included, sockets/kernel excluded.
Controls every6 ticks, reconnect720; minimum499 alive, terminal500, three rejoins,
zero refusals. All15 complete traces match: stock/half13/all13, U5/U250/U500 and
nine historical cases, including frame boundaries/times, checkpoints and RNG.
Integrated full Rust group, two new regressions and 11 visibility tests pass.
Removing standalone or tick refresh fails controls; restored tests pass.

36 socket holds, 10s warmup/12s sample: reductions (median; range) stock 1.08% (0.56 to 2.07%); half13 3.32% (1.17 to 4.46%); all13 2.33% (1.76 to 3.50%). Evolving worlds make these descriptive. One half13 warmup failed TCP connect; retained, then resumed unchanged gates.

## Reproduce and limits

`reproduce.py MODE ARCHIVES OUT`: build, trace, count (--perf PERF), sockets,
pairs. `profile.py ARCHIVE OUT --cpu N --control BINARY` splits overhead;
`emission.py` uses that interface. Python3.12+, patch/user PMU required;
scripts reject -O, arms use separate targets/copied rehashed binaries. Raw data
stays outside /tmp; host0/load1.

Arena-vision overlap: interest loop/broadcast signature. Shared-machine timing
is contaminated; perf_event_paranoid remains2, kernel instructions
unavailable. No CPU-time, latency or capacity claim.
