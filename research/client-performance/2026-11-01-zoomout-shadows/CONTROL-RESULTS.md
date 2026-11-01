# Retained-binary A/A and reversed B/A controls

**The AMD merge blocker remains open.** Identical-binary A/A does not reproduce
the roughly one-millisecond 500-model/default/1080p/4x gap. Reversing execution
order retains that gap and several other unaffected 500-model increases. These
data do not support attributing the increase to ordinary second-run order or
the observed A/A variation. They do not establish a driver or clock cause.

## Exact pair and dataset provenance

These are the old retained executables, not the rebased/fixed candidate. All
cells use **direct offscreen rendering at scale one**, bypassing Original/Native
policies. Do not relabel them or mix them into the future #412-based results.

- **A/A (`retained-02-AA`):** both baseline and second-arm candidate are source
  commit `181bc01f21441d3447cad7acefec1f683b347e29`, executable SHA-256
  `82fda4f9c33e4f56b0b23d4e81857a174856cd0202f9aef45dc9da59436559bd`.
  The same executable runs as left, then right.
- **Reversed B/A (`retained-02-BA`):** baseline source commit
  `181bc01f21441d3447cad7acefec1f683b347e29`, executable SHA-256
  `82fda4f9c33e4f56b0b23d4e81857a174856cd0202f9aef45dc9da59436559bd`;
  candidate source commit `583c502478fce0e576c82af1de5103bd21bc46ff`, executable
  SHA-256 `42fe9aa38ab6effd10abf32d83b7053bc0af03675e3aa3665bfdaabff173eb32`.
  Candidate runs first; tables report baseline → candidate.

The [manifest](control-provenance.json) maps these pairs to
[per-run CPU/GPU medians](control-per-run.csv), the
[full unaffected-control comparison](control-unaffected.csv), and
[AMD clock context](control-clock-context.csv). Every CSV row carries both
commits and binary hashes; per-run/clock rows also identify the actual arm.

## Capture and equality

Five A/A pairs followed by five reversed B/A pairs ran 08:00:00–08:20:15 local,
2026-11-01. All **20** completed with no rejected cells or cargo/rustc overlap.
There are 150 warmed measured frames per adapter/cell/arm. System CPU run medians
were 10.8–23.5%, with a one-second sample peak of 55.2%. Full receipts, clock
traces and binaries remain in scratch. This report required no new GPU run.

All **80** retained default draw snapshot pairs are byte-identical. Every frame
in every unaffected cell matches its paired draw, vertex, draw-call, scale and
prepared-snake counts. Mid/out snapshots and new PNGs were not recorded, so
their count equality is not a new byte-level draw-list or pixel comparison.

## All 18 unaffected AMD cells

This table uses `retained-02-AA` and `retained-02-BA`, with the exact source
commits/binary hashes above. It covers default/mid for both populations and
extents, plus 120-model/4K/full-map, each at 1x and 4x. Widths correspond to
1920×1080 and 3840×2160. Values are medians of five per-run GPU medians; paired
deltas are computed before their median/range. Subtracting the two displayed
medians need not equal the displayed paired-delta median.

| Model | Width px | Zoom | MSAA | A/A GPU ms (left → right) | A/A paired delta ms, median [range] | B/A GPU ms (baseline → candidate) | B/A paired delta ms, median [range] |
| ---: | ---: | --- | ---: | ---: | --- | ---: | --- |
| 120 | 1920 | default | 1 | 1.226 → 1.261 | -0.023 [-0.189, +0.323] | 1.133 → 1.234 | +0.002 [-0.480, +0.493] |
| 120 | 1920 | default | 4 | 2.421 → 2.440 | -0.007 [-0.059, +0.097] | 2.477 → 2.505 | -0.004 [-0.055, +0.132] |
| 120 | 1920 | mid | 1 | 3.772 → 3.765 | -0.022 [-0.107, +0.036] | 3.738 → 3.784 | +0.042 [-0.004, +0.114] |
| 120 | 1920 | mid | 4 | 4.784 → 4.850 | +0.066 [-0.158, +0.198] | 4.825 → 4.835 | +0.002 [-0.116, +0.121] |
| 120 | 3840 | default | 1 | 4.642 → 4.624 | -0.045 [-0.124, +0.076] | 4.639 → 4.627 | -0.004 [-0.020, -0.002] |
| 120 | 3840 | default | 4 | 14.386 → 14.314 | -0.103 [-0.368, +0.197] | 13.644 → 13.928 | +0.415 [-0.007, +1.513] |
| 120 | 3840 | mid | 1 | 13.423 → 13.355 | -0.000 [-0.276, +0.395] | 13.339 → 13.353 | +0.024 [-0.018, +0.192] |
| 120 | 3840 | mid | 4 | 26.631 → 26.667 | +0.144 [-0.275, +0.422] | 24.541 → 24.969 | +0.149 [-0.071, +2.099] |
| 120 | 3840 | out | 1 | 19.677 → 19.777 | +0.005 [-0.147, +0.330] | 19.655 → 19.963 | +0.218 [+0.127, +0.370] |
| 120 | 3840 | out | 4 | 32.977 → 32.812 | -0.106 [-0.235, +0.255] | 31.687 → 31.561 | +0.153 [-0.565, +3.857] |
| 500 | 1920 | default | 1 | 1.509 → 1.494 | -0.005 [-0.073, +0.008] | 1.468 → 1.600 | +0.133 [+0.054, +0.371] |
| 500 | 1920 | default | 4 | 2.438 → 2.402 | -0.026 [-0.048, -0.013] | 2.374 → 3.446 | +1.081 [+0.993, +1.641] |
| 500 | 1920 | mid | 1 | 2.883 → 2.908 | +0.051 [-0.196, +0.348] | 2.954 → 3.085 | +0.129 [+0.093, +0.265] |
| 500 | 1920 | mid | 4 | 3.569 → 3.740 | +0.142 [-0.138, +0.282] | 3.761 → 4.427 | +0.706 [+0.563, +1.636] |
| 500 | 3840 | default | 1 | 3.794 → 3.802 | +0.004 [-0.022, +0.042] | 3.801 → 4.657 | +0.856 [+0.817, +2.877] |
| 500 | 3840 | default | 4 | 12.877 → 12.867 | +0.013 [-0.910, +0.460] | 12.655 → 13.650 | +0.994 [+0.434, +1.084] |
| 500 | 3840 | mid | 1 | 10.190 → 10.213 | +0.023 [-0.067, +0.157] | 10.169 → 10.898 | +0.736 [+0.659, +7.652] |
| 500 | 3840 | mid | 4 | 23.265 → 22.858 | -0.333 [-3.118, +0.247] | 21.555 → 22.375 | +0.458 [-4.815, +3.779] |


At 500-model/default/1080p/4x, A/A paired differences span −0.048 to −0.013 ms;
reversed B/A spans **+0.993 to +1.641 ms** (median +1.081 ms). The candidate
remains slower in all five reversed pairs. The 500-model/default/4K/1x and
mid/4K/1x also retain positive gaps beyond their A/A paired ranges. Other cells
have overlapping or wide ranges; the table preserves those limits.

## Clock context and next isolation

Read-only non-admin AMD ADL queries ran about every 200 ms and at `PLAN`
transitions; NVIDIA clock/P-state queries ran about once per second. AMD
observations span 600–2200 MHz graphics clocks, memory 2400 MHz and SoC
1200 MHz. No clock, power or driver setting changed. These shared observers'
influence relative to the earlier capture is unmeasured.

Each `PLAN` bracket includes both sequential MSAA cases and warmups. Short
default brackets have only four or five clock samples, sometimes spanning clock
and activity changes. They cannot establish the clock state of an individual
render pass or a particular MSAA case. A median between observed clock values
does not prove that intermediate hardware state. These traces do **not** explain
or clear the persistent gap.

The [source/input audit](CONTROL-AUDIT.md) finds no candidate-specific pipeline,
descriptor or derived upload work in the unchanged control. Preceding workloads
may affect unobserved GPU/driver state; this remains a hypothesis. The next
fixed-binary diagnostic should compare identical controls with fresh renderer
history as well as the normal sequence, retaining resource/vertex hashes outside
timing. The rebased candidate and P2/P3 corrections still need their separate
remeasure; these old-binary controls do not validate them.
