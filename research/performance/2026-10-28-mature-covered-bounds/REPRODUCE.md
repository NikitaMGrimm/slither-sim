# Reproduce individual cohorts

These Linux x86-64 helpers require existing Rust 1.99, locked cached dependencies,
Python 3.12+, gcc, binutils and permission to count the current thread's user
instructions. They install nothing and change no system settings. Run heavy work
on the authorized guest. Use a fresh output directory without spaces; preserve
failed runs. Build sequentially with one bounded target. Do not run builds or
other active jobs during measurement windows. Raw traces and full process
inventories stay outside Git. The shared inventory helper redacts the account
before writing command lines.

From this repository checkout, create archives and copy the frozen helpers into
an external output directory (`study` below). Copy the archives/helpers to the
guest before running the remaining commands if the checkout is elsewhere:

```sh
study=/var/tmp/slither-mature-reproduction
mkdir -p "$study/shared" "$study/opt"
git archive d127acd1f0e9db2100c8a1dd5c2fa43e2deff506 > "$study/base.tar"
git archive 9c3b29da359586341af83f0fa45aef9f63b30967 > "$study/candidate.tar"
git archive 6e02fefefb447991d24cf0c3385d5eb8ac6c62ff > "$study/opt/base.tar"
git archive 50220958c900782d53cb225949bf25165709c371 > "$study/opt/candidate.tar"
cp research/performance/2026-10-28-mature-covered-bounds/*.py "$study/"
cp research/performance/2026-10-28-mature-covered-bounds/fixture.rs "$study/"
cp research/performance/2026-10-28-mature-covered-bounds/libm.c "$study/"
cp research/performance/2026-10-28-broadcast-distance/reproduce.py "$study/shared/"
cp research/performance/2026-10-28-broadcast-distance/counter.rs "$study/shared/"
cp "$study/"*.py "$study/fixture.rs" "$study/libm.c" "$study/opt/"
ln -s ../shared "$study/opt/shared"
mkdir "$study/target"
ln -s ../target "$study/opt/target"
```

Build only the arms needed for the chosen cohort; each build copies its binaries
before another build reuses the target. `build.py` records source/archive/binary
hashes. It reconstructs the fixture's historical terminal CRLF explicitly, while
the committed helper uses LF. The measured production change is already in the
candidate archive; the harness and counters are overlaid identically.

```sh
cd "$study"
python3 build.py base
python3 build.py candidate
python3 build.py base --profile
python3 build.py candidate --profile
cd "$study/opt"
python3 build.py base
python3 build.py candidate
python3 build.py candidate --profile
python3 build.py mutation
python3 check-index.py
python3 verify-parity.py
python3 short-pairs.py
```

`check-index.py` uses a pristine candidate archive for debug/release oracle,
narrowed-margin mutation/restoration, then the complete offline Rust group.
`verify-parity.py` compares all 27 original traces and two output mutation controls.
`short-pairs.py` counts the four original short rows after those gates.

Run one cohort at a time; `measure.py` waits for all its warmers before queuing
sequential requests. It preserves logs and stops on any error. Pair mode compares
all warm bytes, two child windows per arm, direct parent windows, one discarded
pair and five alternating pairs. Each child opens a fresh per-thread counter.

```sh
cd "$study"
python3 measure.py saving249-stock base candidate
python3 measure.py saving249-legacy base candidate --legacy 1
python3 measure.py matched-short249 base candidate --warm 0 --ticks 1080
cd "$study/opt"
python3 measure.py index-stock base candidate
python3 measure.py index-legacy base candidate --legacy 1
python3 measure.py matched-short-index base candidate --warm 0 --ticks 1080
```

For a cheap controller/fork smoke check, use a fresh session with `--warm 120
--ticks 2`. This final controller passed that pilot; its counts are not study
results. The original first stock #249 controller used a third baseline child
instead of direct baseline continuation; that retained exception and the
interrupted legacy attempt are described in ATTEMPTS.md.

Diagnostic windows use plain-reference byte comparison and separate counters;
do not report them as plain savings:

```sh
cd "$study"
python3 measure.py phases-stock base-profile candidate-profile --diagnostic
python3 measure.py phases-short base-profile candidate-profile --diagnostic --warm 0 --ticks 1080
gcc -O2 -Werror=attributes -fPIC -shared -o libstudy.so libm.c -ldl
python3 measure.py libm-stock base-profile candidate-profile --diagnostic --preload
python3 attribute.py "$study" libm-stock
python3 measure.py libm-short base-profile candidate-profile --diagnostic --preload --warm 0 --ticks 1080
python3 attribute.py "$study" libm-short
python3 measure.py libm-plain-stock base candidate --diagnostic --preload
python3 attribute.py "$study" libm-plain-stock --plain
cd "$study/opt"
python3 measure.py index-profile candidate-profile --diagnostic
python3 measure.py mutation-stock mutation --diagnostic
python3 measure.py mutation-legacy mutation --diagnostic --legacy 1
```

Use `compare.same_bytes` on every diagnostic `*-window.trace` and the matching
plain `proof-*-0.trace`. For mutation, require differing complete trace bytes
but identical two tag-0 canonical core-state records (the binary framing is
tag:u64, length:u64, payload). The mutation is enabled only in the measured
window; all physical warmup/history is unchanged. The retained `index-stages.py`
and `recover-stages.py` contain these exact acceptance checks. Compare plain and
tagged libm call counts by function exactly, and corrected instruction totals
within the predeclared 20 ppm gate. Calibrations/probes and all nine dynamic
imports must qualify. Inject `LD_PRELOAD` only into the target, never `taskset`.

The two bit-flip controls run `serve.py` with `--warm 120 --ticks 1
--perturb-stream` and stock/protocol-13 settings; both must fail at offset zero,
retaining both differing chunks. They are deliberate failures, not measurements.

`reconstruct.py OUTPUT_ROOT` recreates every generated Rust source and checks
the measured hashes without building. Run for both root and opt. The original
historical wave scripts and retry scripts remain audit records; use the
individual-cohort controller for fresh runs. `select_results.py` selects the
specific retained historical layout, including retries and its final CPU receipt;
it is not a generic collector for arbitrary session names.

Before cleanup, confirm no task workers remain and preserve all copied binaries,
sources, window traces, counters, inventories and failed logs. Verify the actual
target resolves within the chosen output root, unlink opt/target, then remove
only that bounded target. Do not delete raw evidence.
