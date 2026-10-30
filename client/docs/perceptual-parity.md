# Perceptual parity

Slither-client aims to feel and look like the official browser client to a
normal player. Browser implementation details, byte-identical visual telemetry
and pixel-perfect rasterization are not acceptance goals. Prefer simple visual
models with explicit tolerances. The owner's play is the final acceptance;
passing automated budgets alone does not establish perceptual parity.

Protocol bytes, packet-driven game decisions and controls actually sent to the
server remain exact. Visual error must never change a sent angle byte, a
dead-zone branch or a cadence decision, including when camera projection feeds
pointer input. Keep exact command fixtures at these boundaries.

The current replay comparator remains exact until its migration lands; the
budgets below define the intended policy, not its current executable behavior.

## Channel budgets

These are initial drift budgets, chosen before implementation. Compare vectors
by Euclidean distance and headings by shortest wrapped difference. CSS pixels
refer to the displayed image after backing-size conversion, with declared zoom
and viewport geometry retained for both peers.

| Channel | Acceptance |
| --- | --- |
| Protocol and game decisions | Exact bytes, ordering, decoded outcomes, identity/lifecycle, score/result integers, requested session/quality decisions and send success. Received packet coordinates get no tolerance. |
| Server-visible controls | Identical command bytes/boost, sends/order and frozen-clock timing: 0 virtual ms at steering and other cadence gates. Actual server arrivals require separate measurement. |
| Input/event schedule | No missing, extra or reordered source events; 0 ms scheduled-time difference; exact admitted wire-producing coordinates/keys/buttons. Semantic target roles may replace DOM selectors. |
| Own head, camera, body centerline, pupils | p95 ≤0.5 CSS px; max ≤1 px. Check projected head and camera separately to prevent cancellation. Antenna max ≤2 px. |
| Heading/gaze | p95 ≤0.25°; max ≤0.5°. Antenna bulb max ≤2°. Sent heading byte stays exact. |
| Zoom | Absolute difference ≤0.0005 AND relative ≤0.1%, with implied viewport-edge displacement ≤1 CSS px. |
| Layer alpha/normalized visual phase | p95 ≤0.01; max ≤0.02; hidden/fully shown endpoints and visibility lifecycle remain exact. Text coverage instead uses max edge-alpha difference ≤0.08 and run ink-alpha sum within 20%. |
| Animation/UI transitions | Picture phase within one scheduled frame, capped at 20 ms. No early semantic enable/send/focus; logical durations/holds stay unchanged. Compare irregular frames at frozen clocks. |
| Samples, update/draw boundaries, successful pictures | Exact counts against the frozen schedule, including Setup, resize, retry and end sentinel. Distinguish prepared candidates from acknowledged pictures; internal ring/quad counts need not match. |
| Render primitives/no-op draws | Equality is not a gate. Flag changes exceeding 5% or 8 draws, whichever is larger; declared zero-alpha deletion is acceptable with unchanged visible layers/coverage. Resource ceilings remain hard limits. |
| Text/menu geometry | Baseline, box and hit edges within 2 CSS px; run widths within 15% for chosen replacement fonts; announcement extent within 4 px. No clipping, overlap or truncated important text; controls/accessibility remain functional. |

## Drift detection

Compare every required boundary without best-fit alignment, trimming, dropped
samples, resampling or time warping; report max, p95, RMS, signed mean, first
failure, threshold and sample count, retaining all violations privately.
Fail semantic mismatches or any exceeded numeric limit, evaluating each metric's
listed limits independently; flag bias above half that metric's hard maximum for at least
500 ms, and fail when last-quarter RMS exceeds first-quarter RMS by more than
half that maximum (both quarters must contain at least 10 eligible draws).
Bias/RMS rules apply only to numeric metrics with a nonzero tolerance; exact,
semantic and count channels retain their exact checks, and same-peer repeats
use these same envelopes.

## Evidence boundaries

Use authored synthetic offline cases first, representative actual-size images
and short clips, followed by owner play through start, steering/boost, chooser,
death and replay. Record versions, fonts/art and geometry for reproducibility
without requiring matching visual hashes across platforms. Preserve historical failures
and private recordings outside Git.

Retain event IDs and monotonic timestamps for input admission, control decision,
transport send, packet receipt/application and picture prepare/acknowledgement.
CPU acknowledgement is neither GPU completion nor display; synthetic clocks and
unrelated host clocks cannot establish physical input-to-server or
packet-to-screen latency. Physical IME, accessibility and display pacing remain
unverified until exercised. This policy authorizes no outside-server collection.
