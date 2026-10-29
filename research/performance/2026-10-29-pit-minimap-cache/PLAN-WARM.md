# Additional warm-cache row

Frozen after inspecting the completed 300-warm-tick run, before counting this
additional row. Preserve the original row and every pair as cold-cache evidence.
The first run costs about 12.7% more instructions on its sole map tick in both
protocols. Its ready state has `nextMinimap: 1`: the warmup ends before the
first map delivery to a viewer. A discarded fork warms CPU caches but cannot
populate the immutable parent's projection cache.

Use exactly the same staged runtime, fixture, counters, build flags and pair
runner, changing only the minimap row's warmup from 300 to 600 ticks. At 4.2
world seconds the parent has delivered the first map and initialized the cache.
Count the same 360 ticks, including the next map opportunity. Spawn and marker
warmups remain 300 ticks. Require identical complete-byte repeated-fork and
direct-parent proofs, all native catalog/host/queued parity and mutation gates,
then five alternating instruction pairs per protocol after discarded warmup.

Report cold construction and warm reuse separately, including the regression;
do not replace the original row or extrapolate either to live capacity. Record
the new staged source manifests and raw archive independently. Source-level
production code and its completed Rust checks remain unchanged.
