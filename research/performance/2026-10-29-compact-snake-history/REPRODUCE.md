# Reproduce compact occupied histories

## Post-result integrated adoption checks

The maintaining agent adopts the measured implementation while preserving the
original study's rejected verdict and every timing observation. For the
integrated checks, archive current main as `base.tar` and the adopted PR head as
`candidate.tar` in a fresh external directory, apply `harness.patch` there and
copy [adoption.py](adoption.py) alongside it. Run `python3 adoption.py` on the
guest with the same retained import paths (or bind them as described below).
This repeats the focused oracle, all three runtime mutations and restoration,
the full Rust group, 27 traces, wire mutation controls and documentation tests.
It then compares stock complete warm streams through tick 193,000 and six
144-tick continuations, spanning two food-maintenance cycles. It removes the
timing request path from its proof-validator copy. No timing series is rerun.

## Original measured study

Use existing Rust 1.99, Python 3.11.4 or later, a C compiler, binutils, locked
offline dependencies and permission for current-thread user PMU counters. No
installation, system change, affinity or outside game server is needed. All
builds, checks, traces, measurements and reductions for this study run on the
authorized SSH guest. Keep one bounded Cargo target and preserve failed runs.

The frozen final arms are base `e860ec237f172de5df34caff86a9eb26082e82d6`
and candidate `d781ff000d2dfee10627ab1b8a4c07bfff3638c0`. Archive each with
`git archive REV > ARM.tar` in a fresh external execution directory. Apply
`harness.patch` there; its files are execution helpers, not production changes.
The candidate's production change and oracle are already in its archive.
The original research-only delivery retained the measured code and oracle in
`rejected-bounded.patch`, now the [archived measured patch](https://github.com/NikitaMGrimm/slither-sim/blob/76772d07098c884e9643783a0ba874bce0567e5e/research/performance/2026-10-29-compact-snake-history/measured-bounded.patch).
From the repository root, retrieve its exact historical bytes into the external
execution directory:

```sh
git show 76772d07098c884e9643783a0ba874bce0567e5e:research/performance/2026-10-29-compact-snake-history/measured-bounded.patch > /path/to/external-output/measured-bounded.patch
```

It reconstructs that code and oracle on the base. Apply either pilot patch
on top of that bounded implementation to recover its container, reusing the
same oracle. Reverse the pilot patch before switching to the other layout.
Use `git apply --unidiff-zero` for the bounded patch and its reverse; its
zero-context representation retains the full change. The final patch check
compares both reconstructed source files byte-for-byte with the candidate
archive and verifies that reversing each attempt returns to the pristine base.

Reuse, without copying into this study:

- [Full backend fixture](../2026-10-28-mature-covered-bounds/fixture.rs), whose
  warmup, policy, admission and lifecycle recipe remain unchanged at 500 seats.
- [Stream controller](../2026-10-28-mature-covered-bounds/serve.py),
  [comparer](../2026-10-28-mature-covered-bounds/compare.py) and
  [acceptance helpers](../2026-10-28-mature-covered-bounds/accept249.py).
- [Build/trace/inventory helpers](../2026-10-28-broadcast-distance/reproduce.py)
  and [counter](../2026-10-28-broadcast-distance/counter.rs).

Reconstruct the imports in an external directory: put `reproduce.py` and
`counter.rs` together in `imports/bounds/shared/`; put the same shared files in
`imports/maintenance/shared/` and the three stream helpers in
`imports/maintenance/`. Before execution, bind `prior` in `build.py` to
`r/'imports/bounds/shared'` and `prior` in `setup.py` to
`r/'imports/maintenance'`. The retained scripts name their original guest
locations. These two import-location substitutions do not change the Rust
overlay. Keep paths free of spaces. The builder extracts the fixture from each
source archive and overlays identical measurement code on both arms.

```sh
python3 compact-current-driver.py
python3 compact-stream-control.py
```

The driver runs `pipeline.py`, verifies the copied binaries, deletes the Cargo
target and records `target-cleanup.json`, which the timing stage requires. It
then runs `compact-start-mature.py`, `compact-proof-mature.py` and
`compact-proof-short.py`.

`pipeline.py` first runs `check.py`: focused snake tests, then three separate
mutations (wrong removal, lost replacement, reversed enumeration), each required
to fail at runtime with `test result: FAILED`. A `finally` restores the pristine
source after each mutation. The restored focused tests must pass, followed by
`python3 tools/check.py rust`. It writes the actual commands, expected outcomes
and exits to `checks/results.json`, then writes `required-checks-pass.json`
directly from those rows, archive digest and the byte-checked source restore.
No earlier study's check runner or manually asserted receipt supplies this gate.

The pipeline builds base/candidate/radius-mutation binaries with one target,
checks 27 complete catalog/host traces, and requires the radius perturbation to
change both complete traces and tag-2 host-frame payloads for stock and protocol
13. The full-stream fixture instead uses tag 3 for wire payloads. The separate
`compact-stream-control.py` deliberately flips a stream byte and requires the
complete warm-stream comparer to fail; it retains the differing chunks.

The full physical checkpoint lacks presentation lives, hidden food births,
held listings, sector interest and recipient snake histories. Warm the entire
backend once per arm/protocol, without sockets or wall pacing, and fork it for
every window. Compare **every warm byte** through tick 193,000 using bounded
buffers. Two child continuations per arm and both final direct-parent streams
must match completely, including action/state records, frame ordering, packet
bytes and terminal summaries. Short controls use 500 seats at tick zero and
1/10/50 seats warmed for 600 ticks, omitting only the 500-seat disconnect recipe.

Verify all copied binaries against `ARM/pins.json`, record the source/build
pins, and commit them before timing. Remove the sole task-owned target after
verification; `compact-current-driver.py` does this immediately after the
pipeline to leave room for warm proof streams. Its `target-cleanup.json` records
the check. That driver runs pipeline, mature startup, mature proofs and short
proofs without requesting timing. Run the stream-byte control separately before
timing. Keep the warmed parents waiting at their request barriers.

Notify the maintaining agent before timing, keep the guest otherwise idle and
write `primary-timing-notice.json` as `{"unix": UNIX_SECONDS}`. Wait at least
60 seconds, then run:

```sh
python3 compact-time-primary.py
python3 compact-select-primary.py
```

No builds, tests, reductions or guest status commands run during counted windows.
The controller records full process command lines at each boundary, redacting
the configured account in memory before writing. One discarded AB precedes five
AB/BA/AB/BA/AB pairs for each of the ten rows. Each child starts from the same
frozen parent state and advances 1,080 ticks. Direct-parent proofs run only after
all counted windows finish. Do not replace observations or retry for outcomes.

The four grouped user-only, current-thread events are instructions, cycles,
Linux L1d-read-miss and dTLB-read-miss selectors. Read values by event ID; require
at least 99% running/enabled for every interval. CPU brackets the advance, while
policy, admission preparation, frame draining and trace serialization remain
outside. Identical clock/counter overhead is included, without subtraction.
Actual food-epoch changes identify maintenance ticks; check every flag against
the source cadence. Keep both the discarded warmup and all five reported pairs.
The reducer emits their separate whole-window, ordinary and maintenance metrics
and applies the adoption gate verbatim.

The pilot and superseded-primary evidence keep their original revisions and
receipts; neither contributes a final-primary observation. Preserve raw sources,
binaries, traces, failed controls, inventories and counter rows outside Git.
Before pruning task-owned storage, archive it, verify archive members against
their source files, transfer it and compare SHA-256. The selected public data
and table calculator are sufficient to recompute the report's tables without
those local-only archives.

For historical delivery checks, create `final-delivery.tar` with `git archive`
of the original research-only checkout `e379ffd9`, which retains the original
patch filename, alongside the two measured-arm archives. Run
`python3 postchecks.py` from the helper patch in a fresh execution directory.
It runs the seven documentation tests, independently recomputes the public
tables and frozen gate, checks all three patch round trips and requires the
bounded patch's two source files to equal the measured candidate archive.
[postprocessing-checks.json](postprocessing-checks.json) records its exact
commands and exits. These checks do not rebuild or retime either arm.
