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
