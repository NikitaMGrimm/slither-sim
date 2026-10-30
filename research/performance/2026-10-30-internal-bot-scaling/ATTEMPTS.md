# Attempts

- Preparation: source inspection confirms `observe_all_into` can use the current
  Rayon pool, while `step_profiled` supplies opt-in wall-clock phase brackets.
  No measurements are claimed yet.
- First study-harness build failed before execution: three Rust float literals
  lacked leading zeroes, and sha2 0.11's digest array does not implement
  `LowerHex`. Corrected the literals and used explicit per-byte hex formatting;
  retained the failed build log. No runtime source was changed.
- Unselected smoke: 1,000 newborns, 12-tick warmup and 24-tick serial/parallel/
  simulation-replay continuations. Full serial/parallel action digests and all
  three final-state digests agree. These cold newborn costs are preparation,
  not stabilized primary measurements. All four stability-screen tests pass.
- Primary 1k, 2.5k and 5k counting completed; every counted/profiled final state
  matched its verification continuation. The original SSH-launched controller
  and 10k warmup child then disappeared without a captured terminal status.
  The partial 10k log ends at 2,000 ticks (14 virtual seconds); no checkpoint or
  qualification receipt was written. Its cause is not established. The
  post-count and archive controllers refused to proceed on the missing
  completion event. Their failure logs and the partial census are retained.
- Restart only the checkpoint-less 10k cohort from the same seed, unchanged
  frozen controller and counted executable, under a detached controller.
  `restart_primary.py` retains the interrupted directory and pins the retry;
  the three completed populations are untouched. No partial 10k measurements
  are selected, and none of the stability or timing definitions change.
- Before diagnostic execution, synthetic parser tests found that the proposed
  stack classifier handled legacy symbols only. It now handles demangled
  symbols too and fails if it recognizes no tick samples. This was a tooling
  correction before sampled results, not a replacement measurement.
- During 10k warmup, the maintainer requested bounded completion rather than
  continued warmup retries, slow-tick attribution at 10k and botstorm 12k, and
  optional botstorm 6k/9k only if 12k misses real time. AMENDMENT.md freezes
  those changes before remaining timing. The unused extra-warmup proposal is
  superseded. Sleeping post-count/archive controllers were replaced; the live
  native warmup and all completed rows were untouched. Their earlier waiting
  logs remain in the private archive.
- A 0.3-second, unselected user-CPU format probe confirmed perf permissions,
  the monotonic clock option and timestamp/period parsing before diagnostic
  collection. The expected timeout ends its test workload; its data and logs
  are retained. Perf script now explicitly requests `+srcline,+period` instead
  of relying on version-specific default fields. No study timing is selected
  from that probe.
- Before any diagnostic timing, the maintainer requested all pickup-grid guards
  and the chosen path. Source inspection showed the existing event recorder
  permits exact post-step reconstruction of pickup-entry food/circles. The
  already planned diagnostic copies now record it outside world brackets,
  preserve primary-state parity and compare all three guard traces. No source
  hook, production edit or extra timing run is introduced. The original
  checkpoint-only census remains separately labelled.
- The detached 10k retry reached its original guard at 34,000 ticks: 238 virtual
  seconds, 7,378.16 wall seconds including the census-boundary overshoot and
  checkpoint work. All 10,000 slots were alive; mean body length was 51.3174,
  p90 73, p99 87, food 515,400. It did not pass the original ecology screen.
  Under the maintainer amendment, that final checkpoint proceeds to verification
  and the normal counted windows, labelled unsettled. No warmup extension ran.
- During baseline 10k counting, the maintainer requested a separate post-fix
  replay using external #331. POSTFIX_PLAN.md pins head `a6511aad` before its
  timing. Local source inspection confirms its simulator source differs only
  in the grid-cap constant plus comments/tests; Cargo's additional changes
  import the client workspace. The supplement verifies the selected simulator
  dependency tree/profile before building. It waits for baseline attribution,
  and the archive finisher waits for it. The baseline run is untouched.
- All guarded 10k windows and both phase diagnostics completed with the original
  reference world-state digest. The initial A4 reduction verifies fixtures,
  serial/parallel action tapes, census and every counted final state. A small
  Windows table preview without UTF-8 mode displayed replacement characters;
  source/result files were intact. Use `python -X utf8` for that preview/check.
- Stop-handshake deviation, identified from the retained receipts: the qualified
  A4 warmups first passed at 86k/86k/90k ticks but saved at 87k/87k/91k.
  The parent requests stop after reading the census; the native loop checks
  asynchronously at 1,000-tick boundaries. Each saved state is seven virtual
  seconds later than the first qualifying census. These states are retained
  as run, without performance-based reselection. The original plan's literal
  first-qualifying-checkpoint wording was not met; both ticks remain public.
