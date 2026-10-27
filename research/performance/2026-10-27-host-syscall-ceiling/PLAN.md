# Syscall batching ceiling, 2026-10-27

Frozen before collection. Question: how much c7a socket cost could eliminating
syscall boundaries plausibly remove, compared with c8a? Expect c7a null and send
costs to exceed c8a, but do not assume null-call cost explains the TCP-path gap.
Safe RET also changes returns inside handlers; these probes cannot isolate each
mitigation or prove that io_uring reaches the delivery target.

## Fixed collection

- Frankfurt Spot: one c7a.4xlarge receiver/load generator and sequential
  c7a.medium then c8a.medium senders/hosts, one usable CPU each. No outside games.
- Reuse main `4532a76f22958ae3abcca801e90edd30ed9f466f`, exact #232 current/load
  binaries and worker, with their hashes. No Rust rebuild or runtime change.
- Compile the same portable C benchmark with gcc -O3 -pthread on the generator;
  copy/hash to both hosts. Pin sender CPU 0; four receiver threads on CPUs 0-3.
- Five repetitions per host, each null then send. Null: 100,000-call warmup,
  5,000,000 real syscall(SYS_getppid) calls; record empty-loop baseline, CPU and
  monotonic-raw wall ns/call. Reject nonpositive returns. Never print process IDs.
- Send: 389 established TCP sockets round-robin, TCP_NODELAY, nonblocking,
  22-byte writes to the separate receiver. Fresh connections each repetition;
  38,900 warmup calls, then 389,000 timed calls. Record CPU/wall ns/call, failed,
  partial and EAGAIN calls, received bytes/connections and receiver thread CPU.
  Errors/backpressure invalidate that repetition; retain it, no speed retries.
- On c7a only, one official-500 120 s diagnostic hold, 15 s warmup, seed 7,
  eight pinned load shards, zero host bots, same steering/rejoin as #232.
  An eight-client smoke precedes it. Count per-type/per-thread syscall entries
  with perf stat tracepoints for ~119 s inside the hold; denominator is executed
  tick starts inside the recorded observer window. Preserve wrapper timing bounds
  and counts. If perf cannot count before the hold, use strace -c -f instead and
  label its larger observer effect. Traced delivery timings do not qualify capacity.
- Capture /proc user/system counters, mitigations, kernel/compiler versions,
  operator stats and errors. No mitigation changes, reboots or io_uring prototype.

## Estimation and limits

Report median/min/max ns/call across all five attempts. Null CPU cost minus the
empty-loop CPU cost is an entry/exit-plus-handler-and-wrapper proxy, not a pure
mitigation measurement. Send minus that proxy is remaining TCP/ENA/softirq and
in-handler mitigation work; asynchronous work after the timed loop is excluded.

Multiply observed c7a calls per executed tick by each platform's null CPU proxy:
socket-network calls, all socket calls, and all host calls separately. Report
median-based estimates plus a generous max-null-repetition envelope, ms/tick and
share of a 7 ms core budget. The c8a projection uses c7a call counts, explicitly
not a new c8a official-500 measurement. Full elimination ignores io_uring dispatch,
submission/completion and remaining kernel work. Do not turn average CPU savings
into a p99-delay prediction or claim a hard bound on all possible batching gains.

## Budget, failures and retention

Hard cap $1. Generator 60-minute timer; each host 45 minutes; terminate-on-shutdown
and delete-on-terminate root disks. Planned generator <=30 minutes, each host
<=15, plus 30-minute margins. Launch compute caps .65*1+.06*.75+.08*.75=$.755;
reserve $.20 for storage, IPv4 and <=1 GiB downloads. Check login/budget before
each group; stop later work at $.90 projected. Existing login only; if expired,
stop and report without obtaining credentials. Local work <=2 logical CPUs.

Keep raw evidence and failed attempts outside Git, selected JSON and hashes in
the study. No performance-based exclusions/repeats. Delete created resources and
scan every commercial region for instances, volumes, open/active Spot requests,
EIPs and created groups/keys; label disabled/API-inaccessible inventories unverified.
Read-only cleanup-scan retries may recover transient API errors, retaining each
attempt; no new measurement/session recovery. Publish a short research PR, no
hosting/changelog/runtime change, and no cloud/account identifiers in public files.
