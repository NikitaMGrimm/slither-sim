# Official-500 acceptance v4: frozen plan

One authorized quiet-image confirmation series, registered before provisioning
or tick zero. Failed [v3](../2026-10-30-official-500-acceptance-v3/REPORT.md)
remains failed: run 2 had 126 deferrals, with about 1.43 CPU-seconds of other
user work during a four-second zero-idle episode. The process is unidentified.
This prospective environment change cannot rescue or reclassify v3.

## Fixed runtime, workload and gates

Reuse [v3's frozen plan](https://github.com/NikitaMGrimm/slither-sim/blob/bbc7d18c9d5052903bf3b07ba4067a48e8f040d5/research/performance/2026-10-30-official-500-acceptance-v3/PLAN.md)
and its pinned v2/v1/c7a-ladder/#256 sources, without copying them here. V3
[full replay check](https://github.com/NikitaMGrimm/slither-sim/blob/651f9526b038714c7487f4e74db4b240f7a0113d/research/performance/2026-10-30-official-500-acceptance-v3/check_replay.py)
must precede each offline selection. Runtime stays
`388be59aebf3108f5c510aa7269c08b67851b202`, portable locked release
`-C target-cpu=x86-64`, Rust 1.99, thin LTO, one codegen unit. Reuse the verified
v3 binaries, rehash before launch on each pair:

- Host: `d7ddbc80efcd0c0b5fb8e1d61d3931eb3aaf3c64b40171fb8687b533a184c780`.
- Clients: `f8424b4bd4e997c06d9d648a356f9d2ea07f3a651953474d4ae68f25ba3387fe`.

Rebuilding by the pinned v3 recipe is a reproduction option, not a reason to
change binaries between runs. No simulator, generator, policy, socket framing,
logging or stderr/file telemetry-path change. No memory-backed telemetry buffer.
The sole experimental intervention is the quiet host image policy below; added
host observation has overhead and is counted on the same host vCPU.

Each fresh pair: on-demand c7a.medium, every host/observer/diagnostic thread on
its sole vCPU; separate same-zone c8a.2xlarge generator in Frankfurt. Six full
received-frame protocol-19 A4 workers on CPUs 0–5, control on CPU 7, helper
threads inherit worker CPUs. Seed 7, official adaptive density, 500 sockets,
no host bots, no TLS/artificial latency/humans/outside game traffic.

Natural warmup from tick zero: 100 resident/400 rotating clients, inherited
20,000-tick lifetime schedule, no forced churn after tick 180,000. Same original
three 500-tick entry endpoints ending at/after 193,000: living >=490,
mean 45–65 inclusive, p90 156–196 inclusive, p99 >=200. Deadline 45 minutes.
Then ten seconds settling and a complete 1,800-second window; no checkpoint,
trimming, favorable endpoint or pooling. End-class lower bounds only: living
>=490, mean >=45, p90 >=156. Covering census bracket for deferrals and first
census at/after the end for class remain unchanged.

Each run must pass the same gates: **zero deferred ticks**; entry/end class;
all six clients alive/draining and advancing frames; summed live decision +
connector + publisher per-worker CPU median <70% and p99 <80%; input-lateness
p99 **<42 ms in every selected report**, including unchanged overlapping
boundary reports; zero unexpected transport/decode errors, write blocks,
buffer-full/rejects, admission refusals or forced retirement; sampled sockets
>=495 and mean living >=490; both send-queue slopes <8 KiB/s and last-third
mean <=first-third +512 KiB; CPU-clock/task reconciliation within
max(0.03 ms/tick, 2%). Preserve the ladder headroom quantile gate.

Report aligned world CPU p50/p99/p99.9/max, maintenance/ordinary ticks on the
unchanged 504 ms epoch, simulation/socket CPU, client gap/residual/excess tails,
steal, class, queues and cost. CPU/frame report tails use nearest rank
`ceil(n*p)-1` (max n-1), histogram cumulative rank >=`ceil(n*p)`, online and
offline from startup. World p99 <=7 ms and frame tails are report-only. Full
online/offline result equality, tails/maxima included, is enforced before
selection with a retained receipt; a mismatch fails evidence integrity.

## Quiet host image policy and repeat evidence

Apply [quiet_image.py](quiet_image.py) on the **host only before tick zero**.
Record actual discovered units, states and `systemctl list-timers --all` before
changes, after quiescence and after the world. Runtime-mask and stop whichever
of `apt-daily.timer`, `apt-daily-upgrade.timer`, `fwupd-refresh.timer`,
`fstrim.timer`, `man-db.timer` exist. Wait for running corresponding jobs to
finish (active/running, activating or deactivating), bounded to five minutes;
do not kill those jobs. Stop and runtime-mask existing fwupd, snapd and
amazon-ssm-agent services, including snap-packaged SSM and snapd's activation
socket, to prevent reactivation during the window. Missing units are recorded,
not assumed present. Verify targeted timers/services are inactive and masked.
Failure to quiesce stops collection, not a reason for a new attempt.

Keep networking, SSH, timekeeping, journald/rsyslog and the verified shutdown
deadline. No sysctl, scheduler, frequency, network or CPU tuning. Do not silence
server logs or change the ordinary `host.log` -> HTTP -> `host-stream.log` path.
Generator services stay as in v3. All changes disappear with instance teardown.

[host_diagnostics.py](host_diagnostics.py) adds a host-side process sampling all
readable `/proc/PID/stat` cumulative user/system CPU and start-time identity
once per second, alongside the entire `/proc/stat` including `ctxt` and
`procs_running`. Record scan boundaries, vanished/unreadable tasks, and missed
cadence rather than inventing samples. It uses regular files on the same host,
and its CPU is retained with all other processes. Retain a timestamped journal
covering the whole world (hence the complete selected window), plus before/after
timer inventories and actual unit states. No journal reads, scientific
reductions, builds or tests during counting; journal extraction is after stop.
One-second observations still cannot identify every short-lived task or an
individual subsecond off-CPU site; disclose these limits if a stall repeats.

## Sequence, stop rule and resource ceiling

Two independent fresh on-demand runs. Start run 2 only after run 1 completes
and its raw archive is retrieved/verified and its instances/volumes are gone.
A complete run 1 failure is retained; run 2 still supplies the second independent
run of this one predeclared series, as in v3. No completed-run retry, replacement,
third run, tuning between runs or exclusion. An incomplete run stops the series
incomplete; there is **no infrastructure replacement** in v4.

**Stop rule:** if v4 fails any gate, no further acceptance attempts until
world-tick headroom improves. Maintenance-tick mean must fall clearly below
its current 7.1–7.4 ms before another acceptance series. This is a prerequisite
for future work, not a post-result override or a new gate within v4.

Profile `slither-bench`, Frankfurt, on-demand only, tag instances/volumes/network
interfaces/key/group at creation (Project/Task/Role/expiry). Same source-restricted
management and peer-only game/control/artifact ports, IMDSv2, delete-on-termination
root disks, terminate-on-shutdown. Boot shutdown +90 minutes, verified <=92 minutes
from launch; planned pair limit 88 minutes. Never extend timers.

Verify current rates <=$0.06/h host and $0.50/h generator before launch. Two pairs
at full verified lifetime cost at most `2 * (92/60) * $0.56 = $1.717334` compute;
reserve $0.40 for 10/24 GiB gp3 roots, public IPv4 and bounded artifact transfer:
**$2.117334 < $3 hard cap**. At most 512 MiB Internet artifacts, each compressed
run archive <=150 MiB, with 100 MiB reserved for source/binaries/proofs. No extra
build instance, Spot request, paid service or instance replacement. Preserve and
clean a partial launch on failure. No local builds/heavy work or instabox access.

Retrieve/hash each full archive and additional evidence before teardown. Final
scan of every enabled region; delete task key/group and verify task instances,
requests and volumes gone, preserving unrelated resources. No permission
workaround. Raw cloud identities/addresses/hostnames, complete process/journal
data, commands and personal paths stay outside Git. Publish only selected safe
values, archive bindings, bounded cost/cleanup receipts and research index row.
No root README/changelog edit and no runtime optimization in this round.

## Freeze and reproduction

Commit this plan and three small host-side files before provisioning. Stage the
v3 linked harness from `bbc7d18c`, corrected v1 helpers from `fff66d0a`, and full
replay check from `651f9526` as those v3 instructions specify. Overlay this study's
`host_agent.py`, `quiet_image.py`, `host_diagnostics.py` only. Run syntax/module
checks on AWS before the world; retain proof and verify staged delta hashes
against the committed study. Host/client hashes must remain as above. Preserve
all actual quiet-image actions and missing-unit records. Follow inherited
startup, warmup, collection, nearest-rank selection/assembly and teardown; run
the full comparison before inherited `select.py`. Report each run separately.
Freeze commit, source/binary identities and proof/access bindings go in ATTEMPTS
and preflight receipt before timing; no per-file source-hash map in the PR.
