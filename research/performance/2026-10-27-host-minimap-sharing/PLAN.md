# Minimap burst protocol, 2026-10-27

Baseline: current main `ac85528`. Before implementation, measure official-500
and Battledome-64/80 real loopback sockets, no background bots. Keep the
existing player steering/boost and spawn-wait reporting harness. Preserve
admission and arena rules. Do not use ports 8766/8780 or scratch/play.

A separate diagnostic overlay records each simulation-thread tick, including
command intake/admission, physics, broadcast and handoff. Thread-local Linux
instructions:u and thread CPU clocks bracket ticks and each minimap call;
wall clocks report observed worst ticks. Buffer rows until shutdown. Classify
by the actual minimap clock, not receipt timing. Report map-due versus other
ticks, minimap share, worst CPU/wall tick, living/waiting populations and waits.
All host threads share CPU 0; three load shards use CPUs 2/4/6. Jobs/Rayon/tests
one. Shared-machine wall/CPU results are descriptive and contaminated; make
no capacity claim. All raw data stays in persistent storage outside /tmp.

After 15 seconds warmup, hold 45 seconds per workload. Retain all attempts.
Instrumentation overhead remains in diagnostic numbers; whole-process
unmodified native-fixture instruction pairs are the primary optimization
evidence. If official-500's measured minimap burst is well below 1 ms, stop
without implementation and recommend indexed gateway delivery instead.

If warranted, share map encoding/validation by immutable snapshot and matching
recipient baseline, retaining lazy errors, shortest-packet ties, recipient
state, wire bytes/order and reconnect/skipped-map recovery. Keep protocol-specific
packet storage distinct from the shared raster; do not implement protocol 13.
Use the same diagnostic overlay on both arms for the burst comparison.

Before counted pairs, pass focused map tests and the CONTRIBUTING Rust group;
compare complete decompressed portable/protocol, official-500, Battledome
40/64/80 and retained reconnect/decoder-replay traces with the existing shared
overlays. Add a longer map-inclusive window if existing windows omit due maps.
One untallied pair then five alternating pairs, >=99% scheduled counters;
report every measured ratio and range, matching populations/lifecycle. Keep
the measured runtime pin separate from the eventual documentation/PR head.

Publish selected summaries and rerun scripts only, targeting <=20 KB; retain
raw rows, binaries, traces, failed attempts and logs locally. Open one PR; do
not merge. No broad area redesign or long-lived branch.
