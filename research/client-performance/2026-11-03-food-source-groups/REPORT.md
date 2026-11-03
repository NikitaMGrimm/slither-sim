# Reuse unchanged food source classifications

## Decision and measured scope

The incremental classification cache removes most source-group rebuild cost.
On the retained join's art window, RTX/Vulkan 4K p95 improves from
9.313 to 7.080 ms (Original) and 10.190 to 6.756 ms (Native), using five
accepted alternating release pairs per quality. This does not meet 4.167 ms.
These are synchronized, unpaced offscreen callback costs, not presented FPS.

Input **retained-join-A**, SHA-256
`a9e9bbdaf3c8ebc940f9c75ecfda7aa6aff58d09f396119e13ab0ead16d8cc43`:
local-only restricted recording, not distributed. No packet payloads, player
identities, coordinates or source artwork are retained in this study.
The authored rectangle has 25,000 pellets and does not copy the recording's layout.

## Why rebuilds happen

Current-main diagnostic runs found 65 rebuild callbacks in the 300-callback
art window: 22 spawn-only, 41 consumed-food-expiry-only, two with both.
There were 70 spawn and 43 expiry events in those callbacks. Every miss was
membership-epoch invalidation; none was asset-catalog invalidation. Family count
remained 132. Collection/classification accounted for about 97% of the rebuild;
validation and dense slot reconstruction took tens of microseconds.
[Aggregated diagnostics](diagnostic-profile.json) retain the counts and scopes.
Those diagnostic runs overlapped compilation, so their absolute times are not
adoption results. The diagnosis and proposed reuse were reported before building
the incremental cache.

Previously, every membership change copied each full FoodSnapshot, rebuilt its
four-resource key and revisited the tree, even for unchanged objects. The cache
now reuses the previous family assignment when the same creation occupies the
same index. New or swap-moved records still take full classification. Families
are emitted in the original ordered-key order and members in reverse world
order. Source admission, representative validation and signed-zero fallback
remain in place. Failed preparation publishes neither new sources nor lookup.

The core exposes an opaque weak creation identity. It distinguishes equal serial
IDs on rollback forks and prevents address reuse while cached, without keeping
source payloads or the food world alive. Tokens stay in the CPU owner cache,
outside queued pictures and worker jobs. The extra vector uses about eight
bytes per retained food on this 64-bit host (about 0.2 MB for 25,000), with weak
headers possibly reclaimed later. There is no new per-food payload allocation.

## Method, identities and reproduction

Timing baseline is main `bd23661930e06d6a1c6e5fc15765dde45711ea8d`, including
the worker-pool and upload-mirror changes. Candidate runtime revision is
`a5bef3a4bff7c9b73c7545a68627a52c63d68e47`. Later rebasing onto
`39a8621aea48d4352c6f84046303f3cd4673055b` brings comparison-tool changes only;
the measured runtime files are unchanged. These historical branch references
are descriptive, not a requirement that future shallow checkouts contain them.
[Identities](identities.json) record runtime and instrumented source hashes,
different sealed executable hashes, and the same private driver hash in both arms.
[Measurement deltas](measurement-delta.patch) retain passive instrumentation
relative to each runtime (zero-context patches; use `--unidiff-zero` if applying).
No current-file hash gate is installed.

Windows, Ryzen 7 9800X3D (8 cores/16 logical), RTX 5070 Ti/Vulkan and AMD
integrated graphics/DX12. Physical output is 4K for the primary case; AMD also
has a 1080p guardrail. Original uses its production capped world target; Native
uses the production resolution/MSAA plan. No adapter default changed.

The restricted driver and complete journals live in the owner's external local
performance scratch, **access: local-only/restricted**. They are not committed.
In each frozen instrumented workspace the release build command was:

```powershell
$env:CARGO_TARGET_DIR='D:/CargoTargets/community-lag'
$env:CARGO_BUILD_JOBS='2'
$env:CARGO_INCREMENTAL='0'
cargo build --release --locked --manifest-path client/apps/wgpu-slice/Cargo.toml --example join_profile --features frame-profile
```

Exact timing invocation per sealed arm (replace angle-bracket arguments with
owner-authorized local files/directories; never publish them):

```powershell
$env:PROFILE_SIZE='3840x2160'
$env:PROFILE_ADAPTER='rtx'
$env:PROFILE_BACKEND='vulkan'
$env:PROFILE_PHASES='1'
$env:PROFILE_MENUS='1'
$env:PROFILE_WORKERS='4'
$env:PROFILE_WORKER_DELAY_MS='0'
# Original: PROFILE_NATIVE absent. Native: PROFILE_NATIVE=1.
# Timing: PROFILE_PARITY and PROFILE_HASH_PIXELS absent.
& <sealed-arm.exe> replay <restricted-input> <outside-git-output> 10 0
```

AMD substitutes `igpu`, `dx12` and the listed physical size. The driver advances
600 virtual 60 Hz callbacks (ten seconds), applying 29,044 recorded envelopes.
Art is callbacks 120 through 419 inclusive. It runs native menu composition and
egui CPU painting and synchronizes GPU completion each callback. GPU figures
use diagnostic per-pass queries. CPU source, food-copy/advance and instance
preparation scopes are passive and identical in the two arms. Preparation is
nested within submit; copy is nested within advance. They must not be summed twice.

The external launcher alternates A/B then B/A, alternates quality order, and
records host load and cargo/rustc/link/test processes around every sample.
Only observed rustc/link overlap excludes a pair; cargo/test presence alone and
general load do not. All 72 final attempts remain in [runs.csv](runs.csv):
44 selected rows (22 complete pairs), 24 compiler-overlap rows and four clean
partners of excluded rows. The effect did not determine exclusions. Host load
varied substantially; do not compare absolute values with older, quieter studies.
Process Rust allocation totals include worker threads, startup and drain;
requested bytes are allocation traffic, not resident memory or driver allocations.
The replacement weak-token vectors add about 20 MB of allocation traffic per
replay (roughly 0.16%); this is a CPU-time improvement, not an allocation reduction.

A shared-target reuse attempt produced identical executable hashes and then a
missing-new-API link error. It was caught before any timing, retained in
[the receipt](build-reuse-receipt.json), and not used. The sealed arms were rebuilt
sequentially after cleaning only the two affected packages in the checked target;
compile logs and distinct executable hashes were verified. This observation
does not establish a Cargo defect.

## Results

Cells are medians of per-run quantiles. Per-run p95 uses sorted index
`floor((n-1)*0.95)`; pair ranges preserve background variation. Tail-stage cells
first select each run's slowest 16 art callbacks, then take the median within
that selection and across the five runs. [Tail summaries](tail-stages.csv)
contain no scene data. Separate stage medians do not add to a frame quantile.

| Adapter / size / quality | Pairs | Base art median/p95 ms | Cache art median/p95 ms | Paired p95 reduction median [range] ms |
| --- | ---: | ---: | ---: | ---: |
| rtx / 3840x2160 / original | 5 | 5.084/9.313 | 4.595/7.080 | +2.247 [+1.666, +3.137] |
| rtx / 3840x2160 / native | 5 | 5.053/10.190 | 4.653/6.756 | +3.433 [+1.736, +3.773] |
| igpu / 1920x1080 / original | 3 | 13.113/20.239 | 12.758/17.761 | +3.737 [+2.106, +5.037] |
| igpu / 1920x1080 / native | 3 | 13.362/21.420 | 13.345/18.015 | +3.424 [+3.405, +4.222] |
| igpu / 3840x2160 / original | 3 | 15.215/27.518 | 14.466/26.046 | +3.214 [+1.272, +4.621] |
| igpu / 3840x2160 / native | 3 | 18.298/27.439 | 17.530/23.571 | +3.868 [+1.635, +8.433] |

| RTX 4K / quality / arm | Art median range ms | Rebuild median/p95 ms | Whole median/p95 ms | Art GPU median ms | Process Rust requests / GB | Load median [range] % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| original / base | 4.605-6.800 | 2.476/3.540 | 4.271/12.327 | 0.238 | 3327102 / 12.163 | 24.3 [11.0, 87.1] |
| original / candidate | 4.574-6.774 | 0.443/0.548 | 4.099/8.963 | 0.236 | 3327707 / 12.182 | 20.4 [9.3, 92.3] |
| native / base | 4.264-7.019 | 2.844/3.517 | 4.535/12.195 | 0.307 | 3248543 / 12.075 | 23.8 [12.9, 86.8] |
| native / candidate | 4.287-6.577 | 0.487/0.580 | 4.264/9.624 | 0.323 | 3249144 / 12.095 | 22.2 [11.8, 91.9] |

| Candidate slowest 16 art callbacks, median across runs | Original ms | Native ms |
| --- | ---: | ---: |
| wall_ns | 7.7199 | 7.4188 |
| ingress_ns | 1.6520 | 1.8160 |
| food_advance_ns | 1.3288 | 1.5196 |
| food_clone_ns | 0.6839 | 0.6917 |
| pellet_sources_ns | 0.4209 | 0.5125 |
| pellet_prepare_ns | 1.4738 | 1.5146 |
| pellet_math_ns | 0.9078 | 0.9791 |
| submit_ns | 2.2329 | 2.1349 |
| submit_without_prepare_ns | 0.8516 | 0.6448 |
| snapshot_ns | 0.0185 | 0.0182 |
| prey_ns | 0.2251 | 0.2778 |
| minimap_ns | 0.1277 | 0.1461 |
| resources_ns | 0.0901 | 0.1014 |
| egui_cpu_ns | 0.0028 | 0.0029 |
| compose_ns | 0.0000 | 0.0001 |
| gpu_ns | 0.2416 | 0.2898 |

| RTX 4K / quality / arm | Measured art p95 ms | Arithmetic zero-source art p95 ms |
| --- | ---: | ---: |
| original / base | 9.313 | 7.306 |
| original / candidate | 7.080 | 6.888 |
| native / base | 10.190 | 7.195 |
| native / candidate | 6.756 | 6.593 |

The measured Original p95 reduction closes about 43% of its baseline gap to
4.167 ms; Native closes about 57%. Remaining gaps are about 2.91 and 2.59 ms.
Those are observed frame reductions under varying host load, not a claim that
every microsecond of the paired difference is caused by the source scope alone.
The arithmetic zero-source counterfactual subtracts each callback's actual
source scope before recomputing p95, rather than subtracting two percentiles.
Even zero remaining source work would leave about 6.6–6.9 ms on this schedule.
This is not a measured alternative, nor a hardware or real-240-Hz hard floor:
packet burst grouping and overlap differ at another cadence.

In the remaining slow art callbacks, ingress, food advance/copy, instance math
and the rest of submission dominate. Egui/menu composition and GPU do not set
the RTX art-period floor in this run. Food source work is now roughly half a
millisecond on rebuild callbacks and much less relevant to p95. Cold raster/font
preparation, zoom and join composition remain separate work, not changed here.

## Parity and regressions

The same sealed timing binaries also render 600 complete world images in each
of ten configurations: retained replay at RTX 4K, AMD 4K and AMD 1080p, and the
authored rectangle at RTX and AMD 4K, all in Original and Native. Per-frame image
SHA-256 sequences match exactly in all **6,000 pairs**, including spawn/fade,
consumption and expiry. Both arms have 600 callbacks and zero skipped pictures.
[Parity records](parity.json) bind both arms to the different binaries.
For image controls `PROFILE_HASH_PIXELS=1`; egui is omitted identically in both
arms. These compare the previous and new retained renderers, sharing textures,
fragment math and worker machinery; they are not an independent renderer oracle.
The active outbound command digest also matches. Canonical-quad accumulation
was inactive and printed **absent**; no canonical-quad parity is claimed.

Authored tests compare grouping/ranges/slots with the original tree oracle,
including swap removals, conflicting static metadata, retry after source failure,
signed-zero fallback, equal-serial sibling creations, weak lifetime, rollback and
queued-picture isolation. The existing 25,000-pellet regression exercises changing
animation and camera states. [Mutation checks](mutation-checks.json) show that
forcing all creation identities
to match fails the sibling-creation assertion; removing the cached-family
metadata guard fails the canonical-fallback assertion. Both failures were test
assertions, not compiler errors. Source bytes were restored and verified.

The full Windows `npm run check` passed: core/native tests, strict native/Wasm
Clippy (including frame-profile and latency-rig), desktop and Wasm builds,
Canvas real-Wasm ABI checks and repository guards. Focused core identity and
all 14 non-GPU pellet-batch regressions passed. Study recompute and tests pass,
including rejection of wrong binary identities, incomplete callbacks, skips,
malformed digests, wrong configurations and incorrect exclusions. Browser GPU
execution remains unverified.

After rebasing, the focused core identity and 14 pellet-batch tests, all 48
updated comparison-tool tests and the history guard passed. The authored
offscreen worker/serial lifecycle image test also passed on RTX/Vulkan and
AMD/DX12, covering both quality modes, non-empty prefix/overlay, resize and
offline reconnect. The rebased runtime source hashes match the measured runtime.

## Other attempts and limits

[Exploratory rows](exploratory-runs.csv) retain all 168 completed earlier attempts.
Hash collection followed by key sorting was slower and rejected; a once-per-family
admission trial alone gave only a small gain. The winning cache was then piloted
against the earlier serial composition before the current-main measurements above.
Those earlier numbers are not pooled into the adoption table. One partial guard
sample was stopped after the hash trial was rejected; its partial journals and
stop receipt remain outside Git. The rejected implementation is retained as
[a patch](rejected-hash.patch). These exploratory driver/source compositions are
historical, not final-composition parity evidence.

No new connection, visible window, presented-interval measurement or owner play
test was performed. Full-image replay controls do not establish browser GPU,
surface pacing, arbitrary custom asset recipes or actual 240 Hz acceptance.
Wasm/browser build and authored ABI checks are covered by the full client check.
No smoothing, GPU shader, protocol, adapter or visual policy changed.

To recompute every result table and validate retained selection/controls, using
stdlib Python from the repository root (no capture, GPU or network needed):

```sh
python -B research/client-performance/2026-11-03-food-source-groups/recompute.py --check
python -B -m unittest discover -s research/client-performance/2026-11-03-food-source-groups -p test_study.py
```
