# Attempts

- Implemented an exact interval cache, with the uncached public rasterizer
  retained as the reference. The mask does not retain snake cells or wire bases.
- First growth/shrink fixture passed an empty action batch for one slot and
  failed `ActionShape`; then a null action for a living slot failed `Missing`.
  Corrected the test to supply a legal live action. Neither failure supported
  a parity or timing claim.
- Strict Clippy rejected a mutable `from_world` method's naming convention.
  Renamed it `render_world`; no lint was suppressed.
- Corrected fixture, threshold/adjacent-float tests, 17 Battledome presentation
  tests, four protocol-13 tests and default all-target strict Clippy passed
  locally with one job/thread. Deliberately changing the exclusive outside
  threshold to inclusive failed the oracle (exit 101); source was restored in
  a finally block and all 398 active library tests passed (one pre-existing
  ignored test). Full receipts are retained externally. Full trace matrix and
  PMU evidence remain pending.
- No SSH bench use: the maintaining agent has not released it.
