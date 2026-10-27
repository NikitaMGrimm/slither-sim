# Pre-qualification startup correction

The first v0.7.0 eight-client smoke was rejected before any qualification hold.
Its raw summary remains retained: four connection-refused startup errors, all
eight clients playing during the smoke hold, and one affinity violation from
observing the pre-exec `taskset` process before it set the host mask.

The wrapper now checks that the loopback port is unused, starts the host worker,
waits for its listening socket, then starts the load worker before the common
future epoch. Affinity validation covers every sampled thread from that epoch
(including warmup) through shutdown, excluding process-launch samples before
the workload starts. A regression verifies this boundary while retaining mask
and observed-CPU failures inside it. The host still inherits CPU 0 before its
executable starts; no Rust instrumentation or binary changed.

Restart the three-smoke schedule, then execute the original nine qualification
holds once. Keep the original rejected smoke, its summary and its original
wrapper digest. This is an infrastructure correction, not a speed-based repeat
or delivery-gate change.
