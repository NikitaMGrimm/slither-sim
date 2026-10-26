# Reproduce the comparison

Linux/WSL2 prerequisites: Rust 1.99.0, Python 3.11+, `patch`, `taskset`,
`perf`, cached Cargo dependencies and these commits in the local Git repository.
No outside game server is contacted. Use a fresh scratch directory with enough
space for two release targets and complete traces. The commands assume logical
CPUs 0/2/4/6 are available; adapt the four-CPU budget explicitly on another host.

From the repository root, set `out` to a new absolute directory and `perf` to
the available userspace perf executable. A task-local perf build may also need
its library directory in `LD_LIBRARY_PATH`. No administrator or security change
is required by this protocol.

```sh
set -eu
export RUSTUP_TOOLCHAIN=1.99.0 CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=2 RUST_TEST_THREADS=2
out=/tmp/host-prey-new-run
perf=/path/to/perf
mkdir "$out"
mkdir "$out/harness"
prior=research/performance/2026-10-26-host-known-slots
for name in loopback.py paired.py trace_compare.py instructions.py host-players.patch battledome-catalog.patch; do
    git show "489a6b0edfe0ff3891a7a04ce5fbfe7c4308b726:$prior/$name" > "$out/harness/$name"
done
for arm in base candidate; do
    pin=249cdb217d83a234895bc7eee4d98c3636c1cd76
    if [ "$arm" = candidate ]; then pin=fde812bb4c5c6dea52ebb32a8341d98fe45ea76b; fi
    mkdir -p "$out/$arm/source" "$out/$arm/bin"
    git archive "$pin" > "$out/$arm/source.tar"
    tar -xf "$out/$arm/source.tar" -C "$out/$arm/source"
    capacity="$out/$arm/source/research/performance/2026-10-26-host-capacity"
    cp "$capacity/host_probe.rs" "$capacity/host_capacity_load.rs" "$out/$arm/source/examples/"
    patch -o "$out/$arm/source/examples/host_players.rs" "$capacity/host_capacity_load.rs" "$out/harness/host-players.patch"
    patch -o "$out/$arm/source/examples/perf_battledome.rs" "$out/$arm/source/examples/perf_baseline.rs" "$out/harness/battledome-catalog.patch"
    (
        cd "$out/$arm/source"
        CARGO_TARGET_DIR="$out/$arm/target" CARGO_PROFILE_RELEASE_DEBUG=1 taskset -c 0,2 cargo build --locked --release --features networking \
            --example host_probe --example host_capacity_load --example host_players \
            --example perf_baseline --example perf_battledome --example perf_client_retained
    ) > "$out/$arm/build.log" 2>&1
    for name in host_probe host_capacity_load host_players perf_baseline perf_battledome perf_client_retained; do
        cp "$out/$arm/target/release/examples/$name" "$out/$arm/bin/$name"
    done
    sha256sum "$out/$arm/bin/"* > "$out/$arm/binary-sha256"
done
taskset -c 0,2,4,6 python3 "$out/harness/trace_compare.py" "$out/base/bin" "$out/candidate/bin" "$out/traces"
taskset -c 0,2,4,6 python3 "$out/harness/trace_compare.py" "$out/base/bin" "$out/candidate/bin" "$out/traces-dome" --battledome
```

Finish checks before timing. The shared harness rejects existing output
directories, failed children and unequal complete decompressed traces. For each
workload below, run pairs first, then the two separate profiles. The harness
pins every host thread to CPU 0 and three generators to CPUs 2/4/6. Each `paired`
invocation includes the discarded pair and all five alternating measured pairs.

```sh
taskset -c 0,2,4,6 python3 "$out/harness/paired.py" "$out/base/bin" "$out/candidate/bin" "$out/pairs-official-500"
for arm in base candidate; do
    taskset -c 0,2,4,6 python3 "$out/harness/loopback.py" "$out/$arm/bin/host_probe" "$out/base/bin/host_capacity_load" "$out/profile-official-500-$arm" --profile "$perf"
done
for count in 40 64 80; do
    taskset -c 0,2,4,6 python3 "$out/harness/paired.py" "$out/base/bin" "$out/candidate/bin" "$out/pairs-dome-$count" --load "$out/base/bin/host_players" --count "$count" --preset battledome
    for arm in base candidate; do
        taskset -c 0,2,4,6 python3 "$out/harness/loopback.py" "$out/$arm/bin/host_probe" "$out/base/bin/host_players" "$out/profile-dome-$count-$arm" --profile "$perf" --count "$count" --preset battledome
    done
done
taskset -c 0,2,4,6 python3 "$out/harness/instructions.py" "$out/base/bin" "$out/candidate/bin" "$out/instructions" --perf "$perf"
```

Keep full outputs in scratch. Use the baseline capacity study's `analyze.py`
for receipt excess and `profile.py` for categories after Rust demangling and
removal of symbol crate hashes. Report arm medians, every paired ratio/range,
alive/waiting populations, total deferrals and wait quantiles with censored
intervals separate. A rerun is new descriptive evidence, not a native capacity
qualification or a replacement for the retained selected results.

For the supplemental integrated profiles, extract
`bf5e29c1928d6edb69399ab18e7d3dd242e8b6c9` into a fresh source root. Copy only
the capacity study's `host_probe.rs` into `examples/` and build `host_probe`
with the same locked release flags above; no catalog overlays are needed.
Repeat the four `loopback.py` profile calls with this copied probe and the same
baseline-built generators, using fresh outputs. These single-arm profiles
update attribution only; they do not replace the frozen paired comparison.

The rejected grid is retained as [a patch against the same baseline](rejected-grid.patch).
Apply it with `git apply` to a fresh baseline source root to rebuild that attempt;
its full timings/profiles remain local-only. It is outside the proposed runtime.
