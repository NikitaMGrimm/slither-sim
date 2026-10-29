# Initial integrated baseline

Merged main after #274 is `9ad002c1819eecac97b91410321bc611b4d6dde0`.
The plan was committed as `b409578e` before any new collection. Fresh
attribution uses this exact runtime, with compact occupied snake histories and
the previously landed oldest-sector skip. No new layout is implemented before
that attribution is reduced and its candidate selection recorded.

The diagnostic reuses #269's instrumentation/build/driver/symbolization by
reference. Its only adaptations are compact-history footprint accessors, the
predeclared cycles/dTLB/L1d event/repetition subset, the already corrected L1d
sampling period and current runtime/build pins. One task-owned Cargo target
serves all stages: `/tmp/slither-retained-layout-target`.
