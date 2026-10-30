# Reproduce the steering echo and frame-delay study

Read [PLAN.md](PLAN.md) and [ATTEMPTS.md](ATTEMPTS.md) first. All native builds,
probe/load runs, packet replay and reductions run on AWS, never the local PC or
shared SSH guest. Endpoints and cloud receipts remain private. Full raw archives
are local-only with no public download URL; published digests identify them but
do not grant access. The report distinguishes capture observations from inference.

## Pristine runtime and clients

Use `git archive 9cd6ba6922c0a63b525ba88b69f86c8558142596` as `main.tar` and
extract into a fresh remote `source/`. Copy [probe.rs](probe.rs) to
`source/examples/input_latency_probe.rs`. This is a research helper, excluded
from the shipped crate; neither Cargo manifests nor runtime source change.

Reuse #281's exact received-frame constant-boost `aws_clients.rs`, from its
verified local-only `build-sources.tar.gz` member
`arms/candidate/server/source/examples/aws_clients.rs`. Its digest is
`3e3e702536243790ee43047dfbebe3dc7cf7eac20fb31498e0bb0d6e4ce45ca5`.
Alternatively regenerate that example with the linked
[#263 overlay](../2026-10-29-battledome-boost/prepare.py),
[#262](../2026-10-29-battledome-experience/prepare.py) and
[#258](../2026-10-29-c7a-mature-capacity/prepare.py) helpers and their referenced
staging sources, then require the same digest. Copy **only that example** into
the pristine source. Do not copy their server instrumentation. The earlier
source archive and private staging helpers retain their documented access limits.

On the generator, Rust 1.99 with default portable release:

```sh
export CARGO_TARGET_DIR=/srv/slither-study/target
export CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_DEBUG=0 RUSTFLAGS=''
unset CARGO_ENCODED_RUSTFLAGS
cargo build --release --locked --bin slither-sim --example aws_clients --example input_latency_probe
cargo test --release --locked --example input_latency_probe
cargo test --release --locked --example aws_clients high_duty_preserves_heading_and_rule_gate -- --exact
```

Copy the three binaries to `ROOT/bin/{host,clients,probe}`, and the host binary
to the target's `ROOT/bin/host`. Hash all binaries and both example sources.
Compare every runtime `src/` file against `main.tar` and retain one full source
manifest outside Git, publishing its one digest. Preserve failed build logs.
[pin_build.py](pin_build.py), staged beside `source/`, `main.tar` and `bin/`, is
the producer of `pins.json` and `source-manifest.json`: run `python3 pin_build.py`
from that directory after the binaries are copied. It pins this study's runtime,
toolchain, native binaries and helper sources and verifies unchanged runtime source.

The probe command is:

```sh
input_latency_probe PRIVATE_IP PORT OUTPUT_DIRECTORY SECONDS NONZERO_SEED
```

It connects with the stock challenge and protocol-19 login, shared ClientState
decoding and A4 policy. The research boost override respects received rules.
Every frame is decoded. `START` enables randomized probe sends; `STOP` closes
the run and writes buffered records. `connection-N.json` announces each local
source port before the HTTP/WebSocket handshake. A capture controller starts
tcpdump for that flow, then creates `capture-ready-N`. No artificial sleep is
inserted into a measured steering echo; capture setup occurs before login.

## Collect without a server hook

Install tcpdump and iputils-ping on the generator. Only the generator captures
packets. Run [server_agent.py](server_agent.py) on the disposable target:

```sh
python3 server_agent.py ROOT 18769
```

Its private HTTP controller starts the unmodified binary with `host --stats 0.1`
under `taskset -c 0`. It timestamps existing CLI lines; it does not instrument
the runtime. Permit the control/game ports and ICMP only between the task group,
and SSH only from the authorized controller address. The HTTP service is a
study-only private-network controller, without Internet authorization/authentication.

Stage [run.py](run.py) on the generator. Its private job contains `serverPrivate`.
Verify timers, terminate behavior, DeleteOnTermination, same AZ, CPU topology and
clock synchronization first; retain clock checks. Then:

```sh
python3 run.py ROOT JOB_JSON
```

It runs the frozen rows and windows, six decoding workers on CPUs 0–5, probe on
CPU 6, controller/load coordinator on CPU 7, and capture processes on CPU 8.
The per-row seeds are 701/702/703/704. Preserve the non-primary seed-700 smoke
and all failures. Never redispatch after an SSH timeout without inspecting the
existing process/progress file. `progress.json` is atomic and is not an archive
readiness receipt. ICMP ping runs continuously at 5 Hz; WebSocket control and
game pings are independently timestamped by the probe.

## Replay and recompute

After all native counting stops, run focused reductions and documentation checks
remotely. The reducer is standard-library Python:

```sh
python3 -B -m unittest test_reduce
python3 assemble.py SELECTED_INPUTS SELECTED_INPUTS/pins.json selected-results.json
python3 historical.py HISTORICAL_281_DIRECTORY historical-results.json
python3 tables.py selected-results.json > tables.md
```

[reduce.py](reduce.py) binds every recorded binary payload to the ordered capture
by SHA, reassembles both directions, and uniquely matches each probe's command
to an outgoing masked WebSocket packet. Capture times observe the generator
kernel/GRO path, not a physical wire clock. Preserve loss/overlap/truncation or
matching failures; do not silently turn them into successful diagnostics.
The incoming frame timestamp is its completion across all covering TCP segments,
including out-of-order arrival. The main latency always uses one monotonic clock.

The RTT-subtracted summaries use each successful probe's nearest RTT observation
within two seconds, without clamping negative values. Deferral buckets use the
shipped stats intervals, including explicit coverage/reporting-gap categories;
they cannot identify an individual delayed tick. Death/timeouts/boundary misses
are separate from successful echoes and the minimum quota is checked explicitly.

[historical.py](historical.py) needs #281's unchanged four 320 archives,
complete retrieval receipts and selected results. It verifies archive size/SHA
before using the original canonical intervals. Its bounded output does not infer
missing packet timestamps. No historical row is reselected.

Close/member-verify/hash the source/raw archive before atomic readiness. Verify
the download and required sources before any source deletion or termination.
Retain native binaries, source manifest, probes, pcaps, CLI logs, controllers,
tests, failed attempts and analysis outside Git. Publish bounded selected inputs,
one set of pins and the helpers that recompute all tables. Complete the task
cleanup and all-enabled-region scan; disclose unrelated resources separately.
[export.py](export.py) produces `analysis.tar.gz` and atomic `analysis-ready.json`
after checking the separate native build and four row component archives. Stage
the helpers in `analysis-tools/` and put reports, scripts and verification logs
in `SELECTED_INPUTS/analysis/`; run `python3 export.py SELECTED_INPUTS`. Verify
the ready receipt, downloaded size and SHA before teardown. Full recordings stay
in the separately verified components, without duplication in the analysis archive.

`checkpoint.py` produces each `row.tar.gz`, its member manifest and atomic
`row-ready.json`. The selected-inputs tree contains `build-sources.tar.gz`,
`build-ready.json` and the native archive's `pins.json`, plus
`rows/LABEL/{row.tar.gz,row-ready.json}`. Select attempt 2's solo/80/160 and
attempt 3's 320; preserve the failed attempt-2 320 separately. The assembler
checks each component's receipt, digest, member manifest and matching build pins.
Run `supplementary.py FIRST_ATTEMPT_DIRECTORY supplementary-results.json` for
the three unqualified first-attempt checkpoints; never promote them to primary.
