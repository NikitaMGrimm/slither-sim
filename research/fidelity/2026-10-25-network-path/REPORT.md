# Retained official-server network path: effective delivery model

2026-10-25. Exploratory offline research, adapted from the local-only study below. The source analysis read retained official headless captures without game-server contact; local-simulator comparisons are separate. This record preserves measured, fitted and chosen quantities and adds the parent's adoption decision. It does not rerun the numerical study or independently verify the parent's implementation or localhost measurements.

The low-RTT route supports a fitted downlink model with lognormal ordinary variation, a slow correlated component and short wall-clock delivery pauses. This improves central delays and receive bursts over the historical demo profile, but p99 and autocorrelation remain mismatched. It models effective application delivery on one collector-to-server route; absolute propagation, uplink law, direction coupling, server processing and collector tails remain unidentified. There is no representative worldwide player-ping distribution or human-play verification.

## Provenance and access

All source study material is **local-only**, outside Git. Paths and SHA-256 identify retained bytes; they do not make the data downloadable or authenticate official behavior. Full captures are not redistributed. The study used a Codex agent and read only Arena's `recordings/headless`, `fidelity`, `ecology` and `references`; it never read `recordings/community` or Battledome data. Official classification follows retained study/endpoint headers, not present-day IP ownership or region inference.

| Local-only source path | SHA-256 |
|---|---|
| `C:\Users\Nikita\.t3\scratch\network-model-study-20261025\REPORT.md` | `9cf24c8d903205e648ed509c09ddcdb4d246227d24aee6b48be883fba4386035` |
| `C:\Users\Nikita\.t3\scratch\network-model-study-20261025\REPRODUCE.md` | `f6f58eae7768c80be1f0d1ebc9b9995259891794a3e17ba555e719fc3ac2a9f5` |
| `C:\Users\Nikita\.t3\scratch\network-model-study-20261025\ATTEMPTS.md` | `645ae614a6ec32942a128b7dcb606673f31d2544a10c64358104fdf0bf5e25aa` |
| `C:\Users\Nikita\.t3\scratch\network-model-study-20261025\input-sha256.json` | `a1c7cb060c0e70a3322fe21721e08366d463cc431379a89c2af1baf670720728` |
| `C:\Users\Nikita\.t3\scratch\network-model-study-20261025\artifact-sha256.json` | `d258c9e25989123f3be6df33760063d3643ace94a6cee678d46a9dea7479a0b2` |

[source-pins.json](source-pins.json) preserves the 2,608-input manifest's path, digest and count, the 435-entry artifact manifest, material source pins, every analysis script, and selected result digests. External artifact/script names below have local-only paths and digests in that file. Most belong to the original study directory; the separate jitter-study sources appear under material source inputs. Historical source paths/hashes are preserved rather than refreshed against the parent's concurrent edits. The study inspected SlitherSim HEAD `9e4b6300f7cf10d1fe018fecaa22d79e18dab609` and Arena HEAD `aab1149460c0f5606fbdc7d73e295ba79d094789`. Its final worktree status showed concurrent changes in client/host sources and tests; the study made no repository edits or claim that the checkout remained clean.

[proposed-default-profile.json](proposed-default-profile.json) (1,459 bytes) and [validation.json](validation.json) (9,062 bytes) retain the small research outputs with LF normalization only; original and selected byte hashes are distinguished in the pins. The proposal's historical status/schema is preserved. This selected record is not a packaged, verified `evidence-bundle-v1` archive; raw streams, full arrays and source snapshots remain outside Git.

## Corpus and selection

- **Official headless:** 91 compressed JSONL archives, 68.9 MiB, 1,928,130 post-setup frames and 81,843 matched game pings. Declared endpoints are public IPs on port 444. Quality audit: 53 `legacy_unknown`, 38 `quarantine` for missing terminal receipts, including 13 without gameplay setup and two without incoming frames. All gzip payloads reached valid EOF. Usable historical prefixes do not acquire modern verified provenance.
- **Local comparison:** nine archives, 31,084 frames and 2,319 matched pings, never fitted as official data. All 100 official/local compressed SHA-256s differ. `captures.json` retains every audit and `segments.json` every epoch and eligibility flag.
- Eligible setup epochs require at least 1,000 frames and 30 s after a one-second burn-in, reject schema/archive/server-identity problems and permit only the historical `missing_terminal_receipt` issue. No-setup, challenge, tiny-connection and reconnect gaps are excluded. No session is selected for low measured jitter.
- **Primary endpoint 162.19.235.91:444:** train 21 sessions / 113 setup epochs / 12,604.027 s / 1,085,656 frames / 47,486 pings; holdout six sessions / 32 epochs / 3,004.739 s / 243,270 frames / 11,277 pings. Successive lives and paired bots are correlated observations.
- Split by SHA-256 of the parent capture-directory name: first eight hex digits interpreted as an integer, modulo four equals zero for validation. Every bot/life in a directory stays together; `run2-survey` is held out whole. Directories are historical batch proxies; cross-directory overlap was not evidenced but cannot be ruled out. Failed/short survey attempts are not a population ping sample.
- **Retrospective exploratory holdout:** early validation summaries were inspected during method design. Numerical optimization used training targets/schedules only. Future independent dates/collectors are needed for stronger validation; frame counts do not create independent trials.
- The 2,474 fidelity browser chunks contain 49,947 `ws-in` objects: 1,330 elapsed-enabled without sequence, 1,327 sequence-enabled, the remainder without elapsed. When sequence is enabled the first two bytes are a counter even if elapsed=true; never sum them as milliseconds. Browser results are supplemental only. Ecology lacks paired elapsed/receipt clocks and references contain no network gzip captures.

## Measured timing and attribution limits

**Game RTT.** Official archives contain zero `ping-sent`/`ping-rtt` metadata events. Reconstruct outgoing byte 251 to the first unambiguous logical `p`, resetting outstanding requests at close/reconnect. There were no overlapping requests and 73 censored pings. Independently extracted counts/means match the copied quality audit for all 100 archives; metadata/wire cross-checks apply only to newer local captures and share the audit's matching logic.

Pooled primary-route RTT, milliseconds:

| Set | Pings | p1 | p10 | p50 | p90 | p99 | Maximum |
|---|---:|---:|---:|---:|---:|---:|---:|
| Train | 47,486 | 6.229 | 8.576 | 13.357 | 17.278 | 40.756 | 244.211 |
| Held-out | 11,277 | 6.216 | 8.123 | 13.220 | 16.976 | 34.247 | 157.682 |

Training variance is 49.18 ms² within epochs versus 3.61 ms² between epoch means (93.2% within); holdout 31.56 versus 1.46 ms² (95.6%). These ping-weighted descriptive components are not a player-population estimate. Typical epoch-median RTT is 13.50 ms (train p10–p90 12.46–13.99); a 24.09 ms-median epoch is retained. Median within-epoch SD is 4.60 ms (p10–p90 3.06–10.71). Robust RTT trend median is −0.004 ms/min in training, +0.069 held out; the −41.64 ms/min exception remains. A random-walk drift is unnecessary for typical short epochs. Winsorized ping lag-one ACF medians are 0.065/0.054 at ~263 ms sampling, which misses short events. RTT includes callback, pong/input scheduling and both directions; a 20/35/50 ms demo mixture is unsupported for this low-RTT route but can remain a chosen geographic scenario.

**Downlink diagnostic.** In each setup epoch, cumulative decoder clock `S_i` resets to zero; arbitrary setup headers, challenges and reconnects never accumulate as gameplay intervals. With receipt `T_i`, offset is `O_i = T_i − S_i`. Fit a Theil–Sen slope through offset p1 in 5 s bins, subtract `slope*(T_i−T_0)`, then subtract the minimum residual within ±10 s using a time-indexed monotone deque. This centered 20 s floor uses future samples and is exclusively offline. Slopes have median 23.81/22.98 ppm (train/holdout), ~2.4 ms per 100 s. The floor/slope can remove slow real queueing or sustained shifts, so retain raw spans, line-only residuals, slopes and sensitivity; they do not calibrate physical oscillators or identify arbitrarily slow path changes.

Typical epoch excess p50/p90/p99 is 1.70/3.50/8.82 ms in training and 1.65/3.24/8.44 ms held out; pooled p99 is 15.03/9.21 ms because bad epochs remain. Modeling uses median-session features below. Time ACF samples last receipt on a 10 ms grid, excludes ages >50 ms and winsorizes at epoch p99. Sample-hold/cadence effects can explain some 10 ms ACF; that value alone does not establish network memory. Direct frame-lag ACF at lags 1/2/5/10/100 is 0.265/0.186/0.127/0.099/0.033 in training and 0.256/0.210/0.134/0.128/0.039 held out. Longer time correlation requires more than independent noise, but may arise in path, server or collector.

**Spikes and compressed arrivals.** Excess >10 ms forms 7,518 runs (0.596/s) in training and 1,056 (0.351/s) held out. Median observed duration is zero because most runs have one sampled receipt; median surrounding duration brackets are 29.60/31.54 ms. Duration p90/p99/max is 34.12/362.66/3,602.90 ms in training, 6.94/69.51/231.24 ms held out. Peak p50/p90/p99 is 14.44/32.83/72.00 and 14.71/37.58/90.52 ms. One exponential duration misses long exceptions; thresholded run incidence is not the fitted underlying pause hazard. Receipt gaps <1 ms occur in 1.472%/1.036% of transitions; requiring elapsed gaps ≥5 ms leaves 1.414%/0.988%. Distinct server-time frames arriving together support delivery compression, without identifying TCP recovery, socket batching or collector blockage as its cause.

Changing the floor to 5/20/60 s gives pooled train p50 1.610/1.712/1.778 ms and p99 14.646/15.025/15.346; holdout p50 1.455/1.554/1.623 and p99 9.125/9.210/9.297. Central findings and the tail mismatch survive, but sub-0.2 ms distinctions are weakly identifiable. The fitted lognormal median depends on this gauge; it is not measured propagation or the excess median.

**Cadence.** Exactly 7 ms accounts for 47.67%/36.82% of primary elapsed intervals; 6–8 ms for 68.01%/66.46%. Modes near 14/21/28/42 ms and no zero elapsed intervals are already server timestamp/cadence structure. Subtracting elapsed time leaves post-timestamp socket/work delay and integer-clock error. Adjacent 6/8 ms cannot fully separate quantization from send-time variation; do not accumulate independent ±0.5 ms header errors as a random walk. One complete WebSocket frame shares one receipt/delay across its logical packets. Frames at a shared emit instant share the slow state; independent fast draws belong at actual frame boundaries.

The elapsed-only browser stream has two epochs (713 frames/16.29 s and 615/5.50 s) with excess p50/p90/p99 1.57/2.89/6.43 and 2.40/5.20/19.75 ms; RAF p99/max is 12.51/29.20 ms. Both declare `57.129.37.42:444`. They support the ordinary scale but are too short/instrumented for tail or stock-client feel validation.

**Collector attribution.** Retained collector source timestamps `performance.now()` at the start of receive, before recording/decoding: application callback time, not NIC/kernel arrival. Historical official headers lack collector hashes; current source context does not verify their collector version. None of the 91 official archives has collector-timing telemetry, so no official tail can be classified as network-only or removed as a known collector stall. Disk/gzip work after timestamping differs from dispatch blockage before it. Local telemetry has `lateTicks=0`, max timer gaps 39.75–41.05 ms and zero exclusions. The 25 ms probe records gaps >75 ms only; zero does not exclude relevant 5–40 ms disturbances. The future mask excludes `[previous timing meta−100 ms, current meta+100 ms]` only when cumulative lateTicks rises, not whenever an old maximum remains high.

## Uplink and conditional RTT fit

Client send/receipt clocks and first authoritative effects constrain only aggregate uplink + input gating + downlink + observation cadence. No server receive timestamp or synchronized clock identifies absolute direction shares. Subtracting detrended pong downlink excess leaves train residual p1/p10/p50/p90 ~6.15/7.39/11.75/13.92 ms, still including constant delay, uplink and pong scheduling. Epoch RTT/downlink-excess Pearson correlation p10/p50/p90 is 0.328/0.461/0.697; it establishes neither independence nor shared physical congestion.

**Chosen:** mirror the downlink uplink law, independent directions, 50/50 base split and uniform 0–7 ms pong wait. **Fitted conditional on these choices:** effective base RTT 4.9737 ms, not physical propagation. `rtt-fit.json` retains variants.

| RTT quantile | Held-out equal-session mixture, ms | Illustrative model, ms |
|---|---:|---:|
| p1 | 6.477 | 7.760 |
| p10 | 8.474 | 9.433 |
| p50 | 13.383 | 12.852 |
| p90 | 17.014 | 16.637 |
| p95 | 20.191 | 18.369 |
| p99 | 35.555 | 35.766 |
| p99.9 | 90.614 | 70.738 |

Held-out KS is 0.0889 and Wasserstein 0.564 ms. Low and extreme RTT tails remain wrong. Allowing a 14 ms or free wait width materially shifts base RTT and worsens central fit, demonstrating non-identifiability rather than a discovered server rule. Bidirectional RTT autocorrelation has not been validated. Do not apply the chosen pong wait to all player inputs as a measured network law.

## Endpoint transfer

Regions are unknown. Endpoint RTT differences describe this collector's routes, not worldwide player frequencies. Pooled eligible epochs, times in ms:

| Declared endpoint | Sessions | RTT p50 | Downlink excess p50 / p90 / p99 |
|---|---:|---:|---:|
| 103.4.30.88:444 | 4 | 207.07 | 1.81 / 3.49 / 11.75 |
| 162.19.235.91:444 | 27 | 13.34 | 1.68 / 3.68 / 14.40 |
| 198.244.231.35:444 | 2 | 29.13 | 1.68 / 3.37 / 14.69 |
| 57.129.37.42:444 | 2 | 13.69 | 1.58 / 3.07 / 8.43 |
| 94.72.180.82:444 | 5 | 118.70 | 1.83 / 4.37 / 19.05 |
| 95.216.38.155:444 | 1 | 28.87 | 1.73 / 2.96 / 27.44 |

| Endpoint | Sessions | KS | Wasserstein, ms | Observed / modeled p99.9, ms |
|---|---:|---:|---:|---:|
| 103.4.30.88 | 4 | 0.133 | 0.297 | 61.15 / 47.12 |
| 198.244.231.35 | 2 | 0.072 | 0.250 | 69.21 / 43.87 |
| 57.129.37.42 | 2 | 0.071 | 0.276 | 42.54 / 42.09 |
| 94.72.180.82 | 5 | 0.093 | 0.366 | 61.67 / 47.92 |
| 95.216.38.155 | 1 | 0.127 | 0.635 | 101.63 / 45.08 |

The ~1.6–1.8 ms central excess across very different RTTs supports separating baseline RTT from ordinary variation, without establishing a universal tail. Transfer uses fixed primary-trained parameters, deterministic per-epoch subsamples and all eligible sessions per endpoint. It is separate from the six-session primary holdout. One-session endpoints cannot estimate population variation; bootstrap resamples sessions, not frames or bots.

## Fitted model and held-out validation

For a connection and one direction, `X(t)` is a unit-variance Gaussian OU state, with transition

`X(t+Δ) = exp(−Δ/τ) X(t) + sqrt(1−exp(−2Δ/τ)) ξ`, `ξ ~ N(0,1)`.

At each complete frame emitted at `s_i`, draw independent `Z_i ~ N(0,1)` and calculate ordinary extra delay in milliseconds:

`D_i = m * exp(σ [sqrt(c) X(s_i) + sqrt(1−c) Z_i])`.

In wall-clock time, draw Poisson pause starts at rate `λ`; each pause lasts `L ~ Exp(mean ℓ)`. Merge overlapping intervals. For `v_i = s_i + B_direction + D_i`, let `G(v_i)` be the end of the merged pause containing `v_i`, or `v_i` if none contains it. Deliver at

`A_i = max(A_(i−1), G(v_i))`.

Order belongs to the connection's TCP direction; reset clocks/state on a new connection. This produces catch-up bursts without independent additive delays for every message held behind the same pause. The pause is an effective block, **not** a claim of packet loss, retransmission or router congestion. Ordinary variation is nonnegative and right-skewed; the OU state gives slow correlation while per-frame noise provides the fast component. No unbounded random walk, multi-second plateau episodes, per-connection random hazard multiplier, follow-on episode parameter or Pareto tail is supported strongly enough to require it.

The source fit evaluates OU on a 5 ms internal grid with interpolation, starts the filtered state at zero, samples emissions on the retained server-elapsed schedules, and applies the same centered 20 s floor to synthetic excess. This is reproducible but only a numerical approximation to the continuous OU equation; it is **not** a proposed 5 ms delivery quantization. At study time, connection-start transients and whether to use a stationary initial draw remained choices; the adoption section records the parent's subsequent choice. The profile carries exact fitted parameters, not a claim that three decimal digits are known physically.

| Parameter | Fitted value | Evidence status |
|---|---:|---|
| Ordinary median `m` | 1.901396 ms | Train fit in chosen offset gauge |
| Log SD `σ` | 0.506615 | Train fit |
| Correlated log-variance share `c` | 0.177068 | Train fit; weakly identifiable |
| OU time `τ` | 1.837406 s | Train fit; weakly identifiable |
| Pause rate `λ` | 1.646261/s | Effective application-delivery fit |
| Mean pause `ℓ` | 13.933565 ms | Effective application-delivery fit |
| Base RTT | 4.973681 ms | Conditional RTT fit; not physical propagation |
| Uplink share | 0.5 | Chosen symmetry assumption |
| Uplink law/direction coupling | Mirror / independent | Chosen, not separately validated |
| Pong processing wait | Uniform 0–7 ms | Chosen approximation; handling study owns refinement |

### Fitting and held-out checks

All 21 training-session targets contribute equally: calculate quantiles per session across its epochs; compute time ACF/burst features with epoch frame-count weighting inside each session; take the median across sessions. Optimize six parameters against six quantiles, ACF at 10/50/100/500/1,000 ms and fraction of receipt gaps <1 ms. Quantile errors are scaled by the target (at least 1 ms), ACF errors by 0.20, burst error by 0.012; p99.9 receives half weight. Monte Carlo prediction uses three ≤10,000-frame schedules selected at fixed frame-rate percentiles among long training epochs (two source sessions), with fixed random streams. This reduces cost but is an approximation to all training schedules; evaluation then uses every eligible epoch, fresh deterministic seeds and identical actual server schedules for both models.

Bounded differential evolution used 1,050 evaluations, seed 415, one worker, and stopped at its 24-iteration budget **without convergence**. The result is a reproducible candidate, not a proven optimum. No parameter-confidence interval is claimed. A per-frame rare-spike family was tried first; it made event incidence depend on broadcast cadence and was replaced by wall-clock pauses. The pause-overlap implementation was corrected to merge blocks before the final fit; prior variants remain in the directory. Validation spectra/quantiles were not used as the numerical optimization objective.

The table is **median-session features**, with a 4,000-replicate session-cluster bootstrap interval for the observed held-out median. Only six held-out sessions make these intervals coarse and conditional on this source/date. Synthetic columns use the measured emission schedules to isolate delivery; they do not validate the simulator's own broadcast cadence. The historical demo comparison implements ±1 ms jitter, per-connection exponentially drawn 0.05/s episode rate, 3 s exponential duration, 2–4 ms extra RTT split 50/50, shape 2, ordering, and no tick quantization. A constant RTT mixture disappears from detrended excess.

| Feature | Held-out observed | Proposed | Historical demo | Observed bootstrap 95% interval |
|---|---:|---:|---:|---:|
| p10_ms | 0.5847 | 0.6332 | 0.2106 | 0.5262–0.731 |
| p50_ms | 1.627 | 1.564 | 1.035 | 1.486–1.75 |
| p90_ms | 3.314 | 3.47 | 1.849 | 2.928–3.651 |
| p95_ms | 4.621 | 4.49 | 1.95 | 3.797–5.177 |
| p99_ms | 9.979 | 12.95 | 2.958 | 6.583–12.5 |
| p99.9_ms | 40.65 | 43.97 | 3.822 | 21.73–46.96 |
| acf_10ms | 0.4873 | 0.3854 | 0.4852 | 0.3476–0.5818 |
| acf_50ms | 0.1713 | 0.1188 | 0.1654 | 0.102–0.2131 |
| acf_100ms | 0.1411 | 0.1178 | 0.1549 | 0.0997–0.1782 |
| acf_500ms | 0.08713 | 0.09047 | 0.1197 | 0.05206–0.1003 |
| acf_1000ms | 0.03991 | 0.05931 | 0.09097 | 0.03622–0.04974 |
| burst_lt1ms | 0.009833 | 0.01369 | 0 | 0.00577–0.0176 |

For an equal-session subsampled CDF, downlink training KS/Wasserstein is **0.0657/0.306 ms**, held-out **0.0366/0.154 ms**. Historical-demo held-out values are **0.3003/0.897 ms**. KS is an effect size; correlated frames invalidate naive IID p-values. ACF is calculated identically for measured and simulated receipts.

The proposed model matches central delays much better than the demo and generates receive bursts. It still overpredicts median-session p99 by about **30%**, underpredicts 10–100 ms correlation in this final fit and overpredicts 1 s correlation. Some simulated values fall outside the empirical bootstrap interval. It does not capture every long plateau or rare >100 ms event. **Do not present this profile as sufficient evidence for “exactly like a real server.”** If tail fidelity is the priority, use the retained data as a regression target, keep these failures visible, and collect the instrumented confirmation batch below before setting a production default.

The complete [JSON proposal](proposed-default-profile.json) preserves its original research schema and evidence labels. It is not a ready-to-load `LatencyConfig`: adoption maps its `ordinary` and `deliveryPauses` fields to the API below. The proposed route describes a retained low-RTT route; broader base-RTT strata/weights remain scenario choices.

## Local server comparison and host scheduling

Same per-setup clock detrending and **time-based 20 s rolling floor** as above; `delayspread.py` instead uses ±600 frames, so its effective window varies with traffic and connection length. The differences for unstable captures are not a contradiction. Every local archive had zero reported >75 ms collector stalls.

| Local capture | Duration, s | Excess p50 / p90 / p99, ms | Wall span minus server span, ms | Fitted clock slope, ppm |
|---|---:|---:|---:|---:|
| anchor-only-150 | 149 | 8.89 / 16.47 / 24.38 | -4.5 | 1.7 |
| final-proxy | 89 | 17.55 / 147.78 / 961.15 | 913.2 | 317.2 |
| hop-direct | 39 | 11.82 / 18.36 / 24.50 | -25.2 | -8.0 |
| hop-direct-120 | 39 | 1.29 / 7.33 / 11.55 | 0.0 | -11.6 |
| hop-proxy | 39 | 12.96 / 20.02 / 25.89 | 1.1 | 3.4 |
| ours-anchor-tick | 89 | 13.30 / 27.55 / 68.30 | 2.0 | -8.8 |
| ours-b6 | 89 | 16.90 / 109.63 / 286.50 | 85.6 | -2.5 |
| stagger-direct | 44 | 61.62 / 444.68 / 700.93 | 479.6 | 6187.5 |
| stagger-proxy | 44 | 3.52 / 14.94 / 39.78 | -18.9 | 3.3 |

`hop-direct` phase medians for server elapsed modulo 42 ms are **phase 7: 1.175 ms** versus **phase 21: 20.605 ms**, a 19.43 ms range. `anchor-only-150` ranges **1.377–17.782 ms** over its six tick phases. That shape is tied to the host's six-tick work/broadcast cycle, not iid frame jitter. The 120-bot capture reduces p50 from **11.82 to 1.29 ms** and p90 from **18.36 to 7.33 ms**, with remaining phase spread and nonzero tail: reducing background load helps substantially but does **not** establish complete removal of scheduling error. Direct/proxy paths both retain large phase patterns with 480 bots; the proxy alone is not the explanation.

The configured congestion adds **2–4 ms to RTT**, hence only **1–2 ms to one-way delay** at a 50/50 share; independent jitter is ±1 ms. At fixed model state, its delay range is 2 ms; over all episode states it is at most 4 ms. Ordering can hold frames behind earlier frames but does not create a stationary 19 ms phase-dependent range from these bounded draws alone. A further 7 ms delivery quantization can add up to one tick for each tick-gated stage. It can explain part of the low-load spread but cannot by itself explain the large six-phase pattern. The phase/load evidence and the supplied local report support host work/scheduling as the dominant extra source.

`stagger-direct` shows **+479.61 ms wall-minus-server progression over ~44 s** and a **6,187 ppm** fitted slope; this is host simulation time falling behind wall time, not a physical remote clock-rate estimate. `final-proxy` and `ours-b6` have large nonstationary tails; subtracting a rolling floor can hide part of accumulated lag. Keep raw spans, maximum residual and lateness metrics alongside a jitter summary. Some unstable captures have different path/config phases and are not controlled A/B experiments.

The supplied local-only jitter-study `REPORT.md` (full path and SHA-256 in [source-pins.json](source-pins.json)) attributes broadcast-held anchors to a 42 ms cadence, reports anchor correction p50 **3.59→1.53→0.97 world units** for original/per-tick/120-bot cases and profiling bottlenecks in pellet-sector rebuilds and seeker decisions. These correction/profile figures are **supplied study evidence**, not independently recomputed by the source analysis; the source study recomputed the raw timing/phase/excess values above. An anchor delayed until the next broadcast is a send-cadence/modeling issue; queue delivery and host overload are separate timing issues. Changing only the latency RNG cannot repair either.

Application implications from the source study: use monotonic wall-clock deadlines, keep emission/server clocks distinct from delayed flush, preserve per-direction order and drain all due messages after pauses. Record due/actual times and lateness so blocked host work cannot masquerade as modeled jitter; an internal OU grid is not delivery quantization. Each player's connection needs its own link state; bots act from delivered observations and uplink arrivals, while geometric `serve` observations keep their contract. Add only the missing delay for remote humans who already traverse a real route: the full synthetic profile cannot be added on top and called the target RTT or subtract physical Internet delay. Pong/input waits remain separate and must not be double-applied. Recheck low-load/120/480-bot loopback behavior after host scheduling changes; changing the RNG cannot repair broadcast-held anchors or overload.

## Adoption

This section records the **parent-reported simulator adoption**, being implemented alongside this research record. It is not a new fit or independent runtime review by this documentation task.

- `LatencyConfig` replaces uniform `jitterSeconds` and `congestion` episodes with `variation {medianSeconds, logSigma, correlatedShare, correlationSeconds}`: per-message lognormal extra delay combines a per-direction Ornstein–Uhlenbeck correlated state and independent fast noise. The implementation starts the OU state with a **stationary initial draw**, a chosen difference from the study's zero-start 5 ms grid approximation; the study's held-out results do not independently validate that initialization change.
- `pauses {ratePerSecond, meanSeconds}` supplies per-direction Poisson delivery pauses with exponential lengths. Overlapping pauses merge; every message due inside a pause waits until its end. Each direction stays ordered. These effective delivery blocks are not identified loss/retransmission/congestion events.
- Defaults still add **no delay**. Opt-in `LatencyConfig::measured_route()` uses base RTT **4.974 ms**, `uplinkShare` **0.5**, variation median **1.901 ms**, `logSigma` **0.5066**, `correlatedShare` **0.1771**, `correlationSeconds` **1.837**, and pauses **1.646/s**, mean **13.93 ms**. Mirrored uplink and independent directions are **chosen, not separately validated**; base RTT remains conditional on the pong wait assumption.
- The host answers input and pings in the tick they arrive, with pongs leading the frame: server-side wait **0–7 ms plus compute**. This matches the study's chosen 0–7 ms pong wait that conditions the fitted base RTT; it does not turn that wait into a measured official input law. The parent reports this input-timing change at base commit `73698e6`.

**Parent-reported local verification:** localhost, Windows desktop shared with other load, release build, zero added latency, headless collector. Median game-ping RTT after the input-timing change:

| Bots | Before, ms | After, ms |
|---|---:|---:|
| 64 | 13.88 | 6.90 |
| 480 | 14.16 | 7.12 |

These supplied measurements were not rerun here, are separate from the historical study's local comparison, and do not establish official-server fidelity or isolated-host performance. No raw artifact path/digest was supplied for them.

## Capture plan (not executed)

The strongest missing evidence is collector/server attribution and direction decomposition. A fresh capture cannot identify absolute one-way propagation without synchronized/known server timestamps; do not promise that from client data alone.

Freeze a plan before acquisition. At **at most two concurrent official-server connections**, run three separate 10-minute batches on the primary low-RTT endpoint, then three separate 10-minute batches on a second official endpoint in the ~25–35 ms route class. Keep batches separated in time; reserve the last complete batch per endpoint as untouched validation. Sequential batches mean no more than two connections at any moment. If the endpoint is not in the current official list, select a listed equivalent and pin its IP/port; never substitute community/custom servers.

For each batch, use one current headless collector and one stock-client-compatible connection/probe if feasible, with the exact headers/collector source/options and raw frame bytes retained. Record ping sent/received metadata, frame setup boundaries, high-resolution monotonic callback timestamps, per-second event-loop-delay histograms/maxima (including **1–75 ms**, not just >75 ms), timer expected-vs-actual times, memory/GC/backpressure counters, and the stock browser's RAF gaps and packet handling costs. Record client output buffering and controlled, isolated input changes whose causal effects the input study can identify. Do not add a third connection for monitoring.

Keep gzip integrity receipts, source/plan hashes, all failed attempts and censoring; log raw wall-clock span as well as gameplay time. Predefine exclusion of known collector-stall windows, report both unfiltered effective delivery and the clean subset, and freeze success criteria for quantiles, burst incidence and ACF before viewing validation. If network-only tails are required, arrange an approved independent kernel/socket-arrival timestamp trace; callback timestamps alone cannot settle that attribution. No collection or privileged tracing was launched by this research task.

## Attempts, reproduction and remaining limits

The original `ATTEMPTS.md` (local-only path/digest above) retains failed and superseded work:

1. Broad gzip-as-JSONL discovery was stopped before completion; the final version bounds header reads and separately inventories browser JSON. An `np.bool` serialization failure was fixed and rerun.
2. Null browser RAF gaps were filtered, then wrongly concatenated setup clocks produced a spurious ~16.4 s jump. `details-initial-browser-unsplit.json` preserves the invalid result; final `details.json` splits epochs.
3. `fit-per-frame-v1.py` tied spike incidence to cadence (train objective ~0.0346, held-out KS ~0.0684). `fit-poisson-unmerged-v1.py` missed extensions from overlapping blocks (KS ~0.0449); final `fit.py` merges intervals and refits. Their scripts/results remain preserved, not interchangeable with final parameters.
4. `rtt_fit-floored-gauge-v1.py` used floor-subtracted samples; final RTT fitting uses raw positive delays and absorbs the offset into base RTT. Neither identifies physical propagation/asymmetry. The final optimizer's iteration-limit stop is retained.
5. A historical collector path was absent. The study inspected and hashed SlitherSim's live `tools/fidelity/slither-headless.mjs` instead; exact source provenance for old captures remains unknown.

The original `REPRODUCE.md` (local-only path/digest above) gives the complete offline recipe. The source run used Windows, Python 3.13.13, NumPy 2.5.3 and SciPy 1.18.1; `validation.json` records the platform/executable. Numerical threads were capped at one, at most two analysis jobs ran concurrently, no worker pools or child agents. Exact stochastic reproduction requires these versions and frozen inputs; the optional uv environment recipe was not executed/validated by the study.

From the original study directory, using its pinned interpreter (historically `C:\Users\Nikita\miniconda3\python.exe`), run sequentially with `python -B`: `verify_inputs.py`, `test_analysis.py`, then `extract.py`, `analyze.py`, `browser_inventory.py`, `details.py`, `fit.py`, `rtt_fit.py`, `validate.py`, `build_report.py`, `test_analysis.py`, `seal_artifacts.py`. Verify input hashes **before** regenerating output; missing/changed inputs block exact reproduction. Do not refresh discovery or silently substitute new inputs. Capture roots are explicit in the scripts. Superseded fit scripts reuse generic output names: investigate them only in a separate scratch copy, preserving final outputs.

**Source-study verification:** four `test_analysis.py` tests passed (rolling minimum against brute force, known drift removal, cumulative stall-counter masking, deterministic/nonnegative/ordered simulation); all 100 archive RTT counts/means matched the copied audit. These are reported historical checks, not rerun results for this adaptation. Top-level retained artifact/script hashes were checked against `artifact-sha256.json` while creating the record; the full 2,608-input hash verification and numerical pipeline were not rerun. LF-normalized selected JSON parses and retains the source content. No Rust build/runtime, installed-host or human-play test is claimed here.

The remaining limits are historical acquisition/collector provenance, retrospective six-session validation from one environment, weak correlation/tail identification, p99/ACF and rare long-plateau mismatches, unvalidated uplink/direction coupling and RTT time structure, base RTT conditional on chosen pong wait, absent player-population sampling and no human-feel verification. The capture plan is a proposal; this task did not collect data or contact any game server.
