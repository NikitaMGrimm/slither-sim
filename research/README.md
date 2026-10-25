# Research records

Use the [contributor task map](../CONTRIBUTING.md#find-the-change) for code changes
and `docs/` for current commands/behavior. This archive holds dated studies and
selected results, including rejected and inconclusive attempts. Collection or
archive integrity establishes neither scientific rules nor product adoption.

| Topic | Study | Scope and decision |
| --- | --- | --- |
| Battledome | [Food balance](battledome/2026-10-25-food-balance/REPORT.md) | Offline custom-mode before/after runs at 32 and 64 slots; floor-aware death food and total-food refill. |
| Fidelity | [Official baseline selection](fidelity/2026-10-23-official-defaults/REPORT.md) | Configuration choices and explicit model assumptions; no new measurements. |
| Fidelity | [Transport parity reference](fidelity/2026-10-25-transport-parity/REPORT.md) | Offline official headless summaries, empirical transport gates and a localhost host baseline. |
| Fidelity | [Packet scheduler](fidelity/2026-10-25-packet-scheduler/REPORT.md) | Offline retained protocol-19 census; adopted per-tick emission and partial per-family gates; counterexamples and unknown exact rules retained. |
| Fidelity | [Delivery profiles](fidelity/2026-10-25-delivery-profiles/REPORT.md) | Local official/naive/responsive comparison: prompt input and mass reports with six-tick steady turns; 500-seat measurements and failed overloaded 64-seat Battledome attempt retained. |
| Fidelity | [Network path](fidelity/2026-10-25-network-path/REPORT.md) | Retained official-route delivery fit with retrospective held-out checks; p99/ACF mismatches, chosen uplink assumptions and parent-reported latency/input-timing adoption. |
| Fidelity | [Fresh official capture](fidelity/2026-10-25-official-capture/REPORT.md) | Six ten-minute batches on two official endpoints; contaminated timing leaves pause tails unresolved; 243 fresh joins distinguish neither spawn rule; no model change. |
| Fidelity | [Remote timing attempts](fidelity/2026-10-26-vps-timing/REPORT.md) | Raw audit corrects requested post-death closes misclassified by study wrappers; four interrupted streams stay quarantined, endpoint-unusable claim withdrawn; corrected second continuation. |
| Fidelity | [Spawn admission](fidelity/2026-10-25-spawn-admission/REPORT.md) | Retained official join clearances; adopted never-refuse host admission with a most-open fallback and seat-sized dynamic host arenas; the official rejection rule stays unidentified. |
| Fidelity | [Tail/turn jitter](fidelity/2026-10-25-tail-jitter/REPORT.md) | Exploratory shared Windows desktop; batching/stall diagnosis, reverted staggering and owner-reported 500-bot after evidence (43% simulation load, zero deferrals). |
| Performance | [Initial cost baseline](performance/2026-10-23-baseline/REPORT.md) | Exploratory synthetic costs; not an isolated host or stable production comparison. |
| Performance | [Collision bounds lookup](performance/2026-10-23-collision-bounds/REPORT.md) | Adopted lookup change; gain specific to the separated-body fixture. |
| Performance | [Observation batches](performance/2026-10-23-observation-batches/REPORT.md) | Early return adopted for empty batches; rejected attempt and mixed nonempty results retained. |
| Fidelity | [Visibility recheck and follow-ups](fidelity/2026-10-23-visibility-recheck/REPORT.md) | Acquisition successes and failures; no validated replacement subscription rule. |
| Fidelity | [Observation precision](fidelity/2026-10-24-observation-precision/REPORT.md) | Protocol-19 field grids in three local-only captures; adopted 0.01-unit, 0.0001-rad and 0.01-unit/s steps for `serve` frames. |
| Performance | [Serve transport v1 → v2](performance/2026-10-24-transport-v2/REPORT.md) | Controller-side Python/Node costs on one shared Windows desktop; v2 replies 7–27× smaller and steps 3.5–13.6× faster in four synthetic scenarios; 16 worlds step 4× faster server-side on the default pool than with one thread. |
| Performance | [Borrowed observation views](performance/2026-10-24-borrowed-views/REPORT.md) | Not adopted: borrowed fill-and-read batches were 14% faster in a dense sequential world but up to 17% slower in the official 128-snake world; attempt kept as a patch. |
| Performance | [PR comparison statistics](performance/2026-10-24-pr-comparison-statistics/REPORT.md) | v1 error-controlled comparison for PR reports, replaced by protocol v2 for new runs: frozen contract and SciPy reference vectors; no new measurements, hosted calibration pending. |
| Performance | [Observation food index](performance/2026-10-24-observation-food-index/REPORT.md) | Adopted finer food-grid cells with rows trimmed to the vision circle: observation 16–19% faster sequentially and 5–12% in parallel in two synthetic worlds; finer cells only when observers repay them; up to about 15% slower when food is concentrated in patches away from the observers; `observe-wide` and `observe-strip` about 2.5–4% slower; the step's food caches cannot serve observation and a persistent index was not prototyped. |

## Adding or updating material

- Update the existing `docs/` guide for command, API or explanation changes;
  routine fixes usually need no research report.
- For a distinct study, use `research/<topic>/YYYY-MM-DD-<question>/REPORT.md` and
  link it here. One report should contain question, methods, source/build pins,
  outcomes (including failure), limits, decision and reproduction/evidence links.
  Study amendments can stay together; a PR or agent turn is not a new study.
  Preserve frozen plans and historical conclusions.
- Put only useful, bounded result data beside the report. Reuse the existing
  [evidence bundle format](../docs/evidence-format.md); do not add a parallel
  claims/status registry. Keep small regressions in `tests/fixtures/`, reusable
  tools in existing `tools/` paths. Source-snapshot tools do not automatically
  archive this tree.
- Keep raw recordings, full traces, checkpoints, binaries and changing handoffs
  outside Git. When sharing is permitted, give a retrievable URL and digest;
  otherwise label access local-only, restricted or unavailable. Paths/hashes
  identify material without making it downloadable. Historical paths in selected
  JSON are original provenance, not relative file links.

SlitherSim owns simulator/protocol fidelity and simulator/client performance.
Arena owns policy, reward, planner and training/evaluation research, consuming
SlitherSim as a dependency. Do not copy private Arena data here.
Import manifests stay at their historical `docs/` paths.

`research/` is excluded from the Rust crate; packaged guides use repository URLs
to remain usable without the archive. No research framework or extra dependency
is required.
