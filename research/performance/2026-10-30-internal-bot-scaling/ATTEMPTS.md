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
