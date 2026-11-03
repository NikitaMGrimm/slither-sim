# Local validation checkpoint (not a published adoption study)

The bounded worker implementation is committed on perf/client-pellet-workers. This checkpoint preserves ongoing evidence preparation while #450 review finishes. It is not a ready PR or a passing study-check claim: REPORT.md still needs to be composed from recompute.py, then recompute/test validation and integration onto main are pending.

All frozen binaries, source snapshots, raw callback/load journals and restricted replay input remain outside Git in the lane's D: performance scratch. No raw recording or identifying layout is included here. Preserve the unrelated untracked cold-sprite-masks folder.

Completed implementation-run controls: full npm run check (before subsequent comment/test-only edits); all supported participant-count, blocked-claim, 20 ms delay, error-order, panic and sparse tests; exact serial/pool presenter images across Original/Native, prefix/overlay, resize, world replacement and renderer/device teardown; 9,600 complete replay/rectangle/fallback/reconnect image matches.

Retained adoption data: five RTX/Vulkan 4K triplets per quality and five AMD/DX12 triplets per size/quality, all attempts including compiler overlaps; delayed-worker and eight-CPU-load pairs; paced menu/sparse/post-dense process-CPU controls and explicit worker thread counters. The clean sealed cutoff control is worker-density-final-r3, zero observed compiler/link overlap. Early thread-counter observations are marked invalid because the probe originally accepted only zero HRESULT rather than every successful HRESULT.

RTX Original art median/p95, serial split to pool: 5.203/7.980 -> 3.867/7.281 ms. Native: 4.990/7.870 -> 3.711/7.011 ms. Main CPU remains about 3.3-3.5 ms and p95 misses the 4.167 ms target. AMD Native 4K median improves 15.150 -> 14.177 ms, but p95 is 17.360 -> 17.999 ms; disclose this tail regression. Source rebuilds were instrument-confirmed on 65 of 300 art callbacks, with about 1.8 ms median in the slowest 16. No incremental source cache has been written.

Do not claim canonical parity from timing rows. Their preserved inactive accumulator is metadata only. The worker adoption study's checker now uses sent-byte and actual-image evidence. #450 separately retains a genuine explicitly enabled canonical control. New private driver generation prints absent/empty for inactive/empty canonical output; its source-only patch and authored helper test are retained in #451.

The per-spawn immutable source allocation introduced by #450 (280-byte payload plus Arc ownership allocation) and worker temporary allocations are included in the process-wide Rust allocator counters. Process totals include renderer teardown; vendor/OS allocations outside Rust's global allocator are outside that accounting. Required final report must state this, idle sampling limits, unverified four-core hardware/browser presentation and the earlier measured base versus later benchmark-only main changes.
