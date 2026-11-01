# AMD unchanged-control audit

Open merge blocker: the 500-arena/default/1080p/4x AMD control takes
2.4324 → 3.4860 ms despite an exact default frame. This audit compares measured
baseline `181bc01f21441d3447cad7acefec1f683b347e29` with measured candidate
`583c502478fce0e576c82af1de5103bd21bc46ff`. It uses source and retained CPU draw
snapshots/CSVs, with no new GPU execution. It does not explain the timing gap.

The table below uses baseline executable SHA-256
`82fda4f9c33e4f56b0b23d4e81857a174856cd0202f9aef45dc9da59436559bd` and
candidate SHA-256 `42fe9aa38ab6effd10abf32d83b7053bc0af03675e3aa3665bfdaabff173eb32`,
from those exact source commits. The [completed A/A and reversed B/A results](CONTROL-RESULTS.md)
retain the merge blocker; the gap was not reproduced by the identical-binary
control and persists when execution order is reversed.

## Inputs and source paths

| Input | Baseline | Candidate | Evidence |
| --- | --- | --- | --- |
| Complete frame at measured picture 39 | SHA-256 `7cc1abc0a5572800147036947b7b46e512cef1edb1f2b28bf9d8ff7741a65fbe` | Same | Exact in all five retained pairs |
| Quads / vertices / draw calls | 1,246 / 7,476 / 686 | Same | Every measured control frame, 150 per side |
| Referenced texture metadata | 38 immutable handle/revision/size tuples | Same | Parsed from exact frames |
| Referenced RGBA storage | 1,537,184 bytes | Same | Sum of unique width × height × 4 |
| Vertex writes | One `write_buffer`, 149,520 bytes/frame | Same | 1,246 × 120 serialized bytes, unchanged renderer |
| Used vertex buffer capacity | 983,040 bytes | Same | Code-derived from prior matrix history |
| Retained vertex buffers at control | Nine × 983,040 bytes | Same | Size-policy replay of preceding measured history |
| Color resolve target | 1920 × 1080, RGBA8Unorm, 1x (8,294,400 bytes) | Same | Same driver descriptors |
| MSAA attachment | 1920 × 1080, RGBA8Unorm, 4x (33,177,600 logical bytes) | Same | Same renderer descriptors; driver-private storage excluded |
| Bind groups | Clamp/repeat pair per resident texture; texture + filtering sampler | Same | Identical renderer and texture metadata |
| Pipelines | Over, black-over and lighter, each at 1x and 4x | Same | Identical shader, blend, layout and creation code |
| Diagnostic queries | Two timestamps, 16-byte resolve + 16-byte readback buffers per frame | Same | Same driver, requests and timing scope |
| Preparation observer in timing runs | Unset | Unset | Same driver environment; separate profile runs excluded |

The renderer, shader, draw-run resolver, frame geometry conversion, resource
bank, text cache and procedural materializer are identical Git blobs between
these measured commits. The optimization commit changes only the body planner,
its export, the renderer's planner call and planner tests. The control passes
wheel factor one, so its new `distant` branch is false; command count, resource
selection and all emitted commands stay unchanged.

`Renderer::from_adapter_for_diagnostics` requests the same features/limits and
creates all six pipelines before the workload. Pipeline selection depends on
the unchanged quad blend/black-effect flag and the texture's black-pixel
classification, from unchanged procedural/text rasterization. No shader, bind
layout, sampler, target format, MSAA count or pipeline descriptor changed.

`Bank::resources` selects immutable resources from the frame's handle/revision
set; the driver creates a fresh bank and calls `sync_resources(&[])` at each
zoom level. The warmup therefore starts with empty GPU texture residency on both
sides. `sync_resources` uploads only absent revisions, using width × height × 4
bytes. Frozen ingress, stable text keys and the unchanged minimap retain those
revisions after warmup, implying no repeated texture writes in measured control
frames. This is a source-derived result: the original driver did **not** record
texture-write events or per-resource pixel hashes. CPU allocation-byte counters
are not substitutes for GPU upload counters.

The earlier 120-model/full-map/1080p case does change, so history is a plausible
source of GPU state or clock differences even though the later control's inputs
match. The intervening unchanged 120-model/4K/full-map case grows all nine
retained vertex buffers to their same cap on both sides. Retained sizes above
are derived from the public size policy and measured history, not a capture of
GPU allocation events. Warmup pictures are absent from the CSV; unobserved driver
heaps, cache state, clocks and scheduling remain outside this audit.

## Completed follow-up

The follow-up used the original retained executables with redundant PNGs
disabled: five alternating A/A pairs (baseline executable for both labels),
followed by five reversed B/A pairs. The complete workload history and both
adapters retain the same predecessor scenes. Executable hashes, compiler
overlap, system load and clock context are recorded. The result compares
**every unaffected AMD cell** at both sample counts.
The booked window was 08:00–08:25 local, with compiler-exit extension capped at
08:40. All 20 captures completed by 08:20:15. See the linked results for every
unaffected cell and clock limitations; the merge blocker remains open.

A non-admin probe of the installed AMD ADL read-only telemetry getter succeeded:
graphics, memory and SoC clock fields are supported. The control-run supervisor
sampled them about every 200 ms, also when a new `PLAN` line arrived, and
retained NVIDIA clock/P-state queries about once per second. These are observers
shared by all follow-up arms; their possible influence must be reported. The
retained benchmark has one `PLAN` bracket for both sequential MSAA cases, so
clock context cannot be assigned to an individual timed frame or sample count.
The API's sensor definitions and getter contract are documented by
[AMD GPUOpen](https://gpuopen-librariesandsdks.github.io/adl/group__OVERDRIVE8API.html).
This reads telemetry only; no clock, power or driver setting is changed.
