# Reproduce the one-core mature socket ladder

This is a short, evolving-world capacity test, not a paired optimization or an
uptime qualification. Freeze [PLAN.md](PLAN.md) before paid collection. Cloud
collection requires its own authorization, budget and verified teardown.

## Reuse earlier code

Stage an external directory, never a Git checkout for raw outputs. Apply
[#256's harness.patch](../2026-10-28-capacity-checkpoint/harness.patch) there to
recover its scripts; put those files in `shared256/`. Copy these earlier sources
into the external `shared248/` directory:

- [prepare.py](../2026-10-28-official-density-capacity/prepare.py): archived-source
  edits and hash helpers.
- [observe.rs](../2026-10-28-official-density-capacity/observe.rs): read-only
  census, including lengths and deferred ticks.
- [run.py](../2026-10-28-official-density-capacity/run.py): Linux task CPU and
  affinity snapshots.
- [select_results.py](../2026-10-28-official-density-capacity/select_results.py):
  nearest-rank quantiles, CPU-counter interpolation and reconciliation.

The [earlier AWS study](../2026-10-26-epyc-host/REPORT.md) and
[same-session recheck](../2026-10-27-epyc-host-recheck/REPORT.md) own the initial
Spot/build/load methodology. The private run reuses the authorized earlier
`aws_run.py` API wrappers. Credentials, key material, cloud requests, identifiers,
addresses and SSH configuration stay outside every repository. The new scripts
here contain only the changes for private-network, dynamic-count collection.

## Build and launch

Archive runtime `692cd561714b8af3ea95adb3adcbd807b3a48734` as `main.tar` with
Git's commit-comment provenance. Stage this study's `prepare.py`, `build.sh`,
`host_agent.py`, `run_ladder.py`, `metrics.py` and `window.py` beside the archive
and linked directories. On the eight-vCPU generator install the standard build
tools and Rust 1.99.0, then run `bash build.sh` from `/srv/slither-study`.
`prepare.py` applies #256's CPU-clock/census overlays to an archived source copy,
adds a once-per-second external-playing-peer count, and derives a six-worker,
private-network, dynamically resized driver from #256's received-frame A4 client.
It does not change the policy or use authoritative observations.

The build explicitly sets `RUSTFLAGS='-C target-cpu=x86-64'`, removes inherited
encoded Rust flags, uses four build jobs and one bounded Cargo target, and deletes
that target after copying and hashing both release executables. Native Zen 5
instructions are not enabled. Transfer `bin/host` from generator to server over
the private network; verify its SHA-256 against `pins.json` on both machines.

Provision exactly the two authorized Spot instances in the same Frankfurt AZ.
Verify terminate-on-shutdown, one-time Spot/terminate-on-interruption, root-volume
delete-on-termination and the boot-installed shutdown timer before building.
Use private addresses for game, control and binary transfers. SSH ingress is
source-restricted; public game ingress is restricted to the authorized owner's
single-address rule. The control and artifact ports accept only task-group peers.
No Elastic IP, NAT gateway or unrelated service is created.

Stage a private `job.json` on both machines containing `serverPrivate`,
`generatorPrivate`, `gamePort` and `controlPort`. Start `host_agent.py` on the
server, wait for listening readiness, then `run_ladder.py` on the generator.
Start `generator_boundaries.py` on the generator during warmup, before counting;
it uses control core 7 and preserves that machine's private command inventories
around measurement transitions.
The host has zero in-process bots, 751 socket slots and default official
world/delivery with seed 7. Its only vCPU runs every host thread. Six generator
workers pin to cores 0–5, control to core 7; affinity is read back throughout.
All bots opt into physical-food client-equivalent observations and run the same
Forager every 42 ms, decoding and draining received frames with normal pings.

Once warmup starts, probe the public game endpoint from a source allowed by the
owner rule. Only after success write the requested one-line endpoint file outside
all repositories. Delete it when stopping or terminating the server, including
failures. The public endpoint and access-rule address are never public evidence.

## Collection and selection

Follow the plan's tick-zero churn, mature screen, ten-second settling and
30-second windows. No checkpoint restoration or edited food history is used.
`run_ladder.py` resizes the same clients/world, counts complete interior blocks,
checks clients/queues and stops at the predeclared median, deferral or validity
boundary. It takes exactly one descending confirmation. Census drift and newly
short joiners remain visible instead of being hidden behind a fresh warmup.

`host_agent.py` retains full private process command lines at each measurement
boundary, host/task CPU, observer CPU and host-vCPU counters. The generator's
observer records the same command evidence within its 50-ms polling interval.
#256's clocks sum
aligned simulation-thread and socket-thread user-plus-kernel CPU at consecutive
tick boundaries. Compute combined quantiles from those sums; component p99s
must not be added. Phase clocks are nested simulation averages. Collection
overhead charged to host tasks remains included; observer CPU is separate.

After collection and outside counted windows:

```sh
python check_harness.py EXTERNAL_STAGED_HARNESS
python select.py PRIVATE_RAW EXTERNAL_STAGED_HARNESS results.json
```

The synthetic control checks aligned sums against deliberately anticorrelated
components, full-block selection, queue/cadence rejection, human occupancy and
deferred-tick rejection. The selector reproduces each online result exactly,
reconciles clocks with interpolated task counters within max(0.03 ms/tick, 2%),
verifies client frame progress and no forced retirements/refusals in windows,
checks copied-binary identity and retains only cited aggregates. CPU counters
use `CLK_TCK=100`, read back on both instances. The first eight `/proc/stat`
categories describe a separate accounting view; they are not additional host
work to add to task CPU.

Retain failed/inconclusive attempts. A failed generator, cadence, population or
queue gate is a limit, not a qualified host capacity number. Human-affected
windows report sampled public-playing occupancy and `--stats` seat ranges.
No confidence interval, cross-hardware extrapolation or long-uptime claim follows
from a single ascending/descending ladder.

## Teardown and publication

Remove the endpoint file, terminate both instances, cancel any task open/active
Spot requests, check root volumes disappear, and delete the created key pair and
security group. Match direct-launch Spot requests by the recorded request or
instance identity; their launch specification may omit the key name. Enforce
failed cleanup/launch guards with explicit exit-code handling before another
command can create resources. Reattempt every commercial region for nonterminated instances,
open/active Spot requests, volumes and Elastic IPs. Preserve both counts and
unavailable-region results privately; unavailable coverage is not a zero count.
Verify each task resource separately, and retain rate/duration/storage/address
receipts for the bounded cost estimate. Do not enable regions or remove unrelated
resources. Publish only the selected results, this study's changed harness and
report after privacy, whitespace and documentation checks.
