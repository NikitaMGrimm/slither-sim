# Syscall boundary ceiling, 2026-10-27 UTC

**Boundary elimination has a small average CPU budget to recover: about 0.22 ms/tick on c7a.**
The [plan](PLAN.md) preceded collection; [selected results](selected-results.json) retain all ten attempts.
Pinned main `4532a76f22958ae3abcca801e90edd30ed9f466f`; exact [#232](https://github.com/NikitaMGrimm/slither-sim/pull/232)
server/load binaries reused. Own Frankfurt Spot traffic; no outside game servers.

## Microbenchmarks

Five null/send repetitions per core, all valid. Median ns/call; CPU uses thread CPU time, wall uses
MONOTONIC_RAW. Each send is 22 bytes on one of 389 established TCP_NODELAY, nonblocking sockets, round-robin.
Separate four-thread c7a.4xlarge receiver; busiest thread 28.6% of a core.

| Core | Null CPU ns | Null wall ns | Send CPU ns | Send wall ns |
| --- | ---: | ---: | ---: | ---: |
| c7a.medium | 412.0 | 412.2 | 4511.0 | 4515.5 |
| c8a.medium | 49.1 | 49.1 | 1506.7 | 1507.2 |

CPU ranges: c7a null 411.8–413.5, send 4489.0–4530.8 ns; c8a null 48.3–56.9, send 1505.8–1530.2 ns.
Each receiver accepted/closed 389 connections and received 9,413,800 bytes including warmup, without errors.

The empty-loop-adjusted null proxy is 411.5 ns on c7a and 48.8 ns on c8a: 9.1% and 3.2% of send CPU.
Subtracting it leaves 4100 versus 1458 ns/send. This remaining cost includes TCP/ENA, loop differences,
softirq work charged during the loop and in-handler mitigations; later asynchronous work is excluded.
Null includes the trivial handler and libc wrapper, so it is not a pure entry/exit measurement.

Both kernels are `7.0.0-1014-aws` and report Retbleed "Not affected". c7a reports SRSO Safe RET and TSA buffer
clearing; c8a reports SRSO IBPB on VMEXIT only and TSA unaffected. No mitigation was changed. Architecture,
frequency and virtualization also differ; individual mitigation costs remain unidentified.
[Safe RET protects function returns inside handlers](https://github.com/torvalds/linux/blob/master/Documentation/admin-guide/hw-vuln/srso.rst),
so the residual cannot be interpreted as TCP without mitigation costs.

## Counted official-500 hold and ceiling

Perf counted syscall entries by host thread/type inside one 120 s c7a hold, after 15 s warmup.
The configured counter window was 119.499 s; its wrapper spanned 119.779 s and 17,111 executed tick starts.
The wrapper adds 0.23% to nominal duration; counter-enable edges were not independently timestamped.
All 500 clients played, no generator errors/new deferrals occurred, and tick rate was 142.85/s.
Tracing raised excess p99 to 95.865 ms; this is diagnostic, not a capacity qualification.

| Socket syscall | Calls | Calls/executed tick |
| --- | ---: | ---: |
| sendto | 7,413,362 | 433.251 |
| recvfrom | 1,651,650 | 96.526 |
| poll | 47,300 | 2.764 |
| accept4 | 47,300 | 2.764 |
| futex | 45,984 | 2.687 |

Entry counts combine successes/failures. Hypothetical complete boundary
removal multiplies calls/tick by the null CPU proxy; median / largest-null-repetition estimates follow.
Both columns use **c7a counts**; c8a is a projection, not a c8a official-500 hold.

| Scope | Calls/tick | c7a ms/tick | c8a ms/tick |
| --- | ---: | --- | --- |
| Socket network calls | 535.305 | 0.220 / 0.221 | 0.026 / 0.030 |
| All socket calls | 538.000 | 0.221 / 0.222 | 0.026 / 0.030 |
| All host calls, including diagnostics | 547.371 | 0.225 / 0.226 | 0.027 / 0.031 |

The most generous row recovers about **3.23 percentage points of one core on c7a, 0.44 on c8a**.
The socket-only proxy is about 10.8% of #232's 29.4%-of-core socket-system counter; process/task
user/system splits are not additive. Most socket cost survives boundary removal.
This model is not a hard bound: io_uring still dispatches operations,
and batching may change packetization, wakeups and scheduling. Average CPU savings do not predict excess p99.
A 10 ms delivery claim requires a host experiment.

## Reproduction, validation and limits

Use [micro.c](micro.c): `gcc -O3 -pthread micro.c -o micro`, then `micro null 5000000` and
`micro send ADDRESS 8766 389 38900 389000`, with `micro sink 8766 389 OUT.json` on the separate receiver.
Sender CPU 0, receiver CPUs 0–3; setup/teardown excluded, fresh sockets each repetition.
One portable C binary was compiled on the generator for both cores; host load follows #232.
[count_window.py](count_window.py) runs perf; [analyze.py](analyze.py) reduces small tick/count files and ceilings.

Two analysis tests, seven documentation tests, C warning/syntax checks and public identifier audits passed.
Rust checks not rerun.
After the owner's resource update, local work used one CPU for light scripts; frame reduction stayed on AWS.
One host per platform, no confidence interval; tracing may change calls/tick.

The inherited c8a archive command failed on an unmatched `*.log` glob after all repetitions completed.
All timings, receiver checks and native mitigation/CPU-count snapshots are retained in a local record.
The c8a remote archive, lscpu text and downloaded checksum manifest are unavailable; installation verified
the same binary manifest before timing. This limits offline provenance auditing. No replacement run occurred.

| Local-only archive (no public download) | SHA-256 |
| --- | --- |
| c7a.medium | `389348953fc4f29f51c7ba69a563ea64056f45d88f32405059d3e085423ddfb8` |
| c7a.4xlarge | `c9cc36576dcf27f6914b82930bd97a4af073ad83279bbe190746fd95fac8c1c7` |
| c8a-local-record | `921773bbe090b599d8d076fe72c284ec00e39edf703a100c6a5c35a4f3704ab4` |
| session-record | `75a11e918a75b11b17e8b51b33c0fe7a1cb93b8190470195434c84842d0ffd7f` |

## Cost and cleanup

Estimated **$0.105**, below $1, not an invoice: Spot history integrated over observed lifetimes, IPv4,
24 GiB roots, conservative storage/egress allowances and a $0.01 minor-transfer reserve. Generator/host
timers were verified at 60/45 minutes, with terminate-on-shutdown and delete-on-terminate root disks.
Final scan at 2026-10-27T23:08:04.200650+00:00 attempted all 34 commercial regions: all 17 enabled regions
had zero instances, volumes, open/active Spot requests, EIPs and created groups/keys. The 17 disabled/API-
inaccessible regions remain unverified. All created resources were deleted; no credentials were obtained.
