# Native latency stages

The desktop window path can record passive scalar CPU stage observations. From a
clean, pinned build, set these before starting the desktop executable:

```powershell
$env:SLITHER_LATENCY_TRACE = 'C:/new/output/latency.jsonl'
$env:SLITHER_LATENCY_REVISION = (git rev-parse HEAD)
```

Use a new output filename and an existing parent directory. The revision is
launcher-declared provenance, not binary authentication; retain the executable
hash and any source diff separately. Close the app to write once. Unset
`SLITHER_LATENCY_TRACE` to disable recording. No window, connection or synthetic
input is started by enabling it. The separate `--headless` renderer does not use
this desktop recorder. Existing feel-trace v3 is unchanged.

The UI, readiness worker and connector each preallocate and touch 262,144
scalar records. Recording does no formatting, file I/O, buffer growth, or
locking. The pending send join queue is separately bounded to 32 effects.
Full buffers count every
dropped record and continue normal gameplay; there is no subsampling. Shutdown
joins the native workers before writing. Abnormal host returns write an
incomplete sidecar; process termination may leave no file. Overflow, outstanding
work, missing completions and failed sends invalidate analysis. Long sessions
should be bounded to fit the buffers; a full buffer never changes client policy.

## Format and joins

`latency-trace-v1` is JSON Lines: a header, scalar events, and an end record with
the exact record count. The header carries run identity, clock domain, declared
source revision, thread count, completion and dropped count. Events inherit that
run/domain and carry `thread`, contiguous thread-local `event`, `generation`,
`stage`, and decimal u64 strings `ns`, `id`, `related`, `detail`. Thread buffers
are serialized separately; use event order within each thread, not file order
across threads. Each ends with `thread_end`.

IDs are scoped by thread and connection generation. Effect IDs are the existing
effect/control transaction IDs; picture IDs are existing prepared frame IDs.
Messages, polls, raw reads, input callbacks, acquisitions and submissions get
process-unique observation IDs. That lets a message retain reads from the
connector thread across the HTTP upgrade. They never enter the game protocol.

- `ready.id` is the first pending readiness observation's epoch offset. `poll`
  has a unique ID; `poll_ready.related` joins that wake across threads. Repeated
  pending notifications retain the first observation. `ready.detail` records
  readable/writable interest bits 1/2; writable-only wakes are excluded from
  ready-to-read distributions. Connector wakes are not
  socket readability. A wake without a subsequent read has no ready-to-read sample.
- `read.related` is the poll ID; `handshake_read` retains connector reads before
  a poll exists, including possible buffered WebSocket bytes after the upgrade; `detail` counts successfully read bytes.
  `message.related/detail` bound its candidate first/last raw reads. The opaque
  WebSocket buffer can retain part of the previous read, so the lower bound is
  deliberately conservative. Partial frames retain
  that span across polls; buffered messages can share reads. These spans express
  batching uncertainty, not one arrival time per message. `deferred`,
  `apply_begin/end` retain the original message ID and completion timestamp.
- `build_begin/end.related` is the last consumed message watermark, including
  the message being atomically applied for a Setup draw. Zero means no packet.
  `issued`, `retried`, `superseded`, `skipped`, and `presented` join the original
  picture. `issued.related/detail` link the acquisition ID/generation (which may
  precede a Play in that redraw). Resource sync, every submit bracket, pre-present notification and
  present bracket join that issued picture; submission IDs are `related`.
- `callback` precedes routing; each gets `admitted` or `suppressed`. Replaced
  pending intents get `coalesced` (pointer/turning-key/boost channel in `detail`).
  Focus loss is suppressed because existing blur policy changes no held state.
  `decision` uses the effect ID, the relevant admitted input in `related`, and
  keyboard/ping/boost/heading phase in `detail`. `decision_input` retains the
  additional turning-key input contributing to heading. These are retained
  intent dependencies; a held input can contribute to several commands. Pointer
  and held turning-key source IDs survive connection reset with their policy
  state, so input joins can cross generations in the same run.
- `accepted` means tungstenite retained an effect's bytes. `backpressure` can
  precede acceptance or follow it during flush; it never means flushed.
  `flushed` joins each pending accepted effect only after successful flush.
  `ack_effect` is the separate exactly-once game acknowledgement. Raw `write`
  records count successful wire bytes; poll flushes use ID zero because writes
  may batch effects and automatic protocol responses. Flush is the unambiguous
  per-effect completion confirmation. Synchronous raw writes can finish before
  the host returns acceptance; flush timestamps confirm completion and can be
  later than the first OS acceptance. Implicit flushes in reads also retire
  already accepted effects. `write_blocked` records raw write backpressure.
  Effect `detail` distinguishes Connect/Send/Close (0/1/2). Window exit performs
  Close directly; `host_close` records that actual host completion, without
  inventing a game acknowledgement or changing its shutdown policy. A Send
  always requires its own acknowledgement and flush. `game_failed` invalidates
  analysis.

## Analyze and calibrate

From `client/`:

```sh
node tools/latency-trace.mjs /external/output/latency.jsonl
node --test tools/latency-trace.test.mjs
```

The analyzer rejects duplicate IDs/events, orphan joins, changed picture
watermarks, illegal ownership transitions, missing acknowledgements/flushes,
overflow and incomplete tails. It prints counts, p50, p95 and maximum in ms for
ready-to-read, read-to-apply-start, apply-to-build-end, build-end-to-last-submit,
present-call duration, input admission/decision/acceptance/flush and acquire wait.
Setup can build inside application: apply-start-to-build-end is always reported;
apply-end-to-build-end includes only pictures built after that commit. It never
sums independently pooled percentiles. Input-to-flush is joined per command.
`--cpu` permits CPU-driver messages without raw reads and acknowledgements
without GPU/present stages; it must not be used for a native completeness claim.

The existing CPU feel driver has a finite scalar calibration mode:

```sh
cargo build --release --locked -p slither-wgpu-slice --features feel-trace --example feel-replay
target/release/examples/feel-replay --latency-overhead
```

Run from the repository root, with `CARGO_BUILD_JOBS=2`. It runs five pairs of
independent CPU drivers on two worker threads, stepping
off/on frames adjacently with alternating order on the same authored schedule.
Each warms 100 frames, measures 1,200, and checks exact command bytes/order/clocks
and complete feel observations.
It reports p50/p95 of paired per-frame CPU differences, not a difference of
pooled percentiles. Channel waits, initialization, observation copying and
export are outside the timed frames. Both sides enable the existing feel
recorder. This is a CPU-driver overhead check;
it does not measure the native surface, GPU, transport or display path.

One process `Instant` epoch supplies integer ns offsets on both threads (QPC on
Windows). The existing policy f64 millisecond clock, cadence and scheduling are
untouched. Callback entry excludes device/OS queue age; readiness excludes wire
arrival; acquire wait is a CPU scheduling span. Present return and the game's
acknowledgement are not GPU completion, scanout or visible pixels. Successful
flush is OS/transport acceptance, not server receipt. Do not subtract clocks from
different processes or treat nanosecond storage as nanosecond measurement accuracy.

## Calibration snapshot (2026-10-31)

Five release pairs at 960x540 CSS, using the frozen mixed 60/170/240 Hz and
irregular-step schedule above. Each side produced exactly 262 control commands
and 2,604 complete feel observations; each on run retained 8,238 scalar records
with zero drops. The calibration process was constrained to one logical CPU
(Windows process affinity mask 1); application scheduling is unchanged.

| Pair | Paired delta p50 (ms/frame) | Paired delta p95 (ms/frame) |
| --- | ---: | ---: |
| 1 | 0.0001 | 0.1369 |
| 2 | 0.0007 | 0.1296 |
| 3 | 0.0000 | 0.1285 |
| 4 | 0.0004 | 0.1309 |
| 5 | 0.0002 | 0.1297 |

Median pair p50 is 0.0002 ms and p95 is 0.1297 ms (p95 range 0.1285-0.1369).
The 0.1 ms p95 target was **not met** by this paired CPU-frame measurement.
Average frame CPU time stayed around 0.23 ms, with both positive and negative
pair deltas; these high-quantile differences include workload/host variation
and do not isolate a universal recorder cost. Earlier unpinned measurements
also varied above the target (up to 0.1611 ms p95). They were not discarded in
favor of a passing run. Native transport/surface overhead remains unmeasured.
