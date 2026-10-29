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
- All 23 catalog rows and four historical host fixtures passed full-byte
  comparison and terminal checks. The first supplementary 16-seat Battledome
  fixture refused initial slot 14 before tracing: simultaneous ring placement
  was an invalid setup assumption, not an output mismatch. Retained the failed
  source/binary/stderr and switched only the supplementary cases to eight seats
  in a fresh external directory. No successful historical trace was discarded.
- The first eight-seat preparation built successfully but its copy step still
  used the default target after an explicit target override. Corrected the
  helper to copy from the actual Cargo target and started another fresh
  directory. The failed preparation and log remain intact; no unpinned binary
  was accepted as trace evidence.

- The fresh eight-seat stock and protocol-13 traces are byte-identical through
  growth/shrink, deaths and forced reconnects. The projected-radius *0.98
  mutation changes encoded wire bytes in both. The selected correctness receipt
  retains all 23 catalog, four historical host and two supplementary traces,
  focused mutation results and source/archive/binary identities. These debug
  checks establish their stated parity scope, not an instruction saving.
- The documentation test rejected the new report before its index row existed.
  Added the required row with explicit pending-performance status; no completed
  speedup or capacity claim was substituted.
