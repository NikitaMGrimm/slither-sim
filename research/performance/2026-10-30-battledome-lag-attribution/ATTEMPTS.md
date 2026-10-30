# Attempts

- Maintainer accepted #286 and requested fresh attribution of temporary host lag.
  New branch from current main; #286 remains untouched. Verified the explicit
  limited IAM profile before any new cloud work. Freeze observation, calibration,
  primary windows, episode definitions and cap before timing. No fix is included.
- Build and non-primary smoke passed. Linux scheduler counters were initially
  disabled and were enabled in this disposable guest. Full scheduler/IRQ tracing
  is available; both thread CPU/wait counters are nonzero. Six reduction tests
  pass, including trace loss and incomplete-record rejection. A file-handle
  warning in the synthetic loss test was corrected without changing any values.
- The all-IRQ trace failed its predeclared overhead gate: +7.36%, +0.2693 ms
  host CPU/tick versus LIGHT. Original primary windows therefore use LIGHT.
  A prospective scheduler-only diagnostic series is frozen in PLAN before it
  runs, with the same overhead gate. It does not replace original primaries.
- A durability pause helper was staged in anticipation of large full-IRQ primary
  captures. The overhead gate selected LIGHT, so it was stopped before any
  checkpoint pause occurred. No counted window was suspended or changed.
