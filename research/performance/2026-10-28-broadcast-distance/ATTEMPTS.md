# Retained attempts

- The first source-overlay attempt stopped before compilation: its food-interest
  anchor also matched the standalone-broadcast loop. Use the surrounding `now`
  assignment to identify the normal tick loop uniquely. The incomplete source
  and traceback remain outside Git.
- The first completed diagnostic put PMU boundaries around every candidate.
  It completed stock-500 phase and libm runs and the protocol-13 phase run.
  Before using percentages for selection, the excessive boundary overhead was
  identified and that pilot was stopped during its protocol-13 libm run.
  Sources, binaries, logs and completed/partial measurements remain in a pilot
  directory. It is not a plain speedup measurement or a final budget table.
- The revised diagnostic computes the same ordered, read-only coverage booleans
  in one batch per recipient, then processes each candidate in the original
  order. This bounds PMU reads to recipient phases. The temporary boolean vector
  and instrumentation overhead remain diagnostic costs. Plain paired binaries
  retain the original loop and have no phase overlay.
- The guest's data volume was almost full at discovery. No existing artifact
  was deleted; new sources, results and the one Cargo target use separate
  task-owned storage on a filesystem with approximately 11 GiB free.
- The initial callchain parser placed PERIOD after CALLCHAIN, following bit
  order rather than the kernel record layout. It rejected the first record
  before publishing results. The sample child was checked/stopped, partial
  files retained, and the parser corrected to IP, PERIOD, CALLCHAIN. The
  corrected sampler also cleans up children on any parser exception.
- Callchain samples recover the dominant `hypot` callers, but some libm code
  uses the frame-pointer register for other work, leaving 2.1-3.6% of total
  process instruction weight without a caller. Preserve this unknown; do not
  allocate it by nearest exported symbol. Add a calibrated common-bracket shim
  instead: identical measurement code for real calls and a verified one-ret
  control, stable for 64 repetitions. Keep all earlier uncalibrated counters.
- During candidate verification the SSH endpoint rejected new channels with a
  cooling-down deadline, then closed the existing connection. No connection
  routing, service or system setting was changed. Preserve the interrupted
  check logs and inspect state after the stated deadline before resuming.
- External IP samples and the internal current-thread counter disagree:
  stock-500's sampled event counts only about 45% of the internal outer count,
  despite complete PMU scheduling and no lost ring records. A pre-main
  stop/attach handshake does not fix it (about 38% in the retained pilot), so
  a missed startup window is not an established explanation. The cause remains
  unresolved. Neither these percentages nor inferred full-fixture budgets are
  accepted. All sampler sources, binaries, raw samples, windows and failures
  remain outside Git. No other process or system setting was changed.
- The accepted calibration additionally verifies three known 32-NOP-plus-ret
  probes as exactly 33 instructions, using the same common measurement paths.
  Plain and phase-tagged libm call counts agree exactly; their aggregate
  corrected instruction totals differ by under two parts per million. This
  current-thread method supplies the report's attribution independently of the
  rejected external sampler.
