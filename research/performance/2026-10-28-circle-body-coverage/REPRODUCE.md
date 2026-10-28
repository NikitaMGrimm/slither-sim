# Reproduce the view-circle rejection study

Use the preceding [mature fixture recipe](../2026-10-28-mature-covered-bounds/REPRODUCE.md)
for the Linux-only counter requirements and full trace framing. No packages,
limits or system settings change. All heavy work runs on the authorized guest.
Use a fresh external output directory without spaces and one bounded Cargo
target. Keep failed runs, raw traces, sources, copied binaries and redacted full
process inventories outside Git. Do not run other active work during counts.

The runtime baseline is merged #252 main
`85e745096e533143dedfb6c1948b697781e697e9`; the selected circle candidate is
`5904e1f9` (the full archive commit and hash are in results.json).
Create `base.tar` and `candidate.tar` with `git archive --format=tar --output=...`
at those revisions before copying them to the guest.

Copy this study's `build.py` beside those archives. Copy `fixture.rs` from the
preceding mature-bounds study and `reproduce.py` / `counter.rs` from
`2026-10-28-broadcast-distance` into an output `shared/` directory. Copy the prior
study's `serve.py`, `measure.py`, `queue249.py`, `accept249.py`, `compare.py`,
`verify-parity.py` and `short-pairs.py` alongside the new build helper. Copy this
study's `check.py` there too. All are standard-library Python helpers.

Build sequentially, preserving each copied binary before reusing the target:

```sh
python3 build.py base
python3 build.py candidate
python3 build.py base --profile
python3 build.py candidate --profile
python3 build.py mutation
python3 check.py
python3 verify-parity.py
python3 short-pairs.py
```

`build.py` overlays the identical fixture/counters on both runtimes. It supports
the five-argument coverage predicate, brackets body-bound construction
as scope 13, whole-body broadcast bounds as 14, and coverage-grid maintenance as
15. The older phase scopes remain unchanged. Diagnostic builds retain the same
eligibility-vector overhead on both arms. The canonical LF fixture is copied
with one terminal CRLF to match the frozen measured source payload.

`check.py` uses a pristine candidate archive for the original coverage oracle,
the new diagonal endpoint cases, debug/release runs, deliberately narrowed block
margin and circle-radius mutations, restoration, and the full offline
Rust group. `verify-parity.py` compares all 27 original traces and stock/protocol-13
output mutation controls. Run all gates before the five plain pairs.

The unchanged mature scene warms the complete host from tick zero to 193,000,
including all recipients and natural food-presentation history. It never restores
a checkpoint. The unchanged length screen must qualify. Each cohort compares
complete warm bytes live, then repeated fork windows and direct parent references.
It discards one warmup pair and counts five AB/BA/AB/BA/AB pairs over 360 ticks.

```sh
python3 measure.py stock base candidate
python3 measure.py legacy base candidate --legacy 1
python3 measure.py short base candidate --warm 0 --ticks 1080
python3 measure.py phases base-profile candidate-profile --diagnostic
python3 measure.py mutation-stock mutation --diagnostic
python3 measure.py mutation-legacy mutation --diagnostic --legacy 1
```

Require complete phase-window bytes to match the corresponding plain reference.
For both mature output mutations, require differing complete trace bytes but
identical tag-0 canonical physical states. The radius perturbation starts only
inside the measured window. The same warm-stream bit-flip controls as the prior
study must fail at offset zero with both differing chunks retained.

For source reconstruction without compilation or another warmup:

```sh
python3 PATH_TO_THIS_STUDY/reconstruct.py OUTPUT_ROOT PATH_TO_PRIOR_STUDY/fixture.rs
```

Every generated Rust hash must match its measured receipt. The public recipe can
differ from the original Python wrapper while reproducing exactly the same Rust
payload; driver hashes identify the historical wrappers separately.

## Exploratory selection and instruction-IP diagnosis

The circle pilot adds a conservative circle-to-block rejection after
the existing square rejection; `circle.patch` preserves it against the baseline.
Its complete measured-window bytes and fork/parent references must match the
baseline. Its warm stream was not recorded, and its single exploratory count is
not a qualified five-pair performance claim. Both candidates use the same mature
window for selection; the chosen circle change then receives a fresh complete evidence suite.
`cache.patch` retains the nonselected verified-coordinate cache prototype, its
edit oracle and all production changes against the same baseline.

The optional sampler uses the retained #252 phase runtime, whose runtime sources,
manifests, lockfile and build script match merged main. Compile `sample.c` using:

```sh
gcc -O2 -fPIC -shared -o libstudy.so sample.c -ldl
```

Use the prior `serve.py --preload` controller with one `base-profile` arm; inject
the library into the simulator only. `STUDY_SAMPLE_PERIOD=1000003` or `2000003`
is set before launching that parent. The shim records only host advance scope 2,
opens fresh current-thread events at each measured window, and uses a 512 KiB
ring. It rejects loss, throttling, ring overflow, incomplete running coverage and
a sampled-instruction-weight discrepancy exceeding 2%. Probe a short window
before paying for mature warmup. Do not change limits if allocation fails.

For repeated fork samples, preserve `base-profile-parent.csv.samples` and `.maps`
under each `sample-N` label before sending the next request; direct parent output
must finish before reading its files. Every sampled window must match the plain
reference completely. `select-samples.py OUTPUT_ROOT SESSION` resolves inline
source frames from the PIE image base. Raw maps and addresses stay private;
selected locations are repository-relative. Sampling/skid and the coarse period
make these diagnostic estimates; plain counters establish savings.

After all checks, verify that no task worker remains, resolve the actual target
within the intended output root, unlink only its task-owned target symlinks, and
remove that one bounded target. Keep the copied measurement binaries and evidence.

`select-results.py` selects this retained multi-cohort layout (`circle-selected`,
`selected` for the cache pilot, and `sampling-retry`), including failed-attempt,
reconstruction and cleanup receipts. It is not a generic fresh-cohort collector.
The individual reproduction commands produce their own accepted.json records.
Applying cache.patch to the pinned baseline recreates the exact prototype Git tree.
