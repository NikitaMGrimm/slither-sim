# Attempts and tooling notes

- Initial build stopped before extraction: Linux Git cannot interpret this
  Windows worktree's `<repo-gitdir>` gitdir file. Reproduction: run build with the
  `<repo>` repository argument. The main checkout's common
  object database is readable by Linux Git, so subsequent builds use
  `<repo>` as the repository argument, retaining
  the explicit ef866d7 source pin. Failed output remains in its original folder.
- v2 builds the plain binary successfully, then stops at an ambiguous diagnostic
  anchor (both collision and prey paths query a BoundsIndex). Narrowed the
  anchor to the collision closure signature; no measurement was selected.
- A fresh WSL launch returned Wsl/Service/0x8007274c before the v3 build started.
  Other launches were delayed too. A persistent taskset-limited shell recovered
  access without restarting WSL or changing system settings.
- A v2 plain-binary 500-slot warmup pilot is retained separately. It is
  exploratory, excluded from selected counts and does not replace the frozen
  convergence screen.
- The original-search collection retained a complete 500-slot row (130,000 warm
  ticks, untallied run, five counts and endpoint-equal diagnostic), and a complete
  1000-slot warmup (140,000 ticks). Preparation was stopped before the first
  10,000-slot block completed: the full geometry search made large warmups
  impractical. Its partial logs remain in `population-scaling-20261027-final`.
  This was an explicit preparation change, not an excluded low-performing
  measurement. Those completed rows are retained as original-search warmup
  oracles; selected results will use the final harness in fresh output.
- `population-scaling-20261027-bounded` is a compiled warmup-helper prototype,
  superseded before collection by the version that also records bot PMU
  scheduling intervals, enforces one-CPU affinity and includes pose/RNG oracles.
- The final WSL preparation completed 500-slot warmup equivalence at 120,000
  and 130,000 ticks, the bot split, and capped-10k warmup at 70,000 ticks.
  The uncapped-10k warmup was still before its first flushed block after
  3343.74 CPU seconds. The maintaining agent set a one CPU-hour limit,
  required an explicit unsettled result and prohibited further computation on
  the owner's Windows workstation because another worker was timing there.
  All our WSL study processes were terminated
  and their absence verified. Partial and completed WSL artifacts are retained,
  excluded from the final VM scaling table.
- A cold WSL diagnostic found that uncapped 10k rejects the dense eater grid
  (1,073,296 rectangle cells versus the fixed 1,048,576 ceiling). Its hashed
  eater-bitset fallback costs about 4.5 billion pickup instructions per tick
  with newborn bodies. This motivated the bounded/onset diagnostics, not a
  runtime change. Those preliminary numbers do not qualify a steady row.
- Continued on a 4-vCPU Zen 5 EPYC KVM guest; no machine was
  provisioned. Re-fetched the pinned Git runtime, fetched locked dependencies,
  then built offline under `<vm-root>`.
  Both release builds pass. All new builds, study children and checks stay on
  CPUs 2,3 of this VM, each measured child pinned to one CPU. Kernel policy stays
  unchanged. The VM's global Rust installation requires PATH lookup rather
  than assuming `~/.cargo/bin`; the harness now uses `shutil.which`.
- Remote inspection command `rg` failed because ripgrep is absent there;
  subsequent process inspection uses `grep`. No study measurement was lost.
- The remote selector's six synthetic window/censoring guards pass. During
  bounded-control and bot collection, both SSH sessions closed and new SSH and
  remote exec requests returned `machine unreachable` / `host_unavailable`,
  while remote status still reported running/ready. At the last
  successful inspection, the bounded control had flushed tick 291 (80.50
  process CPU seconds). No new control process may start until its actual
  consumed CPU is recovered; partial output must be retained.
- Connectivity recovered without a restart; both jobs survived. The bounded
  run and all three onset probes exited successfully, consuming 118.052659
  aggregate process CPU seconds. Total charged control CPU is 3521.792659
  seconds, including the conservative WSL allowance, below 3600. No further
  uncapped-10k simulation is run. The full primary collector starts while the
  bot repeats finish on CPU 3; affinity remains restricted to CPUs 2,3.
- Once each full-length 500 oracle preparation finished on CPU 2, its remaining
  120k-tick partner was moved from CPU 3 to idle CPU 2 to stop competing with
  the bot sample. These are uncounted preparations only; measured processes
  retain their one-CPU launch affinity.
- The final selector is named `select_results.py` to avoid shadowing Python's
  standard-library `select` on installations where that module is dynamic.
  The VM's Python uses a built-in `select`; its existing subprocess imports
  pass. This changes analysis naming only, not a compiled simulation template.
- The primary reproduction CLI no longer offers uncapped 10k warm/count modes;
  that case, censored by the maintaining agent, is collected only by the
  CPU-bounded probe script.
  The six primary cases and compiled Rust templates are unchanged.
- When the maintaining agent cut this study to one CPU under the owner's
  2026-10-28 machine budget, both large warmups were stopped
  immediately; the old parallel Python controller processes were terminated.
  Both warmups were pinned to CPU 2, with only the 5k control resumed. `/proc`
  reports one thread each, about 345/307 MB resident memory, one running and
  one stopped. `resume_collection.py` finishes these states sequentially before
  starting any counted checkpoint loads, then runs the remaining rows in order.
  All new child environments use jobs/Rayon/tests=1. Old warmups have no Rayon
  work and no virtual-time dependence on the pause. No compiled Rust template,
  controller stream, convergence rule or counted-window length changed.
- Verified all copied binary hashes, resolved the two exact task-owned target
  paths under the retained output, then deleted `plain/target` and
  `profile/target`. Copied binaries, archived runtime/overlay sources, logs and
  unique evidence remain. Identical endpoint files are retained with hard links.
- New native children also have a process-local 2.5 GiB address-space ceiling
  (`RLIMIT_AS`), keeping their resident memory below the maintaining agent's
  approximate 3 GB budget. Checkpoint-loading counts begin only after both retained worlds
  have exited. A limit failure is retained as a failed attempt, never hidden.
- The mature 5k checkpoint failed restoration under the 2.5 GiB address-space
  guard. The production importer held its generic JSON tree and typed world
  together; an exploratory reload without that guard was stopped at 2.25 GiB
  RSS. Both attempts are retained. This was setup, before a counted window.
- A first memory-bounded preparation prototype validated and released the full
  generic tree before typed decoding. It is retained in `results-memory-bounded`
  and superseded: its generic-tree peak still made the larger worlds risky.
- The final research-only helper streams shape validation one record at a time,
  retaining zero-sized record counts, then performs the original typed decode,
  configuration validation and authoritative-state validation. The production
  public importer is unchanged. Each restore also checks all canonical input
  bytes against a full checkpoint round trip. Both compiled arms pass a valid
  round trip and eight malformed-shape rejection controls.
- Rebuilt both arms consistently in `results-selected`, reran the 500 counts,
  and compared that full 1000-tick endpoint/driver against the retained original
  public-importer continuation. The complete warmups for 500, uncapped 5k and
  capped 10k were reused with verified hashes and hard links. Bot and bounded
  control data keep their original binary pins. All later primary rows use the
  same final streaming-loader builds; counted host search remains original.
- The first final uncapped-5k count peaked at 713,528 KiB RSS (about 697 MiB).
  The final collector runs all remaining rows sequentially on CPU 2. External
  build caches were removed after verifying the copied binaries; no active
  world overlaps a test, build or evidence-selector process.
- Source review found a cap-schedule defect before publication: repeated dt
  addition first lags an exact-multiple event beyond the API's 1 ns tolerance
  at tick 98,477. The 70k capped-10k warmup/count extent is earlier, but the
  remaining capped-5k preparation could cross it. Stopped the first untallied
  capped-10k count after about 18 minutes, with no flushed count endpoint;
  retained its empty log/output and the completed earlier rows. The uncapped
  10k CPU-hour accounting is unaffected and that control is never restarted.
- Corrected only research cap-event timing to halfway before each tick. Ten
  focused Python checks pass, including a reproducer rejecting the old plan
  at tick 98,477 and a full warmup/count-extent timing check for the new plan.
  A fresh pair is built in `results-final`, and all primary counts are rerun.
  Reuse verifies that the earlier Rust template differs only in event timing
  and its plan-export helper. The capped warmup's old and new event indices
  must agree on every tick through 71,100. Only the canonical radius-event
  array is replaced; the remaining checkpoint bytes are streamed unchanged,
  their suffix digest retained, and the donor hard link is preserved. The
  generated new plan is checked through 241,100 ticks. Other warmups are
  unchanged; transitive bot/original-importer pins remain their actual origins.
- The corrected capped-10k original-search window completed after 2498.99
  seconds of descriptive wall time. Before inspecting its instruction totals,
  amended repetition method to retain one full original-search reference per
  remaining N and use exact admission preparation outside the step counter
  for repeated physics. The next redundant original run was stopped after
  about 3.5 minutes, before flushing an endpoint; its partial folder/log remain.
  No uncapped-10k process was run and its CPU charge is unchanged.
- The full capped-10k reference has 133.788M step and 35173.612M original
  admission instructions/tick, 13404 deaths in seven simulated seconds, full
  10000 living population at step entry and zero refusals. This establishes
  baseline admission cost directly; acceleration is only research preparation.
  A fresh pair supplies every repeated physics row, with full reference/driver
  endpoint and every-tick population/lifecycle equality required at all N.
- Both SSH and remote execution then returned `machine_cooldown`, until
  2026-10-28T02:17:52Z. No measurement was active at that point. Retain the
  complete original reference and wait without restarting or changing settings.
- Final collection completed all six primary cases and 42 repeated physics /
  diagnostic windows, then passed all 11 Python guards and the full evidence
  selector. Every original-reference full checkpoint/driver and per-tick
  population/lifecycle census agrees with its repeated physics trajectory.
  All length screens pass; no counted admission is refused. The final child
  peak RSS is 1,119,656 KiB. Both new compiler targets were removed before
  collection, after executable hash checks. The table renderer runs directly
  from the validated selected JSON; it never constructs or advances a world.

Retained raw roots (outside Git): the WSL preparation ledger recorded
`<wsl-artifacts>/population-scaling-20261027-{selected,final,v2,v3,bounded}`
and `<wsl-artifacts>/population-scaling-pilot500`. These are not revisited
after the maintaining agent's stop on the owner's Windows workstation.
Final primary VM collection lives at
`<vm-root>/results-baseline`. The original
`results` and superseded `results-memory-bounded`, `results-selected` and
`results-final` siblings remain retained.
Copied binaries, archived sources, complete JSON rows/checkpoints and logs
remain outside Git. The bounded control keeps every reached tick, including
initial admission; a preparatory warmup without a flushed first block has no
invented endpoint.

## Publication correction

Private locations are published as `<repo>`, `<wsl-artifacts>` and `<vm-root>`,
preserving relative structure and original evidence hashes. Kernel metadata
retains only the numeric release. The selector applies this publication view
after validating the original retained data, without modifying raw evidence.
The CPU-hour cap, unsettled label and Windows/WSL stop were instructions from
the maintaining agent; the later Windows CPU and memory budget came from the
owner. The bounded-probe status string now credits the maintaining agent.
Its historical source pin is verified by reconstructing only that exact wording
edit; the published source hash is recorded separately. No simulation is rerun.
