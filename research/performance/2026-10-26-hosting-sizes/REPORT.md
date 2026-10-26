# Short hosting resource samples, 2026-10-26

These three sequential loopback runs give initial resource numbers for the
[Linux hosting guide](../../../docs/hosting.md). They do not qualify a 1-vCPU
server for these populations. The [plan](PLAN.md) was written before collection;
[results.json](results.json) contains selected counters, configuration, executable
and harness hashes, the runtime pin and raw-output digests.

## Results

| Preset | Requested clients | Mean / final playing | Host CPU | Peak RSS | Generator CPU | Executed / deferred ticks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Battledome | 80 | 35.486 / 40 | 55.59% | 9.465 MiB | 9.30% | 4285 / 0 |
| Official | 250 | 250 / 250 | 51.37% | 21.633 MiB | 44.43% | 4285 / 0 |
| Official | 500 | 500 / 500 | 91.08% | 44.188 MiB | 85.18% | 4284 / 0 |

All three completed without generator decoder/network errors. They decoded
35,054, 248,934 and 496,440 own-gameplay frames respectively during the holds.
Battledome recorded 16 measured deaths and 27 admissions. Both official runs
kept every client playing throughout, with no hold deaths or reconnects. These
are the harness's synthetic weaving/avoidance clients, not realistic long-lived
large snakes or background Foragers.

**Battledome is depleted:** host connected players ranged 32–43 in the hold.
The generator allows three pending logins at a time; waiting admission therefore
prevents it from opening all 80 desired sockets. Mean idle client state was
41.515, mean login state 2.994. This is an 80-client attempt, not an 80-connected
or 80-living capacity result. Admission at 64–80 is being changed separately;
refresh this row after that change. No spawn or preset changes were made here.

No speedup or implementation comparison is claimed. The 500-client host and
generator both used most of their dedicated cores. Short-run CPU margin is
small, and zero deferred ticks says nothing about long-run tail latency,
grown-body cost or public-network behavior. Initial low food population after
joins also differs from a mature populated arena. Use at most 250 seats as an
initial small-box configuration and measure the intended workload.

## Method and provenance

- Runtime: `249cdb217d83a234895bc7eee4d98c3636c1cd76`, no runtime edits.
- Linux x86-64 under WSL2 on one desktop core for the host; a separate logical
  CPU for one generator. Shared desktop activity was uncontrolled: observed
  load averages were about 2.4 before preparation and 2.9 between holds.
- Rust 1.99.0, default features, locked release build, thin LTO, one codegen unit.
  Build jobs and Rayon threads both limited to one. Host and generator affinity
  restricted this work to two logical CPUs; no runs overlapped.
- Each case: seed 7, zero background bots, default preset/delivery, no added
  latency, 15 s warmup + 30 s hold + 2 s host cleanup grace. One attempt per case.
  Host startup precedes the common epoch by about one second. No TLS.
- CPU: `/proc/PID/stat` user + system deltas from the first sample after hold
  start to the last sample just before hold end, sampled at about 100 ms.
  Actual CPU intervals: 29.881, 29.978 and 29.984 s. 100% means one logical CPU.
  Host CPU includes simulation, socket service and the lightweight probe wrapper;
  generator CPU is separate. Simulation-thread wall occupancy is not CPU time.
- Peak RSS: host `VmHWM` at that final sample, cumulative from process start,
  including warmup. No proxy, generator, OS or kernel socket memory included.
- All cases processed traffic inside the timed region. Full local outputs and
  sampling/validation scripts are retained outside Git under the task's ignored
  `outputs/hosting-20261026/` directory; access is local-only. Digests identify
  the raw material without making it publicly downloadable.

No failed load attempts were discarded. Preparation encountered Windows Git
worktree metadata that WSL Git could not resolve; making the source archive with
Windows Git fixed it before compilation. A selection script initially lacked
the login-shell Rust PATH and was rerun through a login shell. These were setup
failures, not additional workload attempts; their messages are in local notes.

## Reproduction

Use the same runtime and unchanged study harnesses. `examples/host_load.rs`
measures background bots plus one watcher, so it is unsuitable for the requested
socket counts. This uses the existing
[socket capacity harness](../2026-10-26-host-capacity/host_capacity_load.rs) and
[host counter wrapper](../2026-10-26-host-capacity/host_probe.rs) instead.
Register them only in a temporary source snapshot:

```sh
export CARGO_BUILD_JOBS=1 RAYON_NUM_THREADS=1
SNAPSHOT=$(mktemp -d)
git archive 249cdb217d83a234895bc7eee4d98c3636c1cd76 | tar -x -C "$SNAPSHOT"
cd "$SNAPSHOT"
cp research/performance/2026-10-26-host-capacity/host_probe.rs examples/
cp research/performance/2026-10-26-host-capacity/host_capacity_load.rs examples/
taskset -c 6 cargo +1.99.0 build --release --locked --bin slither-sim \
  --example host_probe --example host_capacity_load --example host_load
```

Choose two available logical CPU IDs in place of 6 and 7 if necessary. For each
case, create a fresh output directory, write its config from `results.json`, then
start both processes with the same epoch (Unix seconds about one second ahead):

```sh
OUT=$(mktemp -d)
# Set COUNT=80, 250 or 500; CONFIG is the matching saved config file.
EPOCH=$(python3 -c 'import time; print(time.time()+1)')
taskset -c 6 target/release/examples/host_probe "$CONFIG" 15 30 \
  "$OUT/host.json" "$EPOCH" >"$OUT/host.log" 2>&1 &
HOST_PID=$!
STUDY_EPOCH_UNIX_S="$EPOCH" STUDY_SERVER_IP=127.0.0.1 \
  taskset -c 7 target/release/examples/host_capacity_load "$COUNT" 18444 30 15 \
  "$OUT/load.json" 0 >"$OUT/load.log" 2>&1 &
LOAD_PID=$!
```

The saved configs bind `127.0.0.1:18444`. Sample `/proc/$HOST_PID/stat`,
`/proc/$HOST_PID/status` and `/proc/$LOAD_PID/stat` across the common hold using
the CPU/RSS definitions above, then `wait "$LOAD_PID"` and `wait "$HOST_PID"`.
Keep each process's exit status, decoded traffic, client states and deferred
ticks. Run the next case only after both previous processes exit.

## Deployment-command checks

The guide's x86-64 v0.6.0 release archive passed its published SHA256SUMS check.
The installation block ran with temporary destination paths and without `sudo`;
all ten shell blocks passed `bash -n`.
Both guide configs parsed with that binary and the pinned build, with loopback
bind overrides. Each existing Python bot run ended `status=playing`, advancing
to about 4.6 s after 100 steps.

`systemd-analyze verify` accepted the extracted unit with only executable/config
paths redirected to the temporary installation. Nginx 1.24.0 accepted the guide's
server block with test ports, hostname and certificate paths substituted. A
plain test listener relayed the existing bot; TLS with a temporary locally
trusted certificate returned HTTP 101. Curl then timed out as expected because
the upgraded connection remained open without a game login.

These checks did not install/enable a system service, exercise the bind
capability or restart policy, obtain/renew a public certificate, test public
reachability or a browser/mod, test ARM64, or execute privileged upgrade and
rollback. Those remain destination acceptance steps, explicitly listed in the
guide. No outside game server was contacted.
