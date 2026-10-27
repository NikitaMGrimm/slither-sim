# Host frame timing, 2026-10-27

Exploratory loopback diagnosis at source `20e5064378ae8cabc7ca50cceacdbe339e6846b4`.
No outside collection. Windows and Ubuntu WSL2 on the same shared desktop;
all study processes restricted to logical CPUs 0 and 1, builds at most two jobs.
Do not use owner ports 8766/8780. Use 8790 onward, sequential hosts.

Question: does OS waiting add uneven delivery despite steady average ticks?
Alternatives: physics/encoding spikes, socket release polling, receiver scheduling,
or legitimate sparse frame production. Frame gaps alone cannot distinguish these.

Workload: Battledome, 64 background A4 bots, two bot workers, responsive delivery,
one stock protocol-19 loopback recorder without the physical-food opt-in. Seed 7.
Start with a 40-second diagnostic run per platform, omit the first five seconds
only in the primary steady-window summary, retain all raw bytes and failures.
Use native receiver timestamps and elapsed-header deltas, including empty gaps.
Report nearest-rank p50/p90/p99/max, gaps >14 ms (two nominal 7 ms ticks),
and adjacent <1 ms then >14 ms pairs. Also report receipt gap minus header gap
to distinguish sparse production from delivery jitter.

Temporary instrumentation records tick wake lateness, world advance including
encoding, early remote handoff, and socket release lateness against modeled due
time. Keep the instrumentation patch and raw traces outside production code.
Use uninstrumented matched runs if instrumentation changes behavior materially.
If a timing fix is justified, freeze its scope, then alternate five baseline/fixed
40-second runs per platform; retain every repetition and host process CPU time.
No physics, packet format/order, protocol-13 encoder, or delivery-model changes.
Compare retained #177/#146 official data with identical gap definitions; do not
interpret receipt gaps as a recovered official internal scheduler.

Full raw outputs/binaries are local-only under the worktree's ignored `outputs/`;
selected summaries and source hashes go beside the report. No renderer or human
smoothness acceptance is implied by loopback measurements.

Diagnostic amendment after the two natural runs: add a synthetic cadence probe
that sends the next single outstanding game ping immediately upon each pong,
including after death. This fills otherwise silent ticks without a fixed-rate
heartbeat change in the host. It isolates output waiting; compare it only with
itself, never as an official-workload parity claim. Natural runs used one reserved
remote slot rather than the default four; both use 64 background bots.

The first synthetic invocation failed before collection: Windows bind returned
OS error 10048 after the WSL run reused 8790, and the new recorder's relative
import climbed one directory too far. Both logs are retained. Subsequent runs
use distinct ports and the corrected relative import; no failed run is replaced
or counted as a timing sample.

Candidate frozen after the natural diagnostics and first completed Windows
synthetic run: replace only the socket thread's post-poll sleep with a bounded
receive on its existing simulation-delivery channel. Wake immediately for encoded
frames; keep the same next-frame due time and 1 ms socket-input polling bound.
No spin, global timer resolution, new dependencies, or world scheduling change.
Compare five alternating baseline/candidate synthetic runs per platform, 40 s
each, distinct ports; also repeat natural traffic. Primary mechanism endpoint:
matched early-handoff-to-socket-release p99; secondary: absolute receipt-gap minus
elapsed-header-gap p99 and host CPU. These are exploratory, shared-desktop results,
not an isolated native Linux or subjective smoothness qualification.

Before any candidate measurement: retain the original deadline sleep whenever a
queued frame has a finite due time. Use channel receive only while no frame is
queued. This avoids changing the OS wait primitive for scheduled future releases.
The initially compiled all-deadline receive prototype was not measured or selected.

Candidate rejected on its first Windows pair: handoff-to-release improved, but
`recv_timeout(1 ms)` slept coarsely when no outbound frame arrived; the one-ping
probe shifted from ~7 ms to ~14/21 ms reply intervals. Do not complete a five-run
adoption series for a known regression. All completed/in-progress runs remain;
the series is stopped before another invocation. The next candidate must preserve
fine-grained socket-input polling as well as notify on encoded output.

Second candidate, frozen before collection: Windows waits on an auto-reset
output event and a per-host high-resolution waitable timer; Linux retains the
channel receive. The simulation signals after publishing remote batches in its
two existing handoff paths. Keep the ordinary finite-deadline sleep and all world
logic unchanged. No spin or global timer changes. Run five alternating pairs on
each platform at 20 seconds per capture (five-second per-setup burn-in), to bound
the expanded experiment; retain separate 40-second natural and initial diagnostics.
Use four reserved remote slots for the matched series. Reject the candidate if
the synthetic pong cadence regresses again, even if output latency improves.

The v2 Windows series stopped while finalizing pair 2's baseline: a new
summary helper named `select.py` shadowed Python's standard-library `select` when
`subprocess` imported `selectors`, producing an argparse error. Renamed the helper
to `summarize.py`; no timing samples are discarded. Pair 2's baseline raw archive
and traces completed but its platform/CPU summary did not, so retain it separately
with unavailable CPU. A first resume refused its already-existing output directory,
without connecting. Resume four full pairs in a new directory and new ports, and
preserve pair 1. These are harness failures, not exclusions by measured timing.

Also run a matched 20-second natural-forager pair per platform with four reserved
slots. Keep these descriptive workload checks separate from the five-pair
synthetic mechanism endpoint. Optional separate broadcast profiling records
encoding duration directly and does not enter the paired timing claims.
