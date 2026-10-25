# Official protocol-19 packet scheduler: retained-capture study

2026-10-25. Exploratory, offline research, adapted from the local study identified below. The analysis used read-only repositories and contacted no server. This record adds adoption notes for simulator commit `20dd5a4`; it does not rerun the study. Full derived files remain local-only. The observations below constrain a scheduler, but **do not identify a complete exact official implementation**. In particular, there is no official physics-tick number, inbound server timestamp, or hidden world-event timestamp in these recordings.

The most consequential findings are independent release paths for anchors, changed motion, mass, food/sector maintenance, prey and HUD; shared server-wide food/HUD phases; and prompt anchor delivery. A six-tick batch of all gameplay packets is inconsistent with the traffic. A fixed per-snake six-tick heartbeat is also inconsistent with the silence and variable report intervals.

## Corpus, provenance and reproduction

The primary corpus is **91 headless archives, 72,230,171 compressed bytes**, in 45 retained capture directories. Of these, 78 contain setup frames: **400 setup epochs, 1,928,130 gameplay frames, 4,112,091 logical packets and 23,792.723 summed connection-seconds**. These seconds include simultaneous clients and post-death traffic; they are not independent server-hours. Twenty IP/port endpoints have usable setup traffic. Thirteen archives have no gameplay setup. Fifty-three archives retain `capture-end`; 38 older archives lack that footer, so their terminal completeness is unknown, even though all gzip streams and decoded records parse successfully. There were **zero framing, protocol-version or tracker errors**.

`inventory.json` preserves every headless header. `manifest.json` records each compressed capture's SHA-256, endpoint, scenario, ending, setup signature, epoch counts and cache name. Admission requires the header and every reconnect endpoint to occur in a retained `active:true` server-list header containing `sid` and `cluster`. The collector's source obtains that list from `https://slither.io/i80124.txt`. There are 33 historical endpoint allowlist entries. This establishes retained acquisition provenance, not present-day ownership or server software identity. No `recordings/community` path was traversed.

The supplemental scan checked **2,474 fidelity browser chunks** (382,993,247 compressed bytes), with SHA-256s in `browser.json`. All inspected sockets target the retained official-client host `57.129.37.42`, ports 443/444. Most streams lack elapsed headers and are not used as elapsed-clock samples. `20260927-headers/elapsed` supplies 1,326 usable post-setup elapsed gaps: median 8 ms, mode 7 ms, consistent with the main corpus. The `sequence` stream's header increments per emitted frame, rather than furnishing an independent physics-tick index. `ecology` contains 26 state-hook archives without raw frame headers; `references` contains images. Their contents cannot supply the missing clocks. Supplemental parsing also had zero errors. These scans are kept separate from the primary denominators.

Source SHA-256s are in [source-pins.json](source-pins.json). The analysis imports a scratch copy of `slither_protocol.py`, with contraction disabled because scheduling does not require rendering old body anchors. Setup resets the server clock to zero; its sometimes-65535 header is an epoch value, not a gameplay interval. No gap crosses setup, connection, or a known entity reappearance. Packet counts include post-death observations. First full `s` after setup identifies the recipient's snake, matching the stock client; no mid-life archive is used to infer a new own ID. `verification.json` records that the comparison file `simulation.rs` changed concurrently under the parent task; this study did not edit it, and preserves its initial source pin. The decoder, collector and stock-client pins remained unchanged.

Reproduce with standard-library Python, from any directory:

```powershell
python C:\Users\Nikita\.t3\scratch\scheduler-study-20261025\run_all.py
```

This refreshes only derived files in the original local study directory, serially, using one logical core. The exploratory run used at most four simultaneous single-thread analysis processes. `extract.py` creates local trusted pickle caches; do not substitute untrusted pickles. `finalize.py` verifies all input SHA-256s again. The complete serial reproduction passed: seven main analysis outputs matched byte for byte, and the census matched canonically after correcting nondeterministic JSON key serialization (`reproduction-check.json`, `RUN_NOTES.md`). All input hashes matched on recheck. `census.json`, `deep.json`, `inputs.json`, `relations.json`, `supplement.json`, `anchor-phase.json`, [order-and-witness.json](order-and-witness.json) and [key-results.json](key-results.json) contain the complete histograms, summaries and witnesses behind the numbers below. Quantiles use nearest rank. Counts describe correlated observations; no frame-count-based confidence intervals are claimed. [key-results.json](key-results.json) also gives capture-batch variation. The existing repository `packet_census.py` was run read-only with `python -B` on the motion-clock archive: its 27,657 frames/56,574 packets include three challenge frames, agreeing with this study's 27,654 gameplay frames. That corroboration shares the protocol decoder and is not independent ground truth.

## 1. Tick period and frame frequency

**Measured.** Among 1,927,730 post-setup headers: median **7 ms**, mean 12.342 ms, p90 21 ms, p95 33 ms, p99 76 ms. The main bins are 7:810,790; 6:242,436; 8:208,041; 14:202,606. **65.43%** are 6–8 ms. Positive longer gaps often lie near integer multiples of seven. A count-weighted nearest-lattice squared-error fit over gaps 5–70 ms, with candidate periods 6.70–7.40 ms in 0.01 ms increments, minimizes at **7.00 ms** (6.99 is almost tied); RMS residual is approximately 0.77 ms. This is an emission-gap fit, not a precise physics-clock estimate.

To account for cumulative rounding, fit consecutive dense runs whose individual gaps are 5–9 ms. Across 848 runs of at least 40 frames, local fitted periods have median **7.050 ms**, p05–p95 **6.869–7.131 ms**, full range **6.727–7.273 ms**. Their fitted cumulative residual ranges have median 2.741 ms. **829/848** cannot fit *any* constant period and phase with only a one-ms-wide accumulated timestamp-quantization band. Cumulative `server_ms mod 7` is essentially uniform (274,353–277,241 observations/bin).

**Inference and uncertainty.** A nominal seven-ms server loop with emission/service jitter is well supported. Exact seven-ms wire emissions, or a universal fixed 7.x-ms emission clock, are rejected by these tests. A fixed internal physics step could still coexist with variable send timestamps. Dense runs are selected for observable traffic, so their periods are not an unbiased estimate of every server loop. Independent rounding of every delta, timestamp differencing, queueing and tick work cannot all be distinguished, although the close long-run receipt/server-clock agreement argues against flooring away a fraction of a millisecond on every frame.

**Frames per tick.** Every decoded gameplay frame contains at least one logical packet. There are no zero/one-ms post-setup gaps; **10,799 gaps are 2–4 ms**. No two successive frames share the same integer server time. This supports one aggregate emission opportunity per loop and omission of empty emissions, but does **not prove** at most one frame per physics tick: the clock has no tick label. Longer gaps cannot reveal whether hidden ticks were empty. A retained counterexample to literal seven-ms spacing is `anchor-encoder-holdout`, connection 1: frame S=25848 follows a 23-ms gap, then S=25851 follows after only 3 ms. One 999-ms header occurs after approximately 1003 ms of receipt time; a header cap is possible but not established by one observation.

## 2. Frame composition and ordering

**Measured.** Frames contain 1–561 packets, median **2**, p95 **5**, p99 **10**, mean **2.133**. Anchors and motion co-occur in **322,630 frames**; mass/tail and motion in 208,522; food-eat and motion in 133,189; food births and motion in 70,637. Pong, minimap, leaderboard, sector maintenance and prey can share a gameplay frame. `census.json` gives the full family-pair co-occurrence matrix and both order directions. There are **zero frames with two reports for the same snake**, and zero with two anchors for the same snake.

Useful ordering constraints: same-snake motion precedes its anchor in **74,057** ordered pairs, with no reverse pair; same-snake mass/tail precedes motion in **47,684** pairs versus **27 reverse** pairs. Pong precedes motion in 51,169 pairs, with no reverse; map precedes motion in 4,610 pairs, with no reverse. Pong is first in **81,830/81,843** pong frames, so “always first” has 13 counterexamples. When `w` and ordinary `f` coexist, **all clears precede all f packets in all 13,624 frames**.

**Uncertainty/counterexamples.** A universal family sort order is false: across different snakes, families interleave; food-birth/sector order occurs both ways, as does appearance/listing order. A full `s` can coexist with an anchor, and appearances/removals can coexist. These constraints are partial ordering evidence, not a full queue reconstruction. One aggregate frame per tick remains inferred as in question 1.

## 3. Anchors, tail removals and fam

**Measured anchor timing.** There are **855,359 anchors**: `g` 687,227; `G` 98,131; `n` 64,140; `N` 4,818; `=` 1,043; `+` **zero**. For ten-anchor windows with constant reported speed and body count, predict an interval `42 * 32 / sp` ms. The middle inter-anchor gap's distance error is `gap * sp / 32 - 42`. On 68,884 own-snake windows its median is **+0.158 world units**, p05–p95 −0.927 to +2.330; 280,566 other-snake windows give median +0.477, p05–p95 −1.750 to +2.330. Reported/retained speed and physics speed need not be identical, which explains why this check is not a zero-bias timing oracle.

The neighboring-window speed estimate `9*42 / (S[last]-S[first])` removes slow speed bias. The resulting absolute correction is own median **0.343**, p95 **1.366**, RMS **0.746** world units; other median **0.629**, p95 **2.121**, RMS **1.131**. Outliers remain, including 33.28 world units for another snake. Windows overlap and are not independent trials. Receipt-clock errors are larger and include network coalescing; they are retained in `deep.json`.

An additional absolute-phase check avoids relying solely on gaps, which cannot detect a constant packet delay. Decode each full `s` head coordinate and latest body anchor; project head progress along the reported heading, and predict `(42-progress)*32/sp`. Select nearly straight cases with progress −0.5..42.5, cross-track error <1 unit, heading changes ≤0.025 rad and speed changes ≤0.03 before the first anchor. For **3,792 other-snake cases**, actual minus predicted first-anchor time is median **+2.829 ms**, p05–p95 **−4.884..+7.804 ms**, range **−20.432..+63.213 ms**. For 93 own cases the median is −4.124 ms; join state can already be several ms old. This check assumes the full-s head is current authoritative position and the body endpoint is the latest anchor; quantization, curved motion, unreported speed changes and setup age limit it.

**Inference.** Prompt emission on creation, normally within a server-loop opportunity, is supported; holding anchors to a recipient-wide 42-ms broadcast is inconsistent with the combined timing evidence. Exact creation tick is not directly observed. Anchor phase is independent of motion: own anchors occur as much as 70,335 ms after the last rotation report while continuing normally. There is no measured phase formula linking anchors to ID or motion reports.

**Tail/fam are separate.** `r` occurs 8,255 times and always carries six bytes, including fam. Consecutive same-snake short tail-removal gaps for others have median **155 ms**, p05–p95 **151–157 ms** (797 gaps ≤250 ms); longer gaps are often multiples, reflecting mass/segment thresholds. This is not evidence to remove a tail unconditionally every 155 ms. Standalone `h` behavior is in question 9. No `+` timing is directly measured; treat its grouping with growth anchors as a semantic inference.

## 4. Motion reports: trigger, phase, own versus others

**Measured.** There are 112,480 own and 1,599,420 other-snake reports. Within an uninterrupted appearance, consecutive report gaps are:

| Recipient relation | Gaps | p05 | Median | p90 | p95 | Maximum |
|---|---:|---:|---:|---:|---:|---:|
| Own | 112,119 | 42 | 63 | 205 | 236 | 70,405 ms |
| Other | 1,587,591 | 41 | 44 | 204 | 206 | 16,966 ms |

For others, the largest modes are 42 ms (344,675), 43 (183,984), 41 (176,357), with substantial 48/49/50-ms modes. Across batches with at least 20 gaps, other-snake median gaps range 42–55 ms; own medians range 43–161 ms. Thus a universal 42-ms period is not an adequate description.

Field presence is decoded from packet length/type, not assumed from a broad rotation event. Other-snake speed-bearing gaps have median **42 ms**, p05–p95 **41–85 ms**, while wanted-angle-bearing gaps have median **48 ms**, p90 **254 ms**. The 200–206 and 250–258 ms modes depend on which fields are changing; they are not global frame clocks. A straight/stable snake can continue producing anchors while emitting no motion for many seconds. Reports that leave the decoded angle/wanted/speed tuple unchanged are uncommon but real: **222 own** and **11,209 other**; turn direction and higher-precision internal values are not represented in that tuple. An unconditional 42-ms heartbeat and “never report an unchanged quantized tuple” are both false.

Reports are staggered across recipients' ticks: among 1,121,172 motion-containing frames, **782,631 contain one snake**, 230,625 contain two, p95 is three, maximum 23. A retained simultaneous-phase witness is `anchor-encoder-holdout`, connection 1: snake 503 reports at S=7583,7624,7666,7708,7750,7792 while snake 410 reports at 7589,7631,7673,7715,7757,7799. Their phases differ by roughly one loop. Other snakes share phases; this is not a unique-ID slot allocation.

Cross-recipient matching uses equal `(snake ID, decoded post-report angle/wanted/speed)` tuples with a unique match within 30 ms receipt time. Across **13 paired batches / 59 overlapping epoch pairs**, 173,786 matched reports align after one connection-origin shift: **171,318 exactly**, all within ±1 ms. The same snake's release phase therefore appears shared across recipients, rather than restarted on each appearance. Median spawn-to-first-report time is 48 ms for others, but ranges **3–16,905 ms**, excluding a universal “first report six ticks after appearance.”

**Inference/uncertainty.** Change-driven reporting with a roughly 40-ms/six-nominal-tick throttle fits better than a fixed heartbeat or recipient-wide broadcast. The exact dirty-value comparator, minimum interval, phase/rearm rule and ID relationship remain **unknown**. There are 68 own and 2,741 other gaps below 35 ms; the 31-ms witness near S=25848 includes a long preceding tick and then a 3-ms interval. Send-time variation can invalidate a literal wire-level 40/42-ms lower bound. The data do not justify coding `snake_id % 6`, an appearance-relative timer, or anchor-phase coupling as official behavior.

## 5. Visibility

**Measured.** There are 12,718 full appearances, 8,420 ordinary removals (`s`, four bytes, removal flag 0), and 1,858 death removals (flag 1). In frame units, **4,594/10,775 appearance frames** also contain food listings. Only **140** share ordinary-food `f` bursts, and **43** share sector clears. Appearances/listings occur throughout the 500-ms maintenance cycle: nearest-f phase has p05–p95 about −242..+231 ms for appearance, −248..+239 ms for `F`.

Ordinary removals split: **2,478/8,015 removal-containing frames** share `w`, **2,439** share `f`; the rest occur between maintenance bursts. Death removals share death/rapid-food `b` in **1,586/1,858 frames**. Last motion report to generic removal has median 38 ms, p95 745 ms, but is not entry/exit latency.

**Inference/unknown.** Introductions can be prompt as interest changes; there is a distinct roughly 500-ms cleanup path for some generic removals. There is no single recovered visibility-check period, and no known geometric event time from which to measure true notification lag. Full `s` alone cannot distinguish a world birth from view entry. A known paired target crossing a calibrated interest boundary is required to identify the remaining trigger. Do not treat every `s-` as a visibility exit, or force all appearances/removals onto the food clock.

## 6. Food and sectors

**Measured.** `f` 142,108 packets occur in **44,349 bursts**, median two foods/burst. Consecutive burst gaps: **median 504 ms**, p05–p95 **501–518 ms**, minimum **479 ms**; omissions can produce multiples or longer gaps. `w` 94,394 packets form 14,578 bursts, usually 6–7 sectors; burst-gap modes include **502/506 ms and 1510–1514 ms**, minimum 482 ms. **13,624/14,578** clear bursts share `f`, with all clears before all f packets. This is strong evidence for a shared nominal 500-ms maintenance opportunity, conditional on actual payload.

Cross-client alignment is decisive: **6,607** paired ordinary-food burst matches are all within ±1 ms after clock alignment, **6,422 exactly**. This is a server-wide phase, not a clock beginning at recipient setup. Missing `f` is not a skipped server task if there was no subscribed food payload.

`F` 93,971 listings occur in 16,353 frames, with a minimum inter-frame gap of **6 ms** and a broad phase relative to f. They therefore are not confined to the 500-ms task. No `W` occupancy packets occur. `b` has 181,929 packets, spread through the maintenance cycle; **1,586 death-removal frames** also carry b. Pickups are `<` 207,453 and `C` 107,991; uncredited `c` is absent. These pickup notifications share ordinary gameplay frames rather than a single food-birth clock.

**Inference/unknown.** Use separate prompt-event paths for subscribed b/pickup deltas and immediate interest-entry F, plus a shared ~500-ms f/clear maintenance path. Birth time and pickup contact time are not observed, so same-tick causality for b, f, `<`/C is not proved. Ordinary f may reflect maintenance-driven creation/notification rather than an earlier hidden event. `F` is an additive listing and `w` is recipient cache clearing, not a measurement of global creation/deletion. The full interest stencil, refresh/debounce deadlines and ordering between F and appearances remain unknown.

## 7. Prey

**Measured.** `y+` entries: 2,113; `j` corrections: 52,424; generic y removals: 1,564; eaten-with-eater removals: 312. Consecutive known j gaps after the first report: **50,348**, median **338 ms**, p05–p95 **334–340 ms**, full range **199–2,694 ms**. Across 43 batches with ≥20 gaps, median is **336–338 ms**. First entry-to-j gaps span **0–339 ms**, median 97; **612/2,076 are same-frame entry and j**. Appearance therefore does not reset a mandatory one-third-second delay.

There are **841 repeat gaps below 300 ms**. When the new reported speed is exactly 11, 247 gaps have median **215 ms**, p05–p95 **211–325 ms**; transitions away from 11 also produce short gaps. This is consistent with a faster flight/pulse update opportunity near 210 ms, but the precise transition logic is not reconstructed. Four successive prey reports have identical decoded state; pure silence-until-quantized-change is false.

Paired matches for 6,306 same-prey corrected states align within ±1 ms, **6,224 exactly**, after the snake-derived clock shift. A prey's phase is consequently shared across recipients and survives recipient appearance; no prey-ID phase formula is established.

**Counterexample/uncertainty.** The maximum retained j gap is **2,694 ms** for prey 41377 in `paired-prey-overlap/bot0`, connection 1, S=57473→60167, with **253 other incoming frames** between them. It is not a frozen connection, and no y reappearance bridges the interval. Generic y-removal timing relative to actual visibility loss remains unknown. Eaten y carries an eater but no contact timestamp. Use a ~333-ms threshold checked at loop opportunities as an inference, not an unconditional hard 338-ms packet timer.

## 8. Pong, leaderboard and minimap

**Measured pong.** 81,843 responses matched retained outgoing byte 251. RTT min **4.925 ms**, median **14.005 ms**, p95 **206.328 ms**, maximum 733.046 ms; geographically different servers are pooled here. **32,369** pongs are alone and the rest bundled. Pong-containing frame headers have median 8 ms and minimum 2 ms, without a separate same-timestamp response. This is consistent with input draining and pong queueing at loop opportunities. It does not measure delay after ping arrival at the server.

**Measured HUD.** Combine M and V, since a delta/full encoding switch is not a missing update. Across **9,161** minimap intervals: median **2504 ms**, mean **2503.994**, p05–p95 **2501–2507**, range **2497–2517**. Across **7,612** leaderboard intervals: median **3008 ms**, mean **3008.062**, p05–p95 **3003–3013**, range **2982–3035**. Long-epoch fitted periods have medians about 2504/3008 ms; the drift is not removed by averaging many updates.

First minimap after setup spans **0–2505 ms**, median 1223; first leaderboard **0–2989 ms**, median 1224. Paired aligned updates: **1,379 maps**, all within ±1 ms (1,346 exact); **1,163 leaderboards**, all within ±1 ms (1,140 exact). Reconnected recipients join the existing phase. This supports **global independent HUD clocks**, rather than setup-relative per-recipient 2.5/3-second clocks.

**Inference/unknown.** Nominal 2500/3000-ms thresholds serviced by the loop, with rearming/service cost, fit the observations. The precise rearm location and reason for the ~8-ms leaderboard excess remain unknown. A phase-locked timer averaging exactly 2500/3000 ms is not what was observed. Pong timing cannot isolate inbound latency or prove synchronous versus next-tick handling without a server receive timestamp.

## 9. Kills, fam, mass, credit and score

**Measured fam.** Standalone h occurs **477,112 times**. Same-snake h gaps for others: 433,019 gaps, median **114 ms**, strong **105/106-ms modes**; among 340,680 gaps ≤250 ms, median 106, p05 102, p95 201. Own h has 32,275 gaps, median 222; short-gap median 107. This supports a changed-fam reporting opportunity near **100 ms**, separate from motion/anchors. There are **754/433,019 other** h gaps below 100 ms (minimum 74), and **44/32,275 own** (minimum 0); it is not a hard timestamp lower bound. Growth anchors and r also carry fam and can bypass a standalone h message. All r packets here include fam.

For credited food eats with a later same-eater mass-bearing packet in the same appearance, **304,687** event pairs give median **9 ms**, mode 7, p95 **89 ms**, maximum **125 ms**; 17,051 are same-frame. This pairing includes h, growth anchors and r; it is descriptive and is not a causal contact-to-mass estimate. Eats with no later known mass packet are censored, not assigned a delay. The common next-loop case is consistent with food consumption occurring after an entity's current mass-report opportunity, while the tail is compatible with a roughly 100-ms dirty-mass reporting gate.

**Measured kills/credit.** There are **113 k packets**, carrying kill counts 1–4. None shares a frame with a death-removal s!, so a rule requiring same-frame kill-credit and victim removal is false in this corpus. Killer visibility and recipient interest can censor the relation; victim removal is not itself a precise world-death clock. There are **zero R packets** and no direct independent real-score-credit schedule to estimate. Leaderboard l cadence is above; game-over v occurs 197 times. No packet evidence supports a heartbeat for k or R.

**Inference/unknown.** Emit changed kill counts/event credit promptly and independently of gameplay batching, but exact k/R release deadlines are unknown. Do not derive score/credit cadence from absent R or confuse ordinary mass updates with the modern kiosk real-score mechanism. Resolve mass throttle/rearm exceptions with controlled pickup/boost trials before treating 100 ms as exact.

## 10. Input handling and server-clock mapping

`inputs.py` matches outgoing single-byte steering 0–250 and boost **253=on / 254=off** to the first matching own wanted-angle or speed change. Returned wanted angle uses the observed floor-quantized target `floor(byte*256/251)`; a match requires that exact byte. Input matching is observational attribution, not proof that an input caused every later state change.

**Clock method.** Fit the lower receipt envelope using one minimum-offset sample per five-second bin, obtaining local time ≈ `a + b*S`. In a ±5-second offline window, every receipt provides `a <= receipt-b*S`; a ping sent at q and answered at S gives `a >= q-b*S`. Take the intersection `[L,U]`. Thus command server time lies in `[(t-U)/b,(t-L)/b]`, without assuming RTT symmetry. Clock-fit drift, integer header quantization and send/receipt callback timing add about millisecond-scale model uncertainty beyond these conditional bounds. This is an offline estimate using future receipts, not an online synchronization algorithm.

Of **92,173 steering commands**, 87,542 find a matching reflection, 234 do not, and 4,397 lack a ping clock bound. Of **968 boost commands**, 776 match directional speed changes, six do not, and 186 lack a bound. Commands superseded before observation and unchanged states are not counted as proven application failures. The full candidate matches and bounds are in `input-matches.jsonl`.

For cleaner steering attribution, require a command held ≥150 ms, no later steering command before the reflection receipt, and a prior command target >2.1 angle-byte units away. There are 17,812 such matches; **15,364** have clock-bound width ≤10 ms across **30 batches**. In that narrow-clock subset:

| Send → first reflected wanted-angle | Median | p95 |
|---|---:|---:|
| Lower causal bound | 5.745 ms | 25.889 ms |
| Upper bound | 11.587 ms | 31.828 ms |
| Conditional interval midpoint estimate | **8.646 ms** | **28.897 ms** |
| Local send → receipt | 13.232 ms | 34.227 ms |

The midpoint estimate has p99 43.485 ms, maximum 149.035 ms; it is not an observed one-way delay. Lower bounds are intersected with nonnegative causal delay; the raw possibly-negative bounds are preserved. Batch midpoint medians span 3.553–9.143 ms, median 8.651. For **164** similarly held boost matches with clock width ≤10 ms, midpoint median **9.278 ms**, p95 **50.200 ms**; median lower/upper bounds are 6.190/12.549 ms. Only speed change is visible—there is no direct echoed boost bit.

**Inference/unknown.** Many inputs are reflected within roughly one nominal tick plus uplink delay, especially after a long quiet interval. Some wait substantially longer, consistent with report throttling, service variation and network delay. Reflection latency combines inbound transport, application and reporting; these data cannot prove application on the next physics tick or isolate its latency distribution. A symmetric RTT/2 subtraction would hide this uncertainty. The best recovered result is the bounded send-to-first-reflection distribution above, not a hidden input-to-application clock.

## Scheduler specification

This is a partial specification with evidence labels. “Measured” means the listed wire constraint; “inferred” is an implementation hypothesis. Unknown choices need an explicitly provisional setting or new evidence, rather than being silently called exact.

| Status | Per-tick action / packet family | Trigger, period and phase | Frame ordering |
|---|---|---|---|
| **Inferred** | Advance physics and drain pending inputs at nominal 7-ms loop opportunities; gather due recipient packets each loop. | Variable observed send-time spacing; exact internal step/wall-clock relation unknown. | At most one nonempty aggregate per loop is supported, not proved; do not impose a six-tick recipient-wide flush. |
| **Inferred**, strong | `g,n,G,N,=` new anchors; `+` by semantics only. | Release when created, independent of motion-report opportunity. Do not wait up to 42 ms. | **Measured:** same-snake motion, when both present, precedes anchor. |
| **Inferred** | `e,E,3,4,5,d,7` motion. | Changed authoritative fields/direction; typical busy spacing ~42 ms, compatible with a ~40-ms/six-tick throttle. Shared per-snake phase across recipients; no unconditional heartbeat. Exact comparator/rearm/ID phase **unknown**. | Partial per-snake order above; cross-snake queue order **unknown**. |
| **Inferred** | `h` fam. | Changed mass, nominal ~100-ms opportunity; preserve rare shorter reports. Growth anchors/r also carry fam. | Mostly before same-snake motion/anchor; 27 reverse mass/motion pairs forbid an absolute global sort. |
| **Inferred** | `r` tail loss with fam. | Segment-loss event; active short gaps ~155 ms arise from mass mechanics, not a mandatory removal timer. | Independent of anchor batch; exact event queue position **unknown**. |
| **Unknown** | Full `s` appearance / flag-0 removal / prey visibility y. | Prompt interest-entry path plus ~500-ms cleanup path is supported. Exact checks, geometry, age and debounce remain unknown. | Do not enforce one appearance/listing family order. |
| **Measured phase**, inferred task | Ordinary food `f` and some sector `w`. | Shared server-wide ~500-ms maintenance, observed ~504-ms gaps; payload only for interested recipients. | **Measured:** when co-bundled, every w precedes every f. |
| **Inferred** | Sector `F`, rapid/death food `b`, pickups `<,C` (unobserved c/W by semantics only). | F on interest entry; b/pickup deltas at event opportunity, independent of f maintenance. Actual event-to-send latency **unknown**. | Preserve compression cache dependencies; broader ordering unknown. |
| **Inferred** | Prey `j`. | Per-prey shared clock, nominal ~333 ms checked at a loop; faster flight/pulse opportunity near 210 ms. Entry does not restart phase; long omissions possible. Exact trigger/rearm **unknown**. | y+ and j may be same-frame. |
| **Inferred** | Pong `p`. | Queue response when ping processed; can co-bundle. Exact receive-to-send deadline **unknown**. | **Measured:** precedes motion; often first, with 13 exceptions to universally first. |
| **Measured phase**, inferred task | `M/V` map and `l` leaderboard. | Independent **global**, not setup-relative clocks; observed mean periods 2503.994 / 3008.062 ms. Nominal 2500/3000-ms thresholds with service/rearm drift inferred. | Map precedes motion in observed mixed frames. |
| **Unknown** | Death s!/v, `k`, `R` and other credit. | Event paths; death-food b often shares victim removal; k never shares s! here; R unobserved. No recovered exact period or deadline. | No universal same-frame kill/victim-credit requirement. |

An implementation can already remove blanket 42-ms batching, keep independent family queues/clocks, preserve global food/HUD phases across joins, and reproduce the measured partial ordering. The retained evidence does **not** justify advertising the remaining scheduler as an exact official clone.

## Adoption

Implemented by `9e4b630`, `c97b89e` and `20dd5a4` (relative to `964d28c`):

- Encode gameplay updates every physics tick (default 7 ms), omitting empty
  emissions. `SimulationConfig.broadcastTicks` is removed and rejected in JSON.
- Release changed quantized motion tuples through one gate per snake, at least
  six ticks apart, shared across recipients. Release changed standalone `h`
  mass through a separate shared 15-tick gate (105 ms at default `dt`).
- Release anchors promptly. Incremental per-snake ordering is tail/standalone
  mass, motion, then anchors; growth anchors carry their own mass. This adopts
  the common ordering while retaining the 27 reverse mass/motion pairs above.
- Make ordinary ambient births wire-visible when `floor(t / 0.504)` advances,
  sharing the sector-cleanup epoch. Boost/death `b` and pickups remain prompt;
  interest-entry `F` remains separate. Ambient physics generation is unchanged.
- Use one correction clock per prey shared across viewers: nominal one third
  second, or a 210 ms guard when the previous/current reported speed is exactly
  11 in protocol units. New viewers do not restart it.
- Run the real-time host on simulation, socket and bot threads. The socket
  thread sends at modeled delivery times; background seekers decode, decide,
  model processing, ping and rejoin through their own client slots. This is a
  local execution design, not evidence of official threading. `RunningHost`
  replaces manual polling; host `botThreads` defaults to 2.

The incremental food index and snake coverage grid are performance mechanisms
intended to preserve wire output under these release rules; they are not inferred
official data structures. Existing synthetic scheduler regressions in
[`simulation.rs`](../../../src/client/simulation.rs) cover shared motion/mass
gates, motion-before-anchor ordering, food maintenance and shared prey clocks.
Their assertions establish local behavior, not official equivalence.

Unknowns remain: exact official dirty comparators and rearm rules, ID/phase
relationships, visibility geometry and triggers, input-application tick,
event-to-send deadlines, mass exceptions, long prey omissions, kill/credit
timing and whether ambient creation itself waits for maintenance. The strict
six-/15-tick gates and regular prey corrections are model choices with the
counterexamples retained above. Shared official HUD phases and observed
2504/3008 ms drift have **not** been adopted: the simulator keeps its existing
connection-relative nominal 2.5/3-second schedules. The study is not an exact
official scheduler specification.

## Candidate regression windows

[fixtures.json](fixtures.json) contains seven short candidate windows with full source path, **compressed input SHA-256**, connection, server/receipt ranges and JSONL line ranges. They exercise motion silence and staggered reports, prompt anchors/prey, a 3-ms post-minimap frame and 31-ms motion gap, paired global phases, prey flight/long-gap cases, death-food ordering, and isolated steering reflection. They are candidates, not installed fixtures. Replay from the relevant setup or a causal prefix; do not initialize a selected window from future state. The complete maximum 2,694-ms prey-gap witness is additionally recorded in [order-and-witness.json](order-and-witness.json).

## Concrete follow-up capture plan — not executed

All attempts below target only an owner-selected official-listed endpoint, use **at most two concurrent connections**, preserve failed attempts, and record raw i/o bytes, monotonic receipt/send clocks, per-connection setup/close, ping send/RTT, collector timing, capture-batch ID, source hashes and intact gzip/ending. Run scenarios sequentially. Freeze selection and model predictions before acquisition; separate acquisition from held-out validation. These are proposals for future owner-authorized collection, not authorization to collect.

1. **Motion gate and input phase, one client, 180 s.** After settling straight with no boost, send known angle changes at randomized 10–80-ms intervals in blocks, then hold for ≥1 s; include long quiet intervals and repeated identical commands. Send legal pings at randomized phases rather than a fixed 250-ms rhythm. Compare fixed-six-tick, elapsed-40-ms-throttle and per-field clocks on held-out blocks. Record all superseded commands; do not use receipt-matched selection alone. Add 12 separated boost-on/off transitions with constant size where feasible. Aim: identify report gate/rearm, quantify skipped steering states, narrow clock bounds. Still cannot directly observe exact internal application tick.
2. **Visibility/sector boundary, two known clients, 300 s.** Establish a safe paired track with target identity pinned, repeatedly move observer near→far→near while target cruises straight. Randomize crossing phase relative to f/w maintenance, with axis and diagonal legs and a control with both stationary relative to each other. Record each client's own full state, target packets, F/w membership and failed/censored legs. Compare paired target ground track to observer entry/removal notifications; predeclare geometric alternatives. Aim: distinguish per-loop introduction, sector-boundary checks and maintenance-only cleanup, without treating unknown bodies/foods as globally absent.
3. **Mass/food event ordering, one client, 180 s.** Straight settled cruise, sparse isolated pickups, then three held boost/coast blocks; retain size/anchor/fam state around each. Select pickups from actual credited `<`/C and retain pre-event food coordinates. Compare next-loop versus 100-ms h-gate predictions and event ordering. Do not intentionally collide with unrelated players; observed deaths remain opportunistic evidence. Aim: recover mass dirty/rearm rules and separate boost drop generation from f maintenance.
4. **Prey and reconnect phase, two clients, 300 s.** Observe the same prey from overlapping view, induce bounded near/far approaches without eating it where possible, and reconnect one client twice while the other remains. Preserve absent/censored prey episodes and all y/j packets. Test global per-prey 333/210-ms clocks versus recipient-relative clocks, and compare global food/map/board phase before/after reconnect. Aim: explain long j omissions and pulse rearm. R/modern real-score cadence needs a separate explicitly eligible official capture; this corpus has no such event.

No packet-only plan can prove a hidden exact physics step or authoritative input-application tick without additional server-provided instrumentation. The useful goal for these follow-ups is to discriminate observable scheduling models and bound the unobserved delays.

## Retained evidence and access

This is a selected research record, not a complete evidence-bundle-v1 archive
or a new validated fixture set. The four linked JSON files total
22,026 bytes after LF normalization, below the
[bundle metadata bound](../../../docs/evidence-format.md#verification-and-bounds).
They are normalized from CRLF to LF with JSON values unchanged; original
absolute paths inside them are historical provenance. `fixtures.json` contains
candidates only. Source pins describe the analysis inputs, including the
pre-change comparison source, not the current tree.

All other bare artifact names in this report refer to **local-only** files under
`C:\Users\Nikita\.t3\scratch\scheduler-study-20261025`. Paths plus SHA-256s
below identify the retained originals; they do not make them downloadable.
The original report and run notes retain the failed serialization attempt,
warnings and reproduction details. Reproduction success is reported from those
records, not independently repeated by this documentation change.

| Local-only path below root | SHA-256 |
| --- | --- |
| `REPORT.md` | `e85bd30ccc3c0588b34083db9c093632e8414faad818b5715fe4a1a9ec21ba2a` |
| `RUN_NOTES.md` | `9568423c41c93f05fb20a566cabb70dcb8536ef9c20dc5e2b18b5ff6d41cd615` |
| `analyze.py` | `01862c7f08aab0fdbe6e583f999f614ce620c2d7b7364b00b73df13ea303fb5e` |
| `anchor-phase.json` | `d45ca0fc7ce346263ec0f21904fffe242c2d17fb567ca8efddf62d12b0e8e193` |
| `anchor_phase.py` | `9cb5a8f500c3daadabde7868f907924c2a4eb12110167b16654ed37f96a391f8` |
| `artifact-manifest.json` | `108578dc4a49ac8eeed3f0b361e7a2d934759d280deb220bfbaae7dbcdb81152` |
| `browser.json` | `b5a06f4dbcb21bab74c5a0f4583da3a7379cd556c38fe3059c1219412f431400` |
| `browser_scan.py` | `cdbfdc54c2e7278067e1010d9c651cd80b12f40cdc1219f7aa1dfc1774b62bff` |
| `census.json` | `eca97b270f82e0e8aa58f638e40b57e301170ee55fe14270aa4dd5bd058d595c` |
| `deep.json` | `b661d4c8effcee5cee201bdd1e0c835880f1789301af4e7ebe9f798252f1a7c8` |
| `deep.py` | `cfc4fe65edcf2eec4c6eb2aaa134696d9bc0b51d3bac1a21609948c8faa70c50` |
| `extract.py` | `bef63a21a66a082bc670ee85c6d0292d453b1cddee4b70422a9482747f79b0f2` |
| `finalize.py` | `8a567374194f87bd2dd2033ca2a95db3ffbce44f2801d9157238925aabd1d5bb` |
| `fixtures.json` | `64e0736528af152522b2ed5dd2ea795e9dd22d0bf8e52c9d68d8fb282ed554db` |
| `input-matches.jsonl` | `9f9fcfc6f5352da674ef7889dc7e63cd4e75b049e0ed4d7abade0adcaa581234` |
| `inputs.json` | `6e1cac8396b0ebf01b6872b9b8bc9568f8ea5ba3f0b187fc070474d6f8ee1653` |
| `inputs.py` | `ce5bf702e50df76a321dd6703d9eb50220fdcdafa8f96a89d397e34a39645d67` |
| `inventory.json` | `454cfecfeb825df2db48164eb39971b4fbf4a7b63c9af705040a5e439a080a0b` |
| `inventory.py` | `deb1a041b49e98c1cd885af80f2c061a15574d18f84f697e6a866e5eb27f9586` |
| `key-results.json` | `3454c53f28aa8feb811225699695cb2d3fb0488cc20197807184b5fe88e34ed5` |
| `manifest.json` | `4bb7b08aca6fe08628e49b887fba0c1ec69508e66b678c0e9683efafc860276a` |
| `order-and-witness.json` | `ac01760eeda82a7865796f33c2e117f0d148a197b16efc8407d486abeb964e73` |
| `order_and_witness.py` | `fd4424946bc9431c8fa9360f2c202b235f547470a2dc548be8faeeffcc8ce8cf` |
| `relations.json` | `be02930ccf907db9dee6acff5368303ef03a83d6b67a9128713ebf23895671a1` |
| `relations.py` | `9f9205cc1c05b82a6a4481fd4e02b1edf6e0d8cc270aab76ad874ea99cca6064` |
| `reproduction-baseline.json` | `06104617398ff9b8b0a09cc3d73a0d3d64aeac0e1841c393feb11b4633e50510` |
| `reproduction-canonical-baseline.json` | `059e2a66684f860243d0062c204cfbfd674fcd3199e94b99cfff72b65a084f88` |
| `reproduction-check.json` | `7cb82f376c404b7c666e9ab2f105300464a72e3d31f812d8e7cb157d12e7ad91` |
| `run_all.py` | `f21ebdbaeaacec6c769a129f10ebbb9ba1df67416f41d4adc4851267cab5c9a4` |
| `slither_protocol.py` | `3f630502723afb19b7c2ba3e4d509084e3e793217e08e5ba53fa41d56126b3d9` |
| `source-pins.json` | `e699a715c8dcbd8767508e6ae31ba97e88266ffa211499bdc58fb020542ad6c3` |
| `supplement.json` | `a5873a5ce40ca5bdfa8a50946d891be96ef278d4634b7c7060e7572ea1ed0030` |
| `supplement.py` | `db03ee7295e92b1fff4cbd076762ce34f65969e8d9ebe923adc61a0cb53ce0a0` |
| `verification.json` | `065ae6865321c751f261855623763252b96180a764641241d16579e2aeb5291e` |
| `verify.py` | `b800ec73c5a134dc87f8cdc7435e68e5e2c285c205ca37d50dba3bbf7dca7ff2` |

The **local-only** directory
`C:\Users\Nikita\.t3\scratch\scheduler-study-20261025\cache` contains
91 trusted derived pickle files. Its tree SHA-256 is
`14cad9b84d436a598a5f15c3b2345ae05618d9b3614af900eb822fd0252b466d`: sort POSIX relative file names, concatenate each
`<file SHA-256><two spaces><relative name><LF>` as UTF-8, then hash those bytes.
The local-only `manifest.json` above maps captures to cache names and records
raw compressed capture paths/hashes. `browser.json` inventories supplemental
raw chunks; selected candidate raw paths/hashes are also in the bundled JSON.

Raw captures, caches and official client assets are not redistributed here.
No permission to redistribute those external materials is established by this
record; access remains local-only. Hashes establish byte identity, not acquisition
truth, server identity or scientific validity.
