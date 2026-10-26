# Occupied-seat default: product decision

2026-10-26. **Synthetic product/feel model, not an identified official law.**
No new capture, outside game-server connection or empirical fit.

The owner wants a match to contract after players leave. The earlier
[radius study](REPORT.md) remains historical: its failure to predict official
waves, counterexamples and retained living-count jitter experiment are unchanged.
The old living-count controller is removed, so its historical driver must use
the source revision pinned in that report for reproduction; enabling
`dynamic_radius` on current source selects the new seat model.

The new default uses occupied seats: living slots and physics-dead slots
waiting to respawn count; retired/dormant slots do not. Joins and leaves affect
the target `670 * sqrt(max(occupied, 1))`; deaths and respawns do not. When the
error exceeds 3% of the current radius, grow at once to the target or shrink
gradually at 23.4375 units/s. Expansion gives snakes room immediately, so joins
do not wait for boundary motion. This is a product choice, not an official fit;
the retained report's +55.118 units/s expansion already exceeds the former cap.
There is no hysteresis, moving average or controller history. A final offset
inside the dead zone is intentional. Official radius samples still override
the controller without a rate cap; fixed recorded replay is unchanged.

Hosts start sized for configured bots, with a one-seat minimum, rather than
empty reserved player capacity. Players contribute after admission. Native
backends and reset worlds start sized for their initial agents. Startup sizing
still only enlarges a supplied config. Sandbox, Battledome and explicit JSON
radii remain fixed unless their flags explicitly opt in. A Rust caller choosing
a fixed radius disables both sizing and dynamics. A synthetic raw-host test
starts with four bots in radius 1340, admits 30 player joins in one burst without
retrying, then grows to `670 * sqrt(34)` on the next tick. Three seeds verify all
34 clients reach playing state and decode the new radius. Bounded spawn admission
remains a local policy, not evidence for an official capacity rule.

Ecology uses the resolved initial config as its density reference. Food target
(floored), ambient attempt rate and custom prey target (rounded) scale with live
area. Official prey keeps its unrounded density formula and one-prey minimum;
zero targets remain zero. Refill earns initial-area seconds weighted by current
area, preserving fractional attempt credit without new checkpoint state.
Surplus prey are removed in stable insertion order. Previously, food outside
a shrinking wall stayed in storage and counted toward refill; the seat model
removes it from all origins on shrink, so stranded pellets cannot starve the
smaller arena. Food inside remains; actual counts need not equal targets.
Fixed replay preserves historical food retention and fixed ecology.

A second synthetic raw-host regression decodes two independent client views.
A recorded radius step isolates shrink: three outside pellets (ambient, boost,
death) and six surplus prey disappear without pickups. Both connections receive
the ordinary food/prey removal packets with no eater; their decoded views retain
only the one inside pellet and two surviving prey. The food sectors stay
subscribed, so this tests physical deletion rather than interest exit. It checks
protocol delivery and reconstruction, not a browser/renderer trial.

The synthetic [fixture](../../../tests/fixtures/arena-seat-following-v2.json)
uses an independent scalar calculation: from 500 to 100 occupied seats,
`R0 = 670*sqrt(500) = 14981.655449248592`, target `6700`, stop threshold
`6700/0.97`. At `dt = .007`, each moving tick subtracts `.1640625`.
`ceil((R0 - 6700/.97)/.1640625) = 49216` ticks, **344.512 seconds**,
ending at radius **6907.155449248592**. The next tick is stationary. This is
a deterministic model calculation, not a measured shrink time or feel trial.

Checkpoint format stays v2; rules revision becomes `slither-sim-rules-v2`.
Rules-v1 checkpoints reject instead of silently changing their continuation.
No config field, JSON shape or ABI layout changes are needed for this model.
