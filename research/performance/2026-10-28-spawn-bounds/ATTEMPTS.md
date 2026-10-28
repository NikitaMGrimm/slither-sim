# Retained attempts

- First focused compile: the helper method needed `pub(super)` visibility.
  Corrected before the oracle ran.
- First completed focused oracle: long/crossing and unusual-coordinate tests
  passed; lifecycle fixture supplied no action for a living snake and the
  step rejected it. Corrected the fixture to send an explicit action.
- First Rust check-group attempt passed lint, then integration compilation
  failed because the bench's shell `rustc` launcher discarded the hyphenated
  `CARGO_BIN_EXE_slither-sim` environment variable. A direct focused transport
  compile (`cargo test --offline --locked --test transport -v`) reproduced it.
  Use `RUSTC="$(rustup which rustc)"` and the equivalent
  `RUSTDOC` setting, without changing installed tools or system settings.
- Initial build copied distinct binaries, but the count-window amendment's
  shared Cargo target reused the baseline for the candidate (equal binary
  hashes and no candidate compilation). Rejected before any measurements.
  Final builds use separate targets and explicitly clean/recompile the root
  package; dependency seeds alone may be reused. Initial and rejected build
  logs/pins remain in the raw archive and support no result.

All artifacts for these attempts remain outside Git. No failed or censored
measurement is relabeled as a completed sample.
