# Host fidelity evidence

The October 23 audit traced the historical research into the standalone host.
The old 500-unit food/snake/prey cutoff and 32-cell minimap were unsuitable
defaults. Several online policies remain unidentified; there is no exact
official preset determined solely by bot count.

| Area | Current behavior | Evidence and limit |
| --- | --- | --- |
| Food | 45 sectors: 7×7 without four corners, 300 units per sector, center led 100 units along physical heading. Retain last-touched sectors for 2.6 s, clean on nominal 504 ms opportunities. | The frozen small-snake classifier reproduced historical receipts and a fresh capture. The combined operational scheduler and large-snake scaling are provisional. |
| Snakes | Complete centerline intersects `world.vision`, default 1,500, for native and browser clients. | Paired receipts reject 500 and whole-world streaming. A 1,500-unit circle reduces missed presence, but is an engineering compromise; entry/exit geometry, retention and size scaling remain unresolved. |
| Prey | Same configurable distance, independently checked as points; actual pickups include eater IDs. | The 500 cutoff also misses received prey. A shared numeric default does not establish a shared official subscription algorithm. |
| Hosted spawning | Radial-lattice candidates; complete newborn corridor must clear living heads and body edges by 600 units. No cramped fallback. | Own joins support a large exclusion zone, with exceptions below 600. This is conservative admission, not a recovered universal rule or invulnerability. Native initial test rings remain synthetic. |
| Minimap | 136×136 global living centerlines, full `M` or XOR `V`, nominal 2.5 s deadlines serviced every physics tick. Stop maps after death; reset on reconnect. | Resolution and cadence are measured. Center/radius mapping agrees with the stock marker. Exact raster filtering and official full/delta selection are unknown. |

## Rechecked evidence

Historical source: `slither-arena-rs` commit
`c2fb7285febb4273034e30699dc880715a334423`, especially
`docs/research/spatial-cells-and-visibility.md`, the October 3 presentation report,
and the retained sector-entry and own-join analyses. APK offline culling values
do not identify online subscriptions.

The fresh six-minute protocol-19 archive has SHA-256
`dd65ff79bf5cb258cf0d72c35af22d07e9c6ff06068b6679e1ec011bafcb8a60`.
Its complete gzip, capture-end integrity, frozen plan and source pins were checked.
All 376 clean small-snake food entries were beyond the old 500-unit circle.
The unchanged temporal classifier disagreed on 21 of 3,394 correlated
known-sector exposures; these are not independent trials or whole-host accuracy.
The [entry fixture](../tests/fixtures/client-food-interest.json) retains selected,
source-pinned regression witnesses and their selection limits.

Two independent decoders agreed on all 134 received maps (122 `M`, 12 `V`), all
136 cells per side. Within-connection intervals were 2,497–2,510 ms. Servicing
nominal 2.5-second deadlines every physics tick avoids the old recurring
2,520 ms broadcast rounding; it does not install 2,504 ms as a server constant.

The paired snake/prey comparison used hash-verified strict-crossing archives.
Its retrospective holdout lacked enough aligned transitions to validate a
threshold or timer. A subsequent bounded two-client attempt failed before either
client joined; both interrupted archives remain retained and establish no new
visibility result. Exact subscription recovery still needs successful paired
crossings with present/absent intervals and held-out legs.

The next frozen cardinal/diagonal plan stopped at its one-client preflight:
neither the retained endpoint nor a disclosed current-list endpoint opened a
WebSocket. Both attempts retained complete capture files with zero frames and
zero joins. The dependent crossings were not run; the new route tooling supplies
no additional visibility validation.

A later explicit-Origin preflight joined, followed by two completed eight-minute
paired acquisitions. The [paired recheck](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-visibility-recheck/REPORT.md)
preserves their hashes, food-model counterexamples and zero eligible controlled
snake/prey crossings. It identifies collector endpoint-accounting defects, but
does not supply a validated replacement streaming rule. Food entry/retention and
the combined cleanup schedule remain approximations, including for small snakes.

The separately frozen corrected-collector follow-up achieved one diagonal route
leg, but none of its comparison points passed all original freshness/alignment
gates. Cardinal acquisition failed and was quarantined; the planned extensions
were not run. Additional food counterexamples remain preserved. The same
[recheck note](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-visibility-recheck/REPORT.md) records the missing denominators
and the independent all-map minimap recheck. No numerical subscription rule was
promoted from these results.

The later [managed paired follow-up (R6)](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-visibility-recheck/REPORT.md#managed-paired-follow-up-r6)
completed both planned acquisitions and their frozen analysis, but neither pair
completed an endpoint-to-endpoint leg. All calibration, held-out, query and
transition denominators are zero. Saved control telemetry shows route divergence
and deaths after initial arrivals; it supplies no subscription correction.

The later [exploratory R7 pilots](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-visibility-recheck/REPORT.md#exploratory-route-pilots-r7)
preserve a direct-snake replay rejection and an avoidance-aborted food route
with zero qualified entries. A separately amended retrospective replay produced
702 qualified received-cache comparisons from one batch, while the original
route still had zero endpoints. This supplies conditional observations, with
no validated replacement visibility rule or numerical policy change.

Radius, ecology and prey population remain configurable models. The provisional
population law now used by `Config::official(n)` is not promoted to host truth by
this audit.
Full raw recordings and local audit outputs stay outside the source package.

## Food sector delivery

The complete six-minute archive above contains 1,513 nonempty `F` listings, no
empty `F` and no `W` sector announcements. The host tracks empty subscriptions
internally, introduces nonempty sectors with `F`, and sends subsequent births and
pickups as deltas. The first birth in an already empty subscription uses a birth
delta; that choice is not an identified server rule. `w` clears retained recipient
food on exit; it does not delete physical food. The selected
[food fixture](../tests/fixtures/client-food-interest.json) pins the full-archive
announcement census as well as entry witnesses. Decoder support for other valid
packet forms does not require the host to emit them.

## Minimap encoding

Each connection retains its own last-sent bitmap. First/reconnected/resized maps
use full `M`; later maps may use the shorter of full `M` and XOR `V`. The base
advances only after queuing that recipient's packet, preserving ordered delivery.
This is a local compression policy: on the retained sequence the older bridge's
shortest-encoding policy disagreed with 10 of 129 noninitial packet choices.
Both encodings reconstruct the intended bitmap; the closed server's selection
rule and raster filtering remain unknown.

## Ecology models

The default `Config::official(n)` ecology is provisional: its population-scaled
radius, food target/refill, radial food profile and working prey density; see
[official and sandbox](configuration.md#official-and-sandbox). Received arrivals
and occupancy cannot identify rejected generation attempts, global counts or
startup seeding. The earlier ambient replacement association did not survive a
later matched-sector check, so it supplies no refill retuning.

The optional radius controller is known to differ: hash-verified
`run4-radius-long-20260927/bot0.jsonl.gz` (SHA-256
`3180b8201f51d5f4fe9732b6c69e85bab8ce49448742a2a72cfd228ef60613ef`)
contains expansions faster than its local rate limit. In connection 4, source
server time 292,116→292,243 ms, radius grows 13,940→13,947. The target/controller
replacement remains unidentified; use recorded `radiusUpdates` with
`dynamicRadius:false` for measured replay. The cited historical studies
`radius-dynamics.md`, `radius-load-followup.md` and `ambient-event-study.md` retain
failed controller fits and negative refill results. Do not install the fastest
observed slope as a new official cap.

## Packet timing

The same archive contains 60,019 packets across 32 command families. Counts depend
on population, movement and recipient exposure; matching total packets per second
would not establish fidelity. Within known prey lifetimes, 332 consecutive `j`
gaps have median 338 ms (211–357 ms). Historical reporting/decision studies at the
source commit above independently constrain ordinary and exact-11-speed pulse
intervals; their original reports preserve failed fits and unresolved onset.

The later paired recheck retains the roughly 338 ms ordinary median, but includes
one shared 1,614 ms same-prey update gap while other incoming traffic continues.
Captured extrema are corpus-specific; periodic local correction traffic does not
recover the official reporting scheduler.

Prey now receive unconditional position corrections near one third second, even
when quantized server position is unchanged. Exact-11 pulse transitions may report
after 210 ms; ordinary turning alone does not accelerate reporting. Opportunities
are serviced on gameplay broadcasts. These per-recipient clocks, including their
initialization at appearance, are approximations: the observed lower bound applies
to consecutive `j`, and an appearance can be followed by `j` sooner. The stock
client predicts continuously; removing unchanged corrections lets rendered prey
drift indefinitely. With finite reporting intervals, prediction and render smoothing
still produce bounded offsets from the quantized collectible position.

Setup advertises the active prey turn rate in reference-frame units: default
`mamu2=0.029`, matching all five captured setups and the existing
[recorded setup fixture](../tests/fixtures/client-hud-leaderboard.json).
Leaderboards use nominal three-second deadlines at physics-tick granularity,
preserving phase instead of accumulating 3,024 ms broadcast rounding. The 107
pre-death captured intervals span 2,989–3,029 ms (median 3,009); neither the median
nor the first-report phase is installed as an exact server constant.

Use the [offline packet census](research-workflow.md#inspect-analyze-and-reproduce)
to repeat family, length and known-prey cadence checks on new captures. It resets
histories at connection/appearance/removal boundaries and retains censored exposure.
The [cadence fixture](../tests/fixtures/client-reporting-cadence.json) is a selected
epoch's direct census derivative, with source/tool hashes and selection limits;
the stationary-prey regression checks its ordinary reporting bounds.

Regression checks: `cargo test --locked` covers sector retention, source-pinned
entry witnesses, actual eater attribution, whole-body coverage, safe admission,
minimap geometry, independent timing, death and reconnect.
