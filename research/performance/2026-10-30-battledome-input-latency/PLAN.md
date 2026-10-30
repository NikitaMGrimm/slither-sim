# Frozen Battledome input-to-effect latency plan

Question: elapsed time from a protocol-19 steering send to receipt of the first
own-snake wanted-angle report reflecting it, and its change with load/backlog.
Also investigate #281's excess/stalls without attributing them from CPU alone.
Research only; no changes to runtime source, server instrumentation or spawn rules.

Pin runtime `9cd6ba6922c0a63b525ba88b69f86c8558142596` (main including #278,
#274/#272/#273 and documentation changes). Default portable release, locked
dependencies, Rust 1.99, empty Rust flags. Archive/hash runtime, client/harness
sources and binaries. Require `src/` to match the pristine runtime archive.
All native work and analysis run on AWS, never locally or on the shared guest.

## Rows, scheduling and observation

One c7a.medium Spot target, its one vCPU shared by all host tasks; a separate
same-AZ c8a.4xlarge Spot generator, six fully decoding load workers on cores
0–5, probe on core 6 and controller on core 7. Private game traffic. Fresh task
security group, never reuse the preceding study's retained group.

Fixed order: probe alone, then total seats 80, 160, 320 (one probe plus 79,
159, 319 constant-boost A4 clients). Host uses shipped Battledome arena/delivery,
seed 7, players.max=641, zero in-process bots and default zero player latency.
Probe alone: settle 30 wall seconds, count 360. Restart the host afterwards;
load 79 clients, warm 300 wall seconds, count 360 at 80 total, then add to 160,
settle 60/count 360, then 320, settle 60/count 360. Keep complete transitions.
No descending or repeated rows, capacity inference or replacement of slow rows.

At least 300 successful, unambiguous probe echoes per row are required. Count
every attempt, failed send, death, timeout and unobserved endpoint; never extend
a window or silently omit a failed row to reach the quota. A short non-primary
loopback/remote smoke test is allowed before counting; retain failures.

The probe uses the shared ClientState decoder and A4 received-frame policy,
stock elapsed framing and challenge/login. Every binary frame is decoded.
Normal decisions every 42 ms; received-rule-gated boost as in #263. At independent
seeded uniform 0.25–0.65 s intervals, perturb the current A4 target by a seeded
signed 0.25–0.65 rad, select a heading byte whose expected wanted byte differs
from both the last sent and latest received target, and send it. Hold that target
until explicit echo or 1 s timeout; keep reading/pinging, then resume A4. Permit
only one outstanding probe. Death/disconnect cancels it and is counted separately.
Random deadlines are serviced independently of receipt/decision boundaries.

Start timestamp: monotonic clock immediately before the WebSocket send call;
retain its return timestamp and send lateness. End: monotonic timestamp taken
immediately after socket.read returns the first frame containing the explicit
own wanted byte equal to the expected target. Parse every own motion form with
the shared decoder's length rules; anchors/current-heading movement, appearances,
opponent fields and matching held state are insufficient. Own forms omit the ID.
Expected byte follows command midpoint decode, 251-step quantization and 256-step
motion encoding. Tests cover every command and packet form, stale/foreign/malformed
signals, supersession, timeout and death. No prediction or authoritative view.

Report p50/p95/p99/max via nearest rank for successful send-to-echo milliseconds,
send-call duration, sample counts/censor reasons, decision/send/poll lateness,
frame gaps and the original excess/residual definitions. Buffer detailed probe
records during counting and write at exit. Retain the original bytes privately.

## RTT, generator delay and deferrals

Measure independent WebSocket control Ping/Pong with unique tokens, no more than
one outstanding, about once per second; retain stock game ping/pong separately.
Also run ICMP ping at 5 Hz on the same private route. ICMP RTT is a network/kernel
proxy; control RTT includes gateway scheduling; game RTT includes tick processing.
Publish all three, and per-sample latency minus nearest ICMP/control RTT within
2 s; keep negatives and missing matches. Do not call a difference pure server CPU.

Capture the probe TCP flows on the generator with tcpdump, full payloads, per-life
source-port filters, packet timestamps and explicit capture/drop statistics.
Reassemble server WebSocket frames; bind captured payloads to received records
by digest and order. Compare packet-completion time to userspace receipt to bound
generator scheduling/reading delay. Mark missing/capture-truncated matches rather
than inventing timestamps. Capture runs on the generator only, not the target.
Clock-step/offset checks are required for wall-clock joins; primary send-to-echo
uses one monotonic clock and needs no host clock synchronization.

Use the shipped CLI `host --stats 0.1`, timestamped by an external reader. No
server-side hooks. Classify a probe as "deferred bucket" when its send-to-end
interval overlaps a stats interval reporting nonzero deferred ticks; a "normal
bucket" has complete coverage and all overlapping intervals zero. Report coarse
100-ms bucket limits and unmatched boundary samples. This is association, not an
individual tick attribution. If no deferrals occur, report that comparison absent.
Retain stats tick rates, process/task CPU and host/generator scheduling telemetry.

Load workers must pass #275's unchanged headroom/input-lateness gates (worker CPU
p50<70%, p99<80%, input p99<42 ms), no thin mode/socket errors and received boost
duty >=99%. Probe must have no decoding/write errors and capture loss/matching
limits must be disclosed. Failed qualification invalidates a row but is retained.

Reanalyze #281's verified 320 archives on AWS after counting: align census,
busy-wall/tick CPU, worker scheduling and existing excess/stall summaries. Explain
which metric behavior is established by its definition (running-minimum accumulated
wall-minus-header residual, not a per-input latency) and which observed delays are
supported by packet/user timestamps. Historical unrecorded packet timing cannot
be reconstructed; distinguish the new diagnostic from attribution of old events.
Rank measured latency leads, with explicit inference/uncertainty; implement none.
If a server hook becomes necessary, propose it and wait for maintainer approval.

## Resource bound and delivery

Hard combined cap $2.50 across all attempts. Profile `slither-bench` only,
Frankfurt Spot, project=slither-sim tags at creation on instances, volumes, key
and group. Terminate-on-shutdown, DeleteOnTermination and verified in-guest
120-minute timers (90 planned plus 30 safety), no extension. Bid caps target
$0.06/h and generator $0.50/h; 122-minute conservative launch bound plus $0.65
noncompute reserve is $1.789, below cap. A replacement is allowed only if spent
cost plus its full timer/reserve stays below $2.50; no On-Demand authority here.
Never modify protected/unrelated resources. Preserve every failed attempt.

Close/member-verify/hash archives before publishing readiness; retrieve and verify
size/SHA and required sources before deleting sources or terminating. Final audit
of all enabled regions: task instances, active/open Spot requests, volumes, keys,
groups and Elastic IPs; disclose unrelated resources without changing them.
No play hold. No identifiers, addresses or host names in public text.

Dated study, selected values and recomputing helpers; reuse earlier tools by link.
Probe source is a reusable research helper staged as a Cargo example remotely,
excluded from the shipped crate. Research index row only, no changelog entry.
PR template Change/Validation/checks and one final assistance footer; do not merge.

## Prospective interruption recovery, before replacement timing

The first generator was reclaimed after counting and before final retrieval.
Keep its verified solo/80/160 checkpoints as the primaries; its unretained 320
is unavailable. Use one replacement Spot generator, within the original cap,
to replay the loaded world's same scheduled prehistory: 300 s at 80, 360 s at
80, 60+360 s at 160, 60+360 s at 320. Only the final 320 window is a replacement
primary; the repeated 80/160 windows are calibration. No primary is chosen by
its numbers. Keep the same pristine runtime, native probe and load client;
repair capture shutdown for future collection, retaining original capture limits.
Keep the original target and its unchanged deadline. Do not extend either timer.

Before any replacement timing, a completeness audit found that those checkpoints
lack the original generator-wide qualification and ICMP logs. They are retained
as supplementary/unqualified data, not primary rows. This supersedes the preceding
recovery selection: repeat all four primary rows on a fresh Spot target paired
with the replacement generator, with the same frozen windows and definitions.
The combined full-timer bound including the first pair is $2.352. Retire the
original target after its closed logs are retrieved; extend no timer. A complete
new 90+30-minute pair preserves the export reserve within the unchanged $2.50 cap.
For recovery durability, secure the closed native source/binary/proof archive
before timing. Each completed row additionally closes a checkpoint containing its
probe/captures, generator qualification prefix, ICMP prefix and a member-verified
snapshot of existing CLI logs. Download and hash-verify each immediately, keeping
another copy on the target. These callbacks run between counted windows and do
not alter a window, quota or selection rule. Use immediate-mode packet capture
with a short post-run drain; retain partial old capture tails as unavailable.

Before the next timing, attempt 2's exit/coverage audit rejects its 320 row:
one load worker exited on a transient empty target file before that window.
Keep the retained lower primaries and this failed 320. Atomically replace target
and RESET files, without changing native programs. Repeat the loaded 300/360 s
80 stage, 60/360 s 160 stage and 60/360 s 320 stage on the same fresh pair;
only 320 is a replacement primary, the first two are calibration. Extend no
timer or budget. Qualification requires all six workers to cover the whole row,
not merely a sufficient count of earlier reports.
