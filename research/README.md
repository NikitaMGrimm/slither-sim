# Research records

Use the [contributor task map](../CONTRIBUTING.md#find-the-change) for code changes
and the maintained guides in `docs/` for current commands and behavior. This tree
holds dated studies and selected results, including rejected and inconclusive
attempts. A completed collection or passing archive check does not establish a
scientific rule or product adoption.

| Topic | Study | Scope and decision |
| --- | --- | --- |
| Fidelity | [Official baseline selection](fidelity/2026-10-23-official-defaults/REPORT.md) | Configuration choices and explicit model assumptions; no new measurements. |
| Performance | [Initial cost baseline](performance/2026-10-23-baseline/REPORT.md) | Exploratory synthetic costs; not an isolated host or stable production comparison. |
| Performance | [Collision bounds lookup](performance/2026-10-23-collision-bounds/REPORT.md) | Adopted lookup change; gain specific to the separated-body fixture. |
| Performance | [Observation batches](performance/2026-10-23-observation-batches/REPORT.md) | Early return adopted for empty batches; rejected attempt and mixed nonempty results retained. |
| Fidelity | [Visibility recheck and follow-ups](fidelity/2026-10-23-visibility-recheck/REPORT.md) | Acquisition successes and failures; no validated replacement subscription rule. |
| Fidelity | [Observation precision](fidelity/2026-10-24-observation-precision/REPORT.md) | Protocol-19 field grids in three local-only captures; adopted 0.01-unit, 0.0001-rad and 0.01-unit/s steps for `serve` frames. |
| Performance | [Serve transport v1 → v2](performance/2026-10-24-transport-v2/REPORT.md) | Controller-side Python/Node costs on one shared Windows desktop; v2 replies 7–27× smaller and steps 3.5–13.6× faster in four synthetic scenarios; 16 worlds step 4× faster server-side on the default pool than with one thread. |
| Performance | [Borrowed observation views](performance/2026-10-24-borrowed-views/REPORT.md) | Not adopted: borrowed fill-and-read batches were 14% faster in a dense sequential world but up to 17% slower in the official 128-snake world; attempt kept as a patch. |
| Performance | [Observation food index](performance/2026-10-24-observation-food-index/REPORT.md) | Adopted finer food-grid cells with rows trimmed to the vision circle: observation 16–19% faster sequentially and 5–12% in parallel in two synthetic worlds; finer cells only when observers repay them; `observe-wide` and `observe-strip` about 3% slower; the step's food caches cannot serve observation and a persistent index was not prototyped. |

## Adding or updating material

- Update an existing `docs/` guide when its commands, API or current explanation
  change. A routine fix usually needs no research report.
- For a distinct study, use `research/<topic>/YYYY-MM-DD-<question>/REPORT.md` and
  link it here. Start with one report: question, methods and source/build pins,
  outcomes (including failure), limitations, decision and reproduction/evidence
  pointers. Amendments to the same study can stay together; a PR or agent turn is
  not a new study. Preserve frozen plans and historical conclusions.
- Put only useful, bounded result data beside the report. Reuse the existing
  [evidence bundle format](../docs/evidence-format.md); do not add a parallel
  claims/status registry. Keep small regressions in `tests/fixtures/` and reusable
  tools in their existing `tools/` paths. The source-snapshot tools do not
  automatically archive this research tree.
- Keep raw recordings, full traces, checkpoints, binaries and changing handoffs
  outside Git. Give a retrievable URL and digest when sharing is permitted;
  otherwise label access as local-only, restricted or unavailable. A path and
  hash identify material without making it downloadable. Historical paths inside
  selected JSON are original provenance, not new relative file links.

SlitherSim owns simulator/protocol fidelity and simulator/client performance.
Arena owns policy, reward, planner and training/evaluation research and consumes
SlitherSim as a dependency. Do not copy private Arena data into this archive.
The existing import manifests stay at their historical paths under `docs/`.

`research/` is repository material, excluded from the published Rust crate.
Packaged guides link to it using repository URLs; they remain usable without an
archive checkout. No research framework or additional dependency is required.
