# Live mature-500 checkpoint after compact histories

Main at the task's start **does not fit the seven-ms world-CPU p99 budget** on
this core. Its three qualified cohorts have p99 **8.180–8.257 ms**, versus
**8.863–9.051 ms** for #256's main rerun in the same session. Maintenance ticks
still dominate the tail: **85.7–95.2%** of main's slowest 1%, averaging
**8.531–8.634 ms** against **3.931–4.353 ms** on other ticks. Socket work and
broadcast lead. There is no runtime change or optimization in this PR.

All six primary holds and their separate coarse diagnostic windows qualify.
Clients have substantial headroom, no observed write backpressure, and zero
reported deferrals. Main's observed maximum nevertheless reaches **14.016 ms**;
the result is not a claim that every tail event improved.

## Question, pins and method

The maintaining agent requested a live replication of the [capacity checkpoint](../2026-10-28-capacity-checkpoint/REPORT.md)
after the exact runtime changes through compact occupied snake histories.
[PLAN.md](PLAN.md) froze three fresh pairs in AB/BA/AB order, the mature class,
client/transport predicates, retries and budgets before host timing. This is a
same-session descriptive checkpoint, not a fixed-trajectory causal comparison.

- A: `551cb1030ea2bf84f83bbeb167233b917900929a`, #256's post-#255 main.
- B: `b750004aeb8f11be61dc483a2ce458c6194f116f`, main when this task began,
  including compact histories. Later main commits, including #278, are not timed.
- One unchanged #256 full received-frame A4 client, built from
  `d127acd1f0e9db2100c8a1dd5c2fa43e2deff506`, serves both hosts. No thin fallback.
- A 4-vCPU Zen 5 EPYC KVM guest: simulation and socket tasks share vCPU 0,
  received-frame clients run on vCPUs 1/2/3, and low-rate control/telemetry on 3.
  Affinity is read back throughout. Zero in-process bots or outside game servers.
- Rust 1.99.0; #256's native-target release, thin LTO, one codegen unit and line
  tables on both arms. This is a same-core build, not a portable-build benchmark.

Each cohort starts at tick zero with seed 7, official delivery/density and the
seven-ms physics tick. The original forced retirement/rejoin warmup ends at
180,000 ticks. At or after 193,000 ticks, three successive 500-tick endpoints
must have at least 490 living, mean length 45–65, p90 156–196 and p99 at least
200. After five seconds settling, one 30-second primary hold is collected. Every
sampled census in its complete interior 100-tick blocks must stay in class.
These worlds evolve independently; radius, food, visibility and actions are not
forced equal. They are synthetic A4 worlds with real sockets, not human traffic.

World CPU is the **aligned sum** of simulation and socket thread CPU at each
completed tick boundary, including user and kernel time. Quantiles are computed
after summing; component p99s cannot be added. Each accepted primary row retains
4,200 ticks, about 29.4 seconds of its hold. Process/task accounting independently
reconciles the means under the original max(0.03 ms/tick, 2%) tolerance.

Primary windows keep optional syscall/fine clocks and per-tick phase exports
off. Existing broad phase clocks remain. Each primary is followed by a separate
20-second coarse export, then a 15-second syscall diagnostic. The coarse export
copies existing phase deltas and actual food-maintenance flags without adding
phase clock calls; export/serialization overhead makes it diagnostic only.
Food cadence is verified over **8,400 per-tick flags per arm**, with **114 measured
72-tick gaps per arm**. Primary cadence labels use that validated source rule.
No socket CPU is shifted to neighboring tick bins. The next tick retains report
observer CPU; post-report markers and neighboring socket means are retained.

## Isolation, failures and evidence boundary

[CALIBRATION-PLAN.md](CALIBRATION-PLAN.md) was committed as `b682c392` before the
fixed idle observations. All twelve 30-second windows qualified. Largest-other
CPU was 0.07–0.09 seconds per idle window, and total other-user CPU 0.14–0.19.
The calibrated envelope rate is 0.002998926999628939, with 10-ms counter quanta.
The measured gate and full plan were committed as `165d172d` before host timing:

`allowance(W) = W * (0.002998926999628939 + 0.01) + 0.02 CPU seconds`.

This is the preregistered idle envelope plus one-percent excess activity and two
endpoint quanta. Each primary's allowance is about 0.402 seconds; observed largest
other-user CPU is 0.07–0.08 seconds. No CPU is subtracted or process allowlist
added. PID/start-time endpoint accounting can miss short-lived processes; the
screen is not proof of complete isolation. IRQ, softirq and steal context is
reported separately and must not be added as disjoint task CPU.

The first A1 attempt failed only the mature class: primary p90 briefly reached
153, below 156. Its original data and costs are retained and excluded. The one
permitted fresh class-only retry qualified. No isolation retry or performance
retry was used. The legacy selector initially raised at class before calculating
isolation. Before the retry's primary window, an independent audit with only that
class assertion bypassed verified every other gate (0.08 other CPU seconds against
0.402 allowed). The selector then evaluated class and isolation together before
naming a retry class. Neither predicate changed; simultaneous failures stop.
Four synthetic combinations verify that classification. See `verification.json`.

The primary series cost **17,355.234 CPU seconds (4.821 hours)** including the
600-second build/calibration/pilot allowance and failed attempt, below 21,600.
All results below are fresh; historical #256 numbers are context, not pooled
samples. Thirty-second asynchronous cohorts do not establish stationarity,
sustained uptime or a causal speedup. Loopback TCP can charge receiver processing
to sender syscalls and overstate real-NIC gateway cost. No cross-CPU capacity
projection is made.

## Selected results

All CPU costs below are milliseconds per completed tick. A = 551cb103; B = b750004a. Rows preserve AB/BA/AB order. Each primary row has 4,200 ticks.

Primary aligned world CPU

| Cohort | p50 | p99 | max | mean |
| --- | --- | --- | --- | --- |
| A1 | 4.170 | 8.863 | 12.864 | 4.260 |
| B1 | 3.850 | 8.233 | 10.792 | 4.081 |
| B2 | 3.756 | 8.257 | 13.188 | 3.995 |
| A2 | 4.849 | 9.051 | 12.829 | 4.755 |
| A3 | 4.678 | 9.012 | 12.486 | 4.765 |
| B3 | 4.437 | 8.180 | 14.016 | 4.413 |


Primary simulation CPU (includes broadcast)

| Cohort | p50 | p99 | max | mean | Broadcast mean |
| --- | --- | --- | --- | --- | --- |
| A1 | 2.214 | 5.459 | 8.685 | 2.325 | 1.525 |
| B1 | 1.945 | 4.336 | 7.547 | 2.046 | 1.202 |
| B2 | 1.921 | 4.355 | 6.242 | 2.027 | 1.193 |
| A2 | 2.663 | 5.592 | 8.686 | 2.716 | 1.853 |
| A3 | 2.586 | 5.964 | 7.959 | 2.703 | 1.822 |
| B3 | 2.235 | 4.502 | 7.590 | 2.278 | 1.347 |


Primary socket CPU

| Cohort | p50 | p99 | max | mean |
| --- | --- | --- | --- | --- |
| A1 | 1.895 | 4.105 | 4.784 | 1.935 |
| B1 | 1.976 | 4.435 | 5.495 | 2.035 |
| B2 | 1.880 | 4.399 | 7.337 | 1.969 |
| A2 | 2.021 | 3.919 | 5.342 | 2.039 |
| A3 | 2.023 | 4.012 | 5.411 | 2.062 |
| B3 | 2.120 | 4.481 | 6.426 | 2.135 |


Maintenance versus other ticks

| Cohort | Kind | Ticks | p50 | p99 | max | mean |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | Maintenance | 59 | 9.399 | 12.864 | 12.864 | 9.443 |
| A1 | Other | 4141 | 4.161 | 6.437 | 8.815 | 4.187 |
| B1 | Maintenance | 59 | 8.522 | 10.792 | 10.792 | 8.634 |
| B1 | Other | 4141 | 3.841 | 6.922 | 9.933 | 4.016 |
| B2 | Maintenance | 59 | 8.446 | 10.403 | 10.403 | 8.531 |
| B2 | Other | 4141 | 3.743 | 6.988 | 13.188 | 3.931 |
| A2 | Maintenance | 59 | 9.695 | 12.829 | 12.829 | 9.543 |
| A2 | Other | 4141 | 4.826 | 6.785 | 11.589 | 4.687 |
| A3 | Maintenance | 59 | 9.486 | 12.486 | 12.486 | 9.650 |
| A3 | Other | 4141 | 4.666 | 7.071 | 8.607 | 4.695 |
| B3 | Maintenance | 59 | 8.448 | 14.016 | 14.016 | 8.599 |
| B3 | Other | 4141 | 4.420 | 7.043 | 9.756 | 4.353 |


Primary slowest 1% (42 ticks per cohort)

| Cohort | World mean | Simulation mean | Socket mean | Maintenance % | Post-report % |
| --- | --- | --- | --- | --- | --- |
| A1 | 9.856 | 6.036 | 3.820 | 100.000 | 0.000 |
| B1 | 9.083 | 4.682 | 4.401 | 85.714 | 0.000 |
| B2 | 9.079 | 4.754 | 4.325 | 90.476 | 0.000 |
| A2 | 10.230 | 6.796 | 3.434 | 95.238 | 2.381 |
| A3 | 10.129 | 6.631 | 3.498 | 100.000 | 0.000 |
| B3 | 9.031 | 5.158 | 3.873 | 95.238 | 2.381 |


Separate coarse diagnostic slowest 1% (27 ticks per cohort)

| Cohort | Socket | Broadcast | Bracketed physics | Admission | Unbracketed simulation |
| --- | --- | --- | --- | --- | --- |
| A1 | 3.593 | 4.193 | 0.610 | 0.000 | 1.220 |
| B1 | 4.192 | 2.979 | 0.625 | 0.057 | 0.730 |
| B2 | 4.140 | 3.423 | 0.706 | 0.000 | 0.831 |
| A2 | 3.698 | 4.465 | 0.701 | 0.000 | 1.245 |
| A3 | 3.505 | 4.809 | 0.684 | 0.000 | 1.302 |
| B3 | 3.545 | 3.779 | 0.853 | 0.000 | 0.924 |


Client headroom and cadence

| Cohort | Client cores | Max worker CPU p99 % | Worst 1-s input late p99 ms | Max pass ms | Sampled max receipt age ms | Kernel queue max B |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | 1.220 | 47.142 | 1.672 | 4.663 | 147.608 | 13130 |
| B1 | 1.193 | 45.781 | 2.803 | 10.807 | 153.012 | 13070 |
| B2 | 1.180 | 44.159 | 7.835 | 18.795 | 212.382 | 14259 |
| A2 | 1.191 | 46.248 | 2.184 | 5.077 | 104.519 | 22657 |
| A3 | 1.287 | 48.391 | 2.160 | 5.488 | 188.289 | 25455 |
| B3 | 1.306 | 51.241 | 1.695 | 4.590 | 127.122 | 23267 |


Separate continued-world syscall brackets (all ticks, not tail-conditional)

| Cohort | Read | Write | Readiness poll |
| --- | --- | --- | --- |
| A1 | 0.125 | 1.450 | 0.155 |
| B1 | 0.110 | 1.322 | 0.158 |
| B2 | 0.127 | 1.541 | 0.154 |
| A2 | 0.120 | 1.409 | 0.149 |
| A3 | 0.133 | 1.573 | 0.174 |
| B3 | 0.135 | 1.599 | 0.185 |


One separate current-main nested diagnostic (inclusive scopes)

| Scope | All ticks | Maintenance | Other | Slowest 1% |
| --- | --- | --- | --- | --- |
| Preparation | 0.184 | 0.773 | 0.176 | 0.770 |
| Snake visibility/encoding | 0.690 | 0.710 | 0.690 | 0.754 |
| Recipient food | 0.274 | 1.358 | 0.259 | 1.338 |
| Membership refresh/ownership (within food) | 0.031 | 0.720 | 0.022 | 0.693 |
| Subscription union (within preparation) | 0.015 | 0.317 | 0.011 | 0.301 |
| Subscription advance/post-physics | 0.070 | 0.658 | 0.062 | 0.639 |
| Frame emission/grouping | 0.171 | 0.284 | 0.170 | 0.298 |


## Interpretation and next leads

On the primary slowest 1%, main's socket task consumes **3.873–4.401 ms/tick**
and simulation **4.682–5.158 ms/tick**. In the separate coarse tails, socket work
is **3.545–4.192 ms**, broadcast **2.979–3.779 ms**, all bracketed physics
**0.625–0.853 ms**, and unbracketed simulation **0.730–0.924 ms**. These diagnostic
conditional means have their own tick ranks and cannot be substituted into
primary p99. Post-report ticks account for 0–2.4% of main's primary tail;
maintenance, not report serialization alone, is the repeating association.

The separately warmed current-main nested diagnostic has 2,700 fine ticks,
including 37 maintenance ticks. Its class, client, queue and isolation checks
pass in both coarse and fine windows. It costs 2,769.607 CPU seconds (0.769 hours),
including its 100-second reserve, below the separate 7,200-second allowance.
New per-recipient timers add overhead; these inclusive scopes are diagnostic
budgets, not savings estimates or a replacement capacity result.

The next portable exact leads, ranked by resolved scope and remaining work, are:

1. **Reuse held-listing storage during membership refresh.** This fallback costs 0.720 ms per maintenance tick, within 1.358 ms
   for all recipient food. The current path already moves unchanged Arc handles, but still takes the old
   vector, reserves a new one and rebuilds it. An in-place sorted merge or recycled capacity could
   reduce allocation and ownership work while preserving clears before additions,
   ascending order, old-listing fallbacks and recipient independence. The entire
   0.720 ms is an upper bound; none is demonstrated removable here.
2. **Update the shared subscription union by the old/new symmetric difference.**
   `FoodSubscriptions::sync` costs 0.317 ms per maintenance tick, nested inside
   0.773 ms preparation. It currently drains every old held sector and re-adds
   every current sector for a changed subscription. A sorted merge could update
   changed counts while preserving all entered/retired refresh bookkeeping,
   including transient overlap effects, and the strongly held revision identity. Again, scope cost is an upper bound.

The subscription/post-physics scope still costs 0.658 ms on maintenance ticks;
#277's rejected retained-sector layout remains rejected. Another 0.639 ms of
recipient food lies outside membership refresh and includes diff lookup, packet
cloning and other fast-path work. That is an unresolved budget, not an isolated
packet-copy saving. Snake visibility/encoding is 0.710 ms on maintenance ticks.
The gateway remains large: separate all-tick write brackets cost 1.322–1.599 ms
on B, versus 0.154–0.185 ms for readiness polling. These syscall windows are not
tail-conditional. This study supplies no newly qualified exact gateway change
and does not reopen the rejected epoll candidate. No lead is implemented.

Across the six primary holds, simulation keeps roughly 142.85 ticks/s with zero
deferrals. Socket sample populations and living/occupied census ranges are 499–500. Mean body lengths span
53.722–59.890, p90 172–196 and p99 244–256. Live radius spans 14,709.5–14,951.7 world units, food
26,636–30,587 and prey 20–35; these are not forced equal between cohorts. There is
no measured write WouldBlock,
buffer-full event, admission refusal or unexpected established-session failure.
Application send queues sample zero; kernel queue maxima are 13,070–25,455 bytes
and slopes are -134.152 to +112.865 bytes/s, far below the frozen growth allowance.
Full-hold boundary audits also pass.

Client CPU averages 1.180–1.306 cores across three workers; the largest worker CPU
p99 is 51.241%. Worst sampled one-second input-lateness p99 is 7.835 ms, below the
42-ms control interval. Frame/byte and lifecycle counter deltas are in results.json;
they use one-second report endpoints, not exact primary boundaries. Receipt age
is an occasional maximum sample (104.5–212.4 ms), not a per-frame inter-arrival
or latency distribution. The existing clients do not retain that distribution.

## Verification, reproduction and access

Both host copies, the common client, eight-client smokes and the 500-client pilot
built and passed on the guest. Every production Rust source byte and generated
hook is reconstructed from its pinned archive/harness; the shared client is
unchanged. CPU-reconciliation, client-saturation and queue-growth mutations fail
at their intended gates. Synthetic arithmetic, food-flag, phase-sum,
primary-detail-contamination and four quality-combination controls pass.
No runtime code changes here, so a new full Rust or 27-trace matrix is not claimed.

[REPRODUCE.md](REPRODUCE.md) gives the commands, source reuse, explicit receipt
writer, controls and bounded target cleanup. [tables.py](tables.py) recomputes
these tables from [results.json](results.json), [tail-results.json](tail-results.json)
and [fine-results.json](fine-results.json). Verification receipts are in
[verification.json](verification.json). Whole archive, instrumented Rust-source and binary
identities are retained without per-file source-hash maps. Calibration data is
in [isolation-calibration.json](isolation-calibration.json); its frozen gate is
[isolation-gate.json](isolation-gate.json).

Raw logs, tick samples, failed attempt, full process-command inventories, sources
and binaries are **local-only**. They have been archived and verified against 5,373 file contents before this
task's guest files are removed; `archive-receipt.json` records the aggregate archive.
The public record contains selected aggregates and their recomputation only.
The reporting-only [amendment](REPORTING-AMENDMENT.md) follows #279: research
index row, no changelog entry. No production change or merge is proposed. The public plan has one privacy-label
redaction; its original preregistration history is retained privately, with every
measurement field unchanged. See [the privacy check](plan-privacy-verification.json).
