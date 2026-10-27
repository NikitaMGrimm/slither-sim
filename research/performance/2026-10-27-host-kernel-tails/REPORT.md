# One-core kernel and scheduling tails, 2026-10-27

**The old 58 ms tail was predominantly late simulation ticks; lower socket priority moves the tail to delivery.**
None of three cheap scheduling prototypes passes. No runtime fix/default or hosting-capacity change is proposed.
Fresh runtime: `587b597` (main at task start, before #212); historical #211: `65ff713`. Later main is unqualified.
The [plan](PLAN.md) and [follow-ups](EXPERIMENTS.md) precede collection; [selected results](selected-results.json) retain every hold.

## Unprofiled official-500 results

One 120 s hold per arm, real protocol-19 sockets, zero bots, one c7a.medium CPU. Same #201 frozen load and gate:
frame-excess p99 <=10 ms and no new deferred ticks, subject to decoded traffic/occupancy. All four miss.

| Socket setting | Excess p99/max ms | Tick completion lag p99/max ms | Busy tick p99/max ms | CPU ms/tick | Core % | RSS MiB |
| --- | --- | --- | --- | ---: | ---: | ---: |
| Default | 72.930 / 186.613 | 71.301 / 182.353 | 9.543 / 24.005 | 4.762 | 68.0 | 51.70 |
| Nice +5 | 19.795 / 82.521 | 5.318 / 15.382 | 6.331 / 15.130 | 4.719 | 67.4 | 50.88 |
| Nice +10 | 20.421 / 93.369 | 4.039 / 12.845 | 5.258 / 14.077 | 4.718 | 67.4 | 50.55 |
| SCHED_BATCH, nice 0 | 43.368 / 165.562 | 44.038 / 159.582 | 9.137 / 22.771 | 4.711 | 67.3 | 51.06 |

All had zero new deferred ticks, approximately 142.86 ticks/s and mean playing >=499.998. Effective socket
nice/policy was verified through /proc; other host threads stayed unchanged on CPU [0]. Full splits/counters
are in JSON. Nice +5/+10 remove most tick lateness but still produce ~20 ms delivery tails. Batch also misses.

## Retained #211 data first

- c7a tick-completion lateness p99 was 57.501 ms versus frame excess 58.384 ms; c8a was 3.030 versus 3.120 ms.
  Individual busy ticks hid accumulated backlog. Zero deferrals permits catch-up backlog below roughly 250 ms;
  it does not establish a 10 ms delivery SLO. Average idle time after bursts cannot recover missed deadlines.
- 366,430/373,020 frames above 10 ms excess (98.2%), and all 108,764 above 50 ms, overlap a >10 ms tick-backlog
  bin or its predecessor (100 ms bins). Only 526,481/7,535,726 frames occupied those exposed bins. This is temporal
  association, not packet-by-packet causal attribution. There were 26 backlog episodes; longest 118 ticks.
- No episode began immediately after a scheduled map/leaderboard tick. Maximum busy time: map-due 11.989 ms,
  leaderboard-due 4.919 ms, neither 16.188 ms. Due flags are reconstructed from the source clocks, not packet tags.
  Receipts retain times/sizes/elapsed headers, not families or actual sends. Absolute since-setup header rounding
  avoids per-frame truncation drift. Receipt bursts alone cannot distinguish late production from late sends.

## Fresh kernel/stage diagnosis

Diagnostic holds are 90 s with a 60 s system-wide cpu-clock profile; profiler/trace timings do not qualify capacity.
Both cores ran the same binary, image and Linux 7.0.0-1014-aws kernel. No lost perf samples or generator errors.

| Profile | Simulation user/kernel % | Socket user/kernel % | Send-path samples % | Socket softirq samples % | Network IRQ/s |
| --- | --- | --- | ---: | ---: | ---: |
| c7a.medium | 30.4 / 1.6 | 4.7 / 31.9 | 18.54 | 6.60 | 21884 |
| c8a.medium | 19.7 / 0.4 | 2.6 / 9.3 | 6.45 | 0.74 | 9222 |

c7a/c8a made 3,721,082/3,770,164 sends and 832,644/836,646 receives in 60 s; c8a made more polls (49,175 vs
34,488). The 3.4x socket-kernel gap is not 3x application work. c7a polling stacks cost 2.88% of the core;
TCP transmit/ENA and interrupt work dominate. Top socket leaves include __dev_xmit_skb, __dev_queue_xmit,
do_syscall_64, sock_poll, ena_start_xmit and handle_softirqs. Kernel costs are spread across the path.
c7a reports Safe RET / TSA buffer clearing; c8a VMEXIT-only SRSO / TSA not affected. Their causal share is
unisolated: this is a platform comparison, not a mitigation intervention. ENA allowance-exceeded and TX queue-stop
counters were zero since boot on both servers. More frequent network interrupts are a concrete further lead.

Profiled c7a simulation accumulated 16.56 s runnable wait and 54,454 involuntary switches during ~90 s.
Slot-0 lateness p99: encode handoff 68.174 ms, send-pop 73.349 ms. Paired encode-to-socket-queue p99 was
2.443 ms, queue-to-send-pop 0.157 ms. Most baseline delay predates handoff; some occurs before socket service.
Send-pop precedes buffered flush/TCP and is not a wire timestamp. C8a encode/send-pop p99 was 3.715/3.861 ms.
Nice-arm post-tick delay is inferred from tick/receipt tails; those arms have no wire timestamps.
Lower nice trades CPU weight for timely ticks; its remaining delivery tail is a different bottleneck. Batch
disfavors wakeups without lowering nice ([Linux scheduling](https://man7.org/linux/man-pages/man7/sched.7.html)); it did not qualify here.

## Reproduction, limits and evidence

Same #201 exact load binary, seed 7, eight pinned shards on separate c7a.4xlarge, 100 ms controls, no boost/TLS/
added latency, 15 s warmup and fresh worlds. Rust 1.99 locked release/thin LTO/one codegen unit, line tables,
jobs=1 and fresh target. [instrument.py](instrument.py) extends #201 only in isolated snapshots; normal holds
disable stage tracing. The worker sets socket nice/policy before warmup. [analyze.py](analyze.py) reduces host
scheduler/stage/kernel evidence: `python analyze.py RAW_HOST OUT.json`; #201's reducer owns delivery grades.
Perf used `record -a -F 99 -e cpu-clock -g --call-graph dwarf,8192`; perf stat counted syscalls/context switches;
/proc sched/schedstat/softirqs/interrupts, mpstat and ethtool provided counters. Only ephemeral host settings changed.
CPU uses user/system ticks; kernel sample groups partition system-wide periods including idle. Some user DWARF
frames are unresolved; kernel stacks/symbols are retained. Tick lateness is start/end minus virtual 7 ms schedule,
minimum-normalized in the half-open hold. Stage timestamps are buffered, with slot-0 families tagged at handoff.
One seed/hold, evolving worlds, historical comparison and profiler overhead limit generalization; no retail EPYC,
human-browser, TLS or long-uptime claim. No performance repeats/exclusions. All six holds re-reduced locally;
three analysis regressions and seven documentation tests pass. Full Rust group was not run: no runtime Rust change.
Raw logs/receipts/profiles/binaries/metadata remain local-only outside Git, not publicly downloadable. Pins are in JSON.

| Local-only archive | SHA-256 |
| --- | --- |
| c7a.medium | `dc73b0b00342477d28cae8b0afa8a60c66a3d4e3e929fafb9da4e31f77400bc0` |
| c8a.medium | `60baa3b7e1a50c08a712adde79ca67733aba1f78e19d6643ddd1132d41871a0d` |
| c7a.4xlarge | `bbb26b185e2b3415314590a91f368b23cad2f3f7baeb77612cd63b3d5c10fcdc` |
| session-record | `921512c543a6e66b6c7e4acd6a9ef1a8b82065b3184ddf73aa388718d62c1664` |

## Cost and cleanup

Estimated **$0.544**, below $3; not an invoice. Compute $0.247, IPv4 $0.006,
storage $0.005, egress $0.277, minor transfers $0.010. Launch quotes × lifetimes;
$0.12/GB-month disks and $0.12/GB downloads, counting repeated archives. No credits. All instances had verified
120-minute shutdown timers, terminate-on-shutdown and root delete-on-terminate; servers ran sequentially.
Final scan 2026-10-27T14:48:04.115648+00:00: all 34 commercial regions attempted, all 17 enabled regions had zero
non-terminated instances, volumes, open/active Spot requests, EIPs and created groups/keys. Other 17 regions were
disabled/API-inaccessible, so their inventories are unverified. No credentials or regions were obtained/enabled.
