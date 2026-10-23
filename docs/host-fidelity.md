# Coverage, spawning and minimap evidence

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
| Minimap | 136×136 global living centerlines, full `M` snapshots, nominal 2.5 s deadlines serviced every physics tick. Stop maps after death; reset on reconnect. | Resolution and cadence are measured. Center/radius mapping agrees with the stock marker. Exact raster filtering and official full/delta selection are unknown. |

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

Radius, ecology and prey population remain configurable models. The provisional
`Config::realistic(n)` population law is not promoted to host truth by this audit.
Full raw recordings and local audit outputs stay outside the source package.

Prey position corrections are emitted every gameplay broadcast, including when
the quantized server position is unchanged. The stock client keeps predicting
from advertised speed; suppressing unchanged reports let rendered prey drift
away from their collectible server positions and snap back on a later update.

Regression checks: `cargo test --locked` covers sector retention, source-pinned
entry witnesses, actual eater attribution, whole-body coverage, safe admission,
minimap geometry, independent timing, death and reconnect.
