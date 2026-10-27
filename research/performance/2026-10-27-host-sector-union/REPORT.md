# Incremental food-sector union, 2026-10-27

**2.513% fewer user instructions** in the fixed official-500 fixture, with
byte-identical trace windows. Keep the change. Baseline `5eb0736`, runtime
`46ecdf8`; [results](results.json) hold full pins, hashes, arm medians/ranges,
all paired ratios and matching population/lifecycle summaries. [Protocol](PLAN.md).

The host visited every connection's subscribed sectors every broadcast. It now
retains membership identities/keys, updates counts only for changed memberships,
and visits the shared union once. Sectors retire after their final subscriber
leaves, including same-broadcast handoffs. Both food streams retain listing
lifetimes. Internal construction order changes; packet order remains sorted by
subscription. Encoders, admission, arena mechanics and thread ownership stay
unchanged.

## Method and results

Shared WSL2 Linux x86-64; Rust 1.99.0, locked release, thin LTO, one codegen
unit, debug 1. Independent targets and copied binaries rehashed after counting.
At most two logical CPUs: outer mask 0,2; counter children CPU 0; Cargo jobs,
Rayon and test threads one. No outside game traffic or cloud.

instructions:u counts the whole native process, including construction,
internal warmup, controllers, client decoding and record drains, then 100
decisions. Sockets/kernel work are excluded. One untallied warmup pair and five
alternating measured pairs; every counter scheduled at least 99%. No measured
pair was excluded. These are descriptive fixture results, not host CPU times.

| Fixture | Median instructions, billions base → candidate | Paired reduction (range) |
| --- | ---: | ---: |
| official-500 | 20.997 → 20.470 | 2.513% (2.510–2.517%) |
| Battledome-40 | 16.115 → 15.800 | 1.952% (1.945–1.961%) |
| Battledome-64 | 13.786 → 13.284 | 3.648% (3.615–3.684%) |
| Battledome-80 | 15.870 → 15.244 | 3.937% (3.929–3.958%) |

Both arms pass existing lifecycle/admission gates with identical populations.
Battledome uses #179's explicitly measured slot-zero reconnect and frame
refresh after 600 warmup decisions. It is secondary during arena redesign;
these native admissions do not measure socket spawn waits or qualify the
original dense fixed-arena target. No live CPU, send-latency or capacity gain
is claimed on this shared machine.

## Checks and reproduction

The full CONTRIBUTING Linux Rust group passed on the integrated runtime.
Both overlaid examples pass 16 tests each. Seven documentation tests pass.
Union/count oracle tests cover handoffs, fresh equal memberships, empty/closed
connections and slot resizing/reuse. Both handoff regressions fail without the
final-owner filter, in an isolated negative-control target. Existing physical/
decorated food and interrupted-broadcast regressions pass. Windows/macOS Rust
checks are left to CI.

Nine complete decompressed comparisons match: portable/protocol 32 operations,
official-500 one decision after warmup, Battledome-40/64/80 four (64 also through
the main catalog), and retained step_all/next_frame 384 with reconnect and
independent decoder replay. Finite traces do not expose every private queue/RNG
or establish live receipt timing.

Use Python 3.12+, Rust 1.99.0, cached Cargo dependencies, GNU patch, taskset,
a Linux Git checkout containing both pins and a working perf instructions:u:

```sh
python3 research/performance/2026-10-27-host-sector-union/reproduce.py \
  /path/to/linux-checkout /new/persistent/output --perf /path/to/perf
```

The sequence ran in stages; reconstructed harness bytes match the measured
inputs. Set LD_LIBRARY_PATH if perf needs it. Raw rebuilt evidence is local-only
in persistent storage. The plan records earlier temporary evidence lost during
disk recovery, including the rejected F cache; none is pooled with this series.
Selected results also survive in the Windows checkout.
