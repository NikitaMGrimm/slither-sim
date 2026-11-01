# GPU-input and forced-off diagnostic results (2026-11-01)

**Candidate A/B remains blocked; no new candidate timing claim.** Actual trace
hashing found resource-history differences in 56 of 72 unaffected candidate
controls. Upload byte lengths and SHA-256 sequences match in all 72. The
forced-off binary matches stock in all 96 full-matrix validation pictures and
all 16 pictures of the separate timed control workload. Its 20 alternating and
reversed timing runs do not reproduce a persistent approximately 1 ms increase.

## Exact sources and artifacts

Every table and CSV below uses stock baseline commit `d1f4b031d3f9c863944791023063af28dd51b97f`,
frame executable SHA-256 `998f0ef0ea72e886ecbc9cdc270eb7fbc59fccaba1c12282006ed35e7a0020b9`.
Candidate and forced-off use commit `804f20c39442d1eccb8eea2074c2dc2e3e58c04b`; the enabled
cut executable is `5e57734ea0cd2754729b3290c2be6915bf6718fced8b6f8ac5401ea873a8439e`, and forced-off
is `5eaaace4f6a540a3b1fe0cc58b145e32314843d94a2cb2dcd3e5dc39aecdc754`. Features are respectively
`frame-profile,feel-trace` and `frame-profile-no-shadow-cut,feel-trace`.

[input-provenance.json](input-provenance.json) pins the exact converter, offline
hasher, source trees, raw-result hashes and all three builds. Each row of
[gpu-input-controls.csv](gpu-input-controls.csv),
[ac07-per-run.csv](ac07-per-run.csv) and [ac07-controls.csv](ac07-controls.csv)
repeats its exact source/binary pair. This #412-based workload uses the real
Original/Native projection/presentation boundary at DPR 1. It is separate from
the historical direct-offscreen retained-02 and A/A/B/A data.

## Why typed hashing initially failed

The offline hasher treated a public-handle drop as the end of a GPU resource's
lifetime. The real blitter trace contains:

```text
CreateBindGroup(blitter_group, ...)
DropBindGroup(blitter_group)
Submit(... SetBindGroup(blitter_group) ...)
```

The encoder keeps the core resource alive after its temporary public handle
drops. The old script therefore reported an uncreated GPU resource reference.
It now retains retired descriptors for queued commands and assigns a fresh
identity to each creation when a pointer address is reused. Six Node regressions
pass, including both lifetime cases. The same retained traces then hash
successfully; no Rust rebuild was needed. Nothing was accepted through a failed
hash, and no candidate timing ran before or after the mismatches.

## First resource differences

These comparisons hash actual wgpu API writes and commands below the Draw layer,
including the blitter. Every actual buffer/texture upload payload in an observed
control is hashed; live descriptors include capacities/usages and retained CPU
inputs. Immutable shader/texture dependencies survive public-handle retirement.
Opaque identities and capture paths are normalized, not byte contents or sizes.
Each validation cell samples one complete picture; these are not timing runs.

- Candidate versus stock: 72 unaffected pictures, 72 identical upload-byte
  sequences, 56 differing broader fingerprints. Components overlap: 52 live
  inventory differences, eight API creation/order differences, four bound-buffer
  descriptor differences. Default Draw snapshots are exact.
- The first sequence difference is the 120-model/4K Original default view after
  an affected 1080p whole-map view: stock retains two additional vertex chunks.
  Current uploads and pass commands match; the retained inventory does not.
- In each 120-model/4K Native whole-map control, the identical 392,520-byte vertex
  upload (SHA-256 `9e1be82def72ba2c42069b1d88bc6ed942e60674747048cd9a472861de1b7251`)
  targets a **983,040-byte** stock buffer and a **524,288-byte** candidate buffer.
  Both use `COPY_DST | VERTEX`. This occurs for both requested sample counts on
  both adapters; Native 4K is effective 1x under #412.
  The stock buffer was grown during the earlier 500-model/1080p Original
  whole-map frame; the candidate buffer was created during 120-model/4K
  Original whole-map. The retained raw allocation calls establish both origins.
- Source `gpu.rs` grows a chunk only when its current capacity is too small, and
  keeps unused chunks. Earlier omitted shadows therefore change later capacity,
  allocation calls and retained contents even when that later Draw list matches.
- Forced-off versus stock: all 96 complete fingerprints match. This confirms
  the connection between the cut and the resource-history differences. It does
  not retroactively establish the precise cause of the old direct-offscreen gap.

## Matched stock/forced-off timing

Pair `rebased-07-stock-forced-off`, with the exact commits and executable hashes
above, covers the 500-model default controls at 1080p/4K, Original/Native, and
requested 1x/4x MSAA on both adapters. A/C means stock first; C/A reverses order.
There are five pairs per order, 30 warmed measured pictures per side/cell/run.
The complete CPU phase, allocation, vertex and draw-call values are in the
per-run CSV; GPU world/blit/overlay deltas are in the control CSV.

| GPU | Physical | Mode | Requested/effective MSAA | A/C paired delta ms [min, max] | C/A paired delta ms [min, max] |
| --- | --- | --- | --- | --- | --- |
| AMD integrated | 1920x1080 | Native | 1x/1x | -0.039 [-0.078, +0.096] | -0.048 [-0.078, -0.005] |
| AMD integrated | 1920x1080 | Native | 4x/4x | +0.008 [-0.057, +0.046] | -0.029 [-0.214, +0.154] |
| AMD integrated | 1920x1080 | Original | 1x/1x | -0.063 [-0.137, +0.017] | +0.045 [-0.059, +0.074] |
| AMD integrated | 1920x1080 | Original | 4x/4x | -0.000 [-0.092, +0.110] | +0.142 [-0.182, +0.365] |
| AMD integrated | 3840x2160 | Native | 1x/1x | +0.050 [-0.189, +0.115] | -0.049 [-0.153, +0.342] |
| AMD integrated | 3840x2160 | Native | 4x/1x | -0.050 [-0.146, +0.107] | +0.065 [-0.013, +0.258] |
| AMD integrated | 3840x2160 | Original | 1x/1x | -0.132 [-0.154, +0.037] | +0.191 [+0.083, +0.332] |
| AMD integrated | 3840x2160 | Original | 4x/4x | +0.052 [-0.066, +0.286] | -0.128 [-0.297, +0.059] |
| RTX 5070 Ti | 1920x1080 | Native | 1x/1x | +0.000 [-0.001, +0.001] | -0.001 [-0.002, -0.000] |
| RTX 5070 Ti | 1920x1080 | Native | 4x/4x | -0.001 [-0.009, +0.003] | -0.000 [-0.001, +0.001] |
| RTX 5070 Ti | 1920x1080 | Original | 1x/1x | -0.000 [-0.001, +0.001] | -0.001 [-0.003, +0.003] |
| RTX 5070 Ti | 1920x1080 | Original | 4x/4x | -0.000 [-0.005, +0.003] | +0.000 [-0.001, +0.005] |
| RTX 5070 Ti | 3840x2160 | Native | 1x/1x | -0.000 [-0.001, +0.001] | +0.000 [-0.003, +0.001] |
| RTX 5070 Ti | 3840x2160 | Native | 4x/1x | +0.000 [-0.003, +0.003] | -0.000 [-0.003, +0.000] |
| RTX 5070 Ti | 3840x2160 | Original | 1x/1x | -0.000 [-0.001, +0.002] | -0.000 [-0.004, +0.001] |
| RTX 5070 Ti | 3840x2160 | Original | 4x/4x | +0.002 [-0.003, +0.004] | -0.001 [-0.006, +0.003] |

AMD 1080p Original/4x paired total-GPU medians are -0.000110 ms in A/C order and
+0.142195 ms in C/A order; corresponding world-pass medians are -0.002870 and
+0.141770 ms. The largest AMD paired increase across all cases/orders is
+0.364825 ms, with a minimum of -0.296885 ms. There is no repeated approximately
+1 ms code-only effect in these rebased controls. Smaller order-dependent
differences remain visible in the complete table; they are not discarded.

All 20 timing runs completed 2026-11-01T10:34:35.884282+01:00 to 2026-11-01T10:40:12.963714+01:00, with no
compiler overlap and 80 exact default Draw pairs. Median system CPU load per
run spans 9.9 to 48.15 percent;
the high-load run remains included. Non-admin AMD clock samples and PLAN
timestamps are retained, but do not establish a clock for every GPU pass.
Hashing, trace archival, readback/waits and analysis are outside reported CPU
timing; reported GPU time is the sum of the named render-pass intervals.

## Limits and next comparison

The old A/A/reversed-B/A result still rules out its simple order/noise explanation.
RTX sparse-pixel nonrepeatability remains unresolved. This new study neither
relabels the old binaries nor claims zero GPU regressions in every history.
The initial baseline trace attempt reached 1,179.55 MiB of scratch before closed
RON archival was added. All raw bytes and failed attempts were retained in
verified lossless archives; subsequent device traces archive as they close.

The booked 12:00-12:40 comparison will explicitly condition both VBO pools with
the same authored geometry sequence before each cell. One extra candidate A/B
pair will retain production history. Both results will be reported before any
decision to change production cache sizing. No production cache change is made
by these diagnostics. Final candidate timing/crop, the end-to-end native
threshold assertion and full npm validation remain pending after the reserved
build windows. The AMD merge blocker remains open.
