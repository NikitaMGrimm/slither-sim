# Frozen plan: exact head edge-block rejection

Profile the post-#272 exhausted-search fixture first, on portable release
settings. Baseline source is `3c55991d` until #272 lands; its subsequent merge
must retain identical runtime source. Sample only after the 300-tick warmup,
with CPU-clock user stacks and no candidate work in that profile. Retain the
profile and its sampling limitations separately from timing.

If head edge scans dominate, cache consecutive eight-edge block bounds once
per obstacle/Safety. A block may skip exact edges only when a conservative
guard proves them beyond the inclusive head width. Ambiguous arithmetic,
non-finite data, degenerate edges and boundary cases keep exact edge tests.
Keep candidate poses, RNG, ordering, admission, refusal and delays unchanged.
If another exact target is larger, document that measured choice before editing.

Extend the independent raw-geometry scalar oracle with multi-block and
adversarial cases. Require wrong-skip mutation failure and exact restoration,
all 23 catalog/six host/two queued traces, protocol-13/19 wire controls, and
the complete Rust check group. No native work on the local machine or SSH guest.

Use default portable release settings in both arms, no target-cpu override.
Count one discarded warmup and five AB/BA/AB/BA/AB pairs per protocol on
the exhausted row and 80/160 synthetic controls from #272. Preserve full-host
fork and direct-parent parity. Hardware instructions require at least 99%
scheduled time per tick; report CPU p50/p99/max and the slowest 1% separately.
Measure block/Safety construction separately so its cost is visible. Include
construction in complete search/tick timing; separate diagnostic clocks must
not perturb selected production intervals.

One Frankfurt Spot builder through the limited profile, creation tags on all
resources, terminate shutdown behavior, DeleteOnTermination and in-guest
planned-duration plus 30-minute poweroff. Combined cap $1, including failures
and noncompute allowance. Retrieve only completed/hash-verified archives,
preserve sources until verified retrieval, then clean task resources and scan
all enabled regions. Keep identifiers and raw archives private. Reuse earlier
harnesses by reference and publish bounded results, failed attempts and lean
archive/manifest pins. Open one template-compliant PR; never merge. If exhausted
CPU does not improve, publish the rejected attempt and its measured numbers.
