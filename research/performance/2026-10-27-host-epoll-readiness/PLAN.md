# Persistent readiness native screen

Keep only a small two-file Linux readiness change with unchanged service,
delivery, flush and wait timing. Per-open registration avoids fd-reuse aliasing;
compaction updates the event's current index. Keep poll as failure fallback.
Test against the old poll snapshot after writes, partial drains, reindexing,
hangup, descriptor replacement and registration failures.

Baseline origin/main at experiment freeze, candidate only the readiness change.
Freeze source and compiled binary hashes before holds. Use an unchanged copy of
the native run.sh under the authorized capacity directory and the same shared
lock. A private bench copy changes only its owned paths, port to 18934, shard
count to one and load affinity to CPU1. Host CPU0; no other heavy work during
holds. Same frozen generator and telemetry/validity gates for both arms.

After a short smoke per arm, three 120-second official-500 pairs after 15-second
warmup, order AB, BA, AB. No performance-based exclusions. Each pair must reduce
total host CPU milliseconds per executed tick, with valid traffic/affinity and
generator headroom. If not consistent, reject without more searching. Retain
all failed attempts and diagnose harness failures before any recovery. Do not
change security/system settings, install packages, use forbidden ports, merge,
or contact outside game servers. Record frame-excess and tick tails as controls,
not a capacity qualification. No instabox CLI; SSH only.
