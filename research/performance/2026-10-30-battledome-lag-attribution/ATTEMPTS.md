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
- After collection, add an explicitly exploratory client diagnostic around
  adjacent ticks with no pacing wait, with a one-tick margin. It does not replace
  or relax the frozen >20-ms episode definition. The ordinary full reducer and
  the bounded augmentation path call the same function; scheduler events need
  not be decoded again to append this diagnostic.

- Both series completed with all generator gates passing. No Spot interruption
  or rejected generator occurred. Full final-row re-reduction equals the
  augmented cached result. Seven documentation checks and exact table matching
  pass remotely. All required archives were retrieved/verified before teardown;
  the 17-enabled-region audit is clean and the task key/group are deleted.
- Post-publication CodeQL identified 13 path flows guarded by Python assertions.
  Those guards disappear with `python -O`. Replace them with explicit validation
  before trace mutation; two short offline boundary tests pass normally and under
  `-O` (under one second each locally). No native/reduction work ran locally.
  Preserve every measured archive and original source pin; this corrects the
  future controller only and does not re-label the measured helper version.
