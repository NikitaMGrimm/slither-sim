# Constant-boost Battledome check — frozen before paid collection

2026-10-29, follow-up to #262. Runtime remains
`165a5bb764aba6cbf39f42503e090f9b5e3c4d22`, portable x86-64 Rust 1.99 locked
release, same shipped responsive Battledome preset, seed 7, 641 remote slots,
zero in-process bots and no artificial network latency. No production or shared
policy changes. A4 chooses every heading; the study client overrides only its
boost bit when received rules say free boosting at the floor. Missing/ordinary
rules preserve its original output. A native regression must preserve heading
bits, gate the override and leave the shared policy source unchanged.

One c7a.medium Spot server and one eight-vCPU Spot generator in the same
Frankfurt AZ; private bot/control/artifact traffic, all host tasks on one vCPU,
six client workers on cores 0–5 and control on 7. Existing source-restricted
management/game access remains private. No use of the occupied SSH bench guest.
No local builds in this collection; later code checks use at most -j 1 locally.

Budget $2 for this follow-up, one pair with no automatic replacement. Planned
duration at most 45 minutes, both boot-installed terminate timers 75 minutes
(plan +30), terminate-on-shutdown, one-time Spot/terminate-on-interruption,
DeleteOnTermination root disks. Bid ceilings $0.06/h + $0.30/h bound full-timer
compute at $0.45; reserve $1.40 for disks, IPv4 and bounded transfer, total
$1.85 maximum. Probe the existing AWS session first and periodically; stop on
expiry without refresh or workaround. Record launch quotes and all paid time.

Warm naturally from tick zero for five minutes at 80 real protocol-19 received
frame clients. Every frame is decoded and A4 decides every 42 ms. Measure 80,
160 and 320 bots in one evolving world, with 60-second settling and 90-second
marked windows, using #262's aligned simulation/socket CPU and phase hooks.
Actual living-tick boost share must be at least 70% in each accepted window;
report it and admitted boost-food births separately (free floor boost produces
no pellets). Keep all overload rows; stop further collection only for invalid
generator/queues/treatment or process failure. No descending arm or play hold.

The #262 comparison uses its matching ascending rows, not a new low-boost run.
Skipping its intermediate counts changes cohort sizes/world history; report
length, food and population census and label comparisons descriptive, without
a causal paired speed ratio, hardware extrapolation or uptime qualification.

Use canonical complete 100-tick interior blocks from the final server log,
aligned world CPU p50/p99/max, deferrals, phase/cadence context, whole client
report frame excess/jitter/stalls, join/respawn quantiles and pending ages,
alive/occupied seats and source/binary pins. Reconcile means to task counters
within max(0.03 ms/tick, 2%). Keep #262's generator CPU/lateness, queue and decode
gates; do not loosen them. Record all deferrals outside marked windows too.

Carry forward the repaired newline reader, archive-ready publication and source
cleanup guard. Download only after close/gzip/SHA-256 and atomic readiness;
verify local size, digest and required evidence before deleting source instances.
No play hold: tear down after verified retrieval. Cancel task Spot requests by
recorded instance/request IDs, delete task keys/groups and volumes, then scan
all enabled regions for zero nonterminated instances, active/open Spot requests,
volumes and Elastic IPs. Preserve disabled/inaccessible coverage as unavailable.
Never touch protected unrelated resources. No identifiers or addresses in public
text. Retain raw logs, keys and cloud receipts privately; publish selected
results and only changed harness code in a short follow-up/addendum PR.
