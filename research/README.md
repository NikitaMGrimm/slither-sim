# Research records

Use the [contributor task map](../CONTRIBUTING.md#find-the-change) for code changes
and `docs/` for current commands/behavior. This archive holds dated studies and
selected results, including rejected and inconclusive attempts. Collection or
archive integrity establishes neither scientific rules nor product adoption.

| Topic | Study | Scope and decision |
| --- | --- | --- |
| Battledome | [Pit-only food](battledome/2026-10-27-pit-food/REPORT.md) | Owner-approved local play build, full floor-death drops filtered to the pit, and short matched 32/64-seat food-supply checks; no long-run equilibrium claim. |
| Battledome | [Opt-in mode rules](battledome/2026-10-27-rules-extension/REPORT.md) | Pinned stock handler ignores the extension; loopback opt-in exclusion, exact live rules and shared Forager adapter regressions. |
| Fidelity | [Shared browser admission](fidelity/2026-10-27-browser-admission/REPORT.md) | Client-derived shared challenge and short modern login; loopback browser acceptance with existing output protocols. |
| Fidelity | [Protocol-13 host compatibility](fidelity/2026-10-27-protocol13-host/REPORT.md) | Local replacement/stock browser acceptance, shared-world wire adapter and official-500 output parity; no new outside capture. |
| Fidelity | [Protocol-13 message catalog](fidelity/2026-10-26-protocol13-catalog/REPORT.md) | Official legacy/stock captures, scrubbed wire-13/19 witnesses and an offline decoder; conditional timing and failed-attempt limits. |
| Fidelity | [Legacy browser login](fidelity/2026-10-26-legacy-login/REPORT.md) | Two bounded official sessions: existing 27-byte challenge format, selector-12 login then protocol-13 setup; implementation stopped before changing the protocol-19 host. |
| Performance | [One-core EPYC socket capacity](performance/2026-10-26-epyc-host/REPORT.md) | Sequential official 250/500 and Battledome 32/80 socket holds on one Zen 4 and one Zen 5 core; delivery grades, spawn waits, operator stats and cloud cleanup. |
| Performance | [Hosting resource samples](performance/2026-10-26-hosting-sizes/REPORT.md) | Three short loopback socket holds on one desktop core; CPU/RSS sizing only, with depleted Battledome admission and no VPS capacity claim. |
| Bots | [A4 adoption](bots/2026-10-26-a4-adoption/REPORT.md) | A4-mean becomes the public default, with A1 selectable; selected private held-out gameplay and cost results, maintainer mean-cost gate and retained tail-limit failures. |
| Bots | [Frozen Forager port](bots/2026-10-26-forager-port/REPORT.md) | Immutable A1 and test-only A4-mean parameters, selected decision conformance; runtime defaults unchanged. |
| Battledome | [Stock marker visibility](battledome/2026-10-26-stock-presentation/REPORT.md) | Offline packet and stock fade lifetimes: retained failing shrink baseline, rapid replacement listings and overlapping ring generations; no browser pixel claim. |
| Battledome | [Crowded spawn capacity](battledome/2026-10-26-spawn-capacity/REPORT.md) | Fixed strict, fixed relaxation and proportional arena growth; loopback waits, early safety and admission CPU at 40/64/80 seats. |
| Battledome | [Pit-aware background bots](battledome/2026-10-27-pit-aware-bots/REPORT.md) | Shared A4 enters the live pit; three-seed offline inside populations, pit deaths and body kills, with unchanged pit-less decisions. |
| Battledome | [Constant-density admission](battledome/2026-10-27-arena-playtest/REPORT.md) | Owner-approved 64-seat playtest, fixed/density spawn screens and directional pilots; rejected attempts and safety/capacity limits retained. |
| Battledome | [Safe spawn band](battledome/2026-10-26-safe-spawn/REPORT.md) | Local 120-second sensitivity grid and retained initial attempt; cached admission, head-path/departure protection and socket-first waiting. |
| Battledome | [Food balance](battledome/2026-10-25-food-balance/REPORT.md) | Offline custom-mode before/after runs at 32 and 64 slots; floor-aware death food and total-food refill. |
| Fidelity | [Official baseline selection](fidelity/2026-10-23-official-defaults/REPORT.md) | Configuration choices and explicit model assumptions; no new measurements. |
| Fidelity | [Visibility pilot](fidelity/2026-10-26-visibility-pilot/REPORT.md) | Failed cardinal-x feasibility gate: verified paired archives but no qualified route exposure; diagonal unrun, no visibility law fitted. |
| Fidelity | [Food-interest pilot](fidelity/2026-10-26-food-interest/REPORT.md) | Failed frozen P3 feasibility gate: three completed slots, fourth aborted for avoidance; verified archive, five invocations unrun, no food-interest or replacement inference. |
| Fidelity | [Movement scheduler](fidelity/2026-10-26-movement-scheduler/REPORT.md) | Six verified frozen invocations, held-out wire-gap counterexamples and conditional wanted-angle reflection bounds; coverage and transport gates pass, no replacement scheduler selected. |
| Fidelity | [Own wanted/speed gate](fidelity/2026-10-26-wanted-gate/REPORT.md) | Phase-conditioned offline P2 check supports keeping the official motion gate; screened early reflections and ingress uncertainty retained. |
| Fidelity | [Collision pilot](fidelity/2026-10-26-collision-pilot/REPORT.md) | [Failed official pilot one](fidelity/2026-10-26-collision-pilot/EXECUTION.md): seven attempted slots, zero launched; pilot two stopped by the frozen gate, no online rule fitted. |
| Fidelity | [Arena radius](fidelity/2026-10-26-arena-radius/REPORT.md) | Exploratory retrospective audit: sustained official waves, rejected predictive candidates and death-driven controller jitter; hold initial radius as a practical default, not an identified official law. |
| Fidelity | [Transport parity reference](fidelity/2026-10-25-transport-parity/REPORT.md) | Offline official headless summaries, empirical transport gates and a localhost host baseline. |
| Fidelity | [Sparse-view cadence](fidelity/2026-10-26-sparse-view-cadence/REPORT.md) | Offline conditional cadence: sparse official views slow too; no heartbeat change, view-matched gates recommended; the residual 6–10-peer difference is traced to workload in the follow-up below. |
| Fidelity | [Cadence residual](fidelity/2026-10-26-cadence-residual/REPORT.md) | Offline follow-up: local speed/boost activity and concentrated report phases explain most of the sparse-view residual in synthetic occupancy sensitivities; no host change. |
| Fidelity | [Host transport with 500 bots](fidelity/2026-10-26-host-transport-500-bots/REPORT.md) | Three AWS loopback repetitions per arm: 500-bot pool passes, but separate cadence and held-out delay failures prevent a consistent-parity claim; sparse baseline and acquisition failures retained. |
| Fidelity | [Packet scheduler](fidelity/2026-10-25-packet-scheduler/REPORT.md) | Offline retained protocol-19 census; adopted per-tick emission and partial per-family gates; counterexamples and unknown exact rules retained. |
| Fidelity | [Delivery profiles](fidelity/2026-10-25-delivery-profiles/REPORT.md) | Local official/naive/responsive comparison: prompt input and mass reports with six-tick steady turns; 500-seat measurements and failed overloaded 64-seat Battledome attempt retained. |
| Fidelity | [Network path](fidelity/2026-10-25-network-path/REPORT.md) | Retained official-route delivery fit with retrospective held-out checks; p99/ACF mismatches, chosen uplink assumptions and parent-reported latency/input-timing adoption. |
| Fidelity | [Fresh official capture](fidelity/2026-10-25-official-capture/REPORT.md) | Six ten-minute batches on two official endpoints; contaminated timing leaves pause tails unresolved; 243 fresh joins distinguish neither spawn rule; no model change. |
| Fidelity | [Remote timing study](fidelity/2026-10-26-vps-timing/REPORT.md) | Corrected death-close handling; six verified batches with near-zero held-out delivery tails and persistent server-clock scheduler exceptions; four earlier interrupted streams remain quarantined; no model change. |
| Fidelity | [Clean timing references](fidelity/2026-10-26-server-timing-references/REPORT.md) | Offline reference audit, unfiltered transport LOO envelope and separate masked characterization from completed remote streams; retained failed envelopes, home cases and selected scheduler regressions; no model/runtime default change. |
| Fidelity | [Spawn admission](fidelity/2026-10-25-spawn-admission/REPORT.md) | Retained official join clearances; adopted a most-open host admission fallback that still refuses when no sampled pose clears contact range, and seat-sized dynamic host arenas; the official rejection rule stays unidentified. |
| Fidelity | [Tail/turn jitter](fidelity/2026-10-25-tail-jitter/REPORT.md) | Exploratory shared Windows desktop; batching/stall diagnosis, reverted staggering and owner-reported 500-bot after evidence (43% simulation load, zero deferrals). |
| Fidelity | [Visibility recheck and follow-ups](fidelity/2026-10-23-visibility-recheck/REPORT.md) | Acquisition successes and failures; no validated replacement subscription rule. |
| Fidelity | [Observation precision](fidelity/2026-10-24-observation-precision/REPORT.md) | Protocol-19 field grids in three local-only captures; adopted 0.01-unit, 0.0001-rad and 0.01-unit/s steps for `serve` frames. |
| Performance | [Initial cost baseline](performance/2026-10-23-baseline/REPORT.md) | Exploratory synthetic costs; not an isolated host or stable production comparison. |
| Performance | [Native Linux host capacity](performance/2026-10-26-host-capacity/REPORT.md) | One core holds 100 socket clients at 2.4 ms excess p99 for 300 s; 250 fails the 300 s confirmation and the world falls behind at 500. 500 background bots run in real time. CPU profiles; no optimization. |
| Performance | [Host broadcast and frame ownership](performance/2026-10-26-host-efficiency/REPORT.md) | Adopted stable food subscriptions, shared peer deltas and owned frames: byte-identical traces, about 5% less host CPU per tick at 250 and 500 clients; 250 narrowly passes 10 ms, 500 still falls behind. Superseded pre-#113 round retained. |
| Performance | [Host known-slot enumeration](performance/2026-10-26-host-known-slots/REPORT.md) | Official-500 and admission-limited boosting Battledome 40/64/80: byte-identical traces and descriptive WSL2 CPU/instruction pairs; no native capacity claim. |
| Performance | [Shared prey visibility bounds](performance/2026-10-26-host-prey-visibility/REPORT.md) | Byte-identical bounds reduce fixed-fixture instructions; cold-start WSL2 socket CPU is inconclusive at official-500 and Battledome-40/64/80; native capacity unqualified, admission unchanged. |
| Performance | [Sparse food delivery](performance/2026-10-26-host-food-sharing/REPORT.md) | Shared change hints remove repeated recipient scans: byte-identical traces and 6.53% fewer official-500 fixture instructions; controlled Battledome fixtures and contaminated WSL2 live timings, no capacity claim. |
| Performance | [Incremental food-sector union](performance/2026-10-27-host-sector-union/REPORT.md) | Changed memberships update the shared union: nine byte-identical traces and 2.513% fewer official-500 fixture user instructions; no live CPU or capacity claim. |
| Performance | [Shared minimap packets](performance/2026-10-27-host-minimap-sharing/REPORT.md) | Shared encodings remove repeated map-due work: twelve byte-identical traces and 1.97% fewer official-500/400-decision fixture instructions; map-due ticks drop from 291M to 19M simulation-thread instructions. Shared-machine wall times are not speed claims. |
| Performance | [Legacy minimaps and tail ticks](performance/2026-10-27-host-tail-ticks/REPORT.md) | Shared legacy map conversion: fourteen byte-identical traces and 0.709%/3.516% fewer mixed-official/all-legacy-dome fixture instructions; default Battledome tails remain admission dominated with waiting players. |
| Performance | [500-bot AWS qualification](performance/2026-10-26-500-bot-qualification/REPORT.md) | 500 default A4 bots plus a watcher, one pinned simulation/socket core and two bot workers: six 300-second parent/merge holds pass both fixed delivery grades; no #136 regression beyond trial spread. Spot interruptions and supplementary holds retained. |
| Performance | [Forager kernel shortcuts](performance/2026-10-26-forager-kernel/REPORT.md) | Exact occupancy and clearance shortcuts; platform conformance and paired AWS replay measurements. |
| Performance | [Battledome spawn cost](performance/2026-10-25-battledome-spawn-cost/REPORT.md) | Offline host admission profiling; bounded scoring preserves selected poses, refused host bots wait one wall second, and spawn-ring capacity is measured separately from CPU cost. |
| Performance | [Collision bounds lookup](performance/2026-10-23-collision-bounds/REPORT.md) | Adopted lookup change; gain specific to the separated-body fixture. |
| Performance | [Observation batches](performance/2026-10-23-observation-batches/REPORT.md) | Early return adopted for empty batches; rejected attempt and mixed nonempty results retained. |
| Performance | [Serve transport v1 → v2](performance/2026-10-24-transport-v2/REPORT.md) | Controller-side Python/Node costs on one shared Windows desktop; v2 replies 7–27× smaller and steps 3.5–13.6× faster in four synthetic scenarios; 16 worlds step 4× faster server-side on the default pool than with one thread. |
| Performance | [Borrowed observation views](performance/2026-10-24-borrowed-views/REPORT.md) | Not adopted: borrowed fill-and-read batches were 14% faster in a dense sequential world but up to 17% slower in the official 128-snake world; attempt kept as a patch. |
| Performance | [PR report row audit](performance/2026-10-26-pr-report-audit/REPORT.md) | Retained merged-PR comments, row costs and A/A calibration; six targeted timing rows with ten unchanged deterministic traces. |
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
