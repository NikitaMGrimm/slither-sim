# Small-count turn-offset capture: interrupted development

**Decision: inconclusive. Attempt 2's first development invocation was stopped
after 61.565 seconds by the post-launch process-exclusivity guard.** Both clients
had joined protocol 19; the actor reached count **26**, the observer **11**.
Neither primary target was attained and **zero cells** were acquired. Our two
sockets closed. The original permanent-stop rule applies after sockets open, so
invocations 2–4 were not launched and there was no held-out checkpoint.

Attempt 1's failed preflight opened no game socket. It remains unchanged in its
original external archive, [attempt-1-results.json](attempt-1-results.json), and
the original report at commit `d92f499a2ae4964ad7518ce2dc12a683dbd224f2`.
The owner explicitly authorized attempt 2 and the narrowly scoped zero-socket
preflight amendment before collection resumed.

| Outcome | Count |
| --- | ---: |
| Official discovery requests, both attempts combined | 1 |
| New discovery requests in attempt 2 | 0 |
| Reserved official invocations / maximum | 1 / 4 |
| Task game-socket attempts, both attempts combined | 2 |
| Primary cells acquired / planned | 0 / 4 |
| Optional 174 cells acquired / planned | 0 / 2 |
| Held-out invocations | 0 |

There is **no new evidence rejecting D111 or any fixed offset k=6..10**. The
previous long-174 rejection of original-count D106 remains as reported in the
[count-phase study](../2026-10-28-turn-count-phase/REPORT.md), conditional on its
angle/grid and timing assumptions. This interruption adds no qualifying turn-rate
measurement and supports no mechanics correction. Simulator behavior and fixtures
are unchanged. Missing cells are not zero-error measurements.

## Frozen question and acquisition

The owner approved a reduced experiment after the offline study: exact received
counts **37 and 100**, cruise only, both turn directions; count 174 optional within
the same bounds. The live comparison is D111 against D106 with a **single fixed
integer count offset k=6..10**, shared across sizes, directions and invocations.
Original-count D106 is a separate diagnostic, not a refitted candidate.

The [plan](PLAN.md) and [predictions](CAPTURE-PROTOCOL.json) were committed at
`fd8962419cd762952160e86942f694ac2390de7b` **before any discovery request or game
connection**. The implementation, [acquisition details](ACQUISITION.md) and
[source hashes](SOURCE-FREEZE.json) were then committed at
`2df58630fce4b7c16a0c1cbe43e399b8b1642196`, also before the request. Both commits
remain in history. The accepted #236 branch was not modified or pushed.

The owner-authorized [attempt-2 amendment](ACQUISITION.md#owner-amendment-attempt-2-before-new-requests)
was committed at `20146991ad92bdc0de6548fe524216783dca62be`; the final preflight
policy corrections were committed at `8ab600c2d5bfe331938149456f1293c184ec1d0d`,
both before attempt 2's first game request. Only the outer zero-socket preflight
policy, its tests and policy hashes changed. Capture, native-client and analysis/
scoring sources, cells, durations, uncertainty and decision rules are unchanged.
The manifest retains the original policy hashes alongside their amended versions.

The fixed predictions and duration calculation are:

```text
Q_D(n) = .033/.008 * (.13 + .87*((7-clamp(1+(n-2)/D,1,6))/6)^2)
fixed k predicts Q_106(max(2,n-k)); D111 predicts Q_111(n)
A = 2*asin(sqrt(2)/39) + 2*pi/65536 = .07263554952022173 rad
B(j) = .001 + 2*j/1000 seconds
compatible iff abs(abs(angle)-Q*T) <= A+Q*B(j)
```

Primary j is 42 ms; sensitivities are 0, 7 and 14 ms. Take the largest pairwise
separation time against k=6..10, add 25% and round upward to five seconds:

| Received count | D111 rad/s | Worst separation at j=42 | Frozen minimum arc |
| --- | ---: | ---: | ---: |
| 37 | 3.757715359 | 16.602 s, k=6 | 25 s |
| 100 | 3.146555982 | 44.590 s, k=6 | 60 s |
| 174, optional | 2.510711387 | 270.946 s, k=8 | 340 s |

The controller allows another four seconds for settling and count stability.
These are conditional separation calculations; neither event-to-send jitter nor
server clock-rate error is proven bounded by the assumed budget.

At most two connecting/open/closing game sockets and four invocations of at most
1200 seconds were authorized. Two development invocations must cover all four
primary cells in their union before an analysis checkpoint and two held-out
invocations. Ordinals are flushed before launch; failed launches consume them.
No favorable reruns after gameplay data are allowed. Attempt 1 reserved no ordinal;
attempt 2 reserved ordinal 1 before launch. Its interruption consumes that ordinal.
The other three ordinals remain unused because the post-launch stop is permanent.

The frozen rejection rule requires **both** held-out invocations to have complete
primary coverage and agreement. D111 is rejected in favor of an offset only if
each invocation excludes D111 and the same k survives every primary cell across
both. Every k is rejected in favor of D111 only if D111 survives all primary cells
and each k is excluded in each held-out invocation. Optional 174 cannot gate or
replace primary coverage. Every other outcome, including this one, is inconclusive.

## Retained attempt 1 and authorized amendment

The exclusive guard found no matching other Node/Python collector at its initial
process screen. It allowed one inert discovery-list request using the existing
authored decoder, without requesting or executing official client JavaScript and
without discovery-time game sockets. The retained timing entry was present and
active, so the protocol's preferred binding was used; no fallback was attempted.
Listing as active does not establish admission, protocol 19 or current reachability.

In attempt 1, the guard found an established non-task connection matching the
bound game endpoint. It stopped before launching `capture.mjs`, wrote the permanent
study-stop record, retained stdout/stderr and the child process outcome, and released
its own lock. A subsequent read-only check confirmed the existing connection and
the absence of an official invocation ledger, task guard and task lock. No other
process or connection was altered. The original model decision was
`inconclusive-development-coverage`.

The owner then clarified that replacement prevention applies after seeing gameplay
data and authorized attempt 2. The amended policy checks exclusivity immediately
before each invocation, waits 15 minutes after a zero-socket failure, and allows
at most four re-checks after the initial check. Exhaustion stops permanently.
Post-launch exclusivity failures retain the original immediate permanent stop.
The new attempt uses a separate external directory; the old stop was not removed.

The existing discovery response contains 141 entries, 35 marked active. Its bound
entry remains listed active in that retained response, so attempt 2 reused the
same binding and made **no new discovery request**. This reuse is not a fresh
claim about listing freshness, admission or reachability. There was no scouting
connection or endpoint fallback.

## Attempt 2: invocation and interruption

Invocation 1's initial exclusivity check passed; no waiting interval or re-check
was needed. One actor and one observer connected, with one life and one protocol-19
setup each. Both remained cruise-only. Paired eligibility occurred, but the actor
was still growing toward 37 when the process screen failed after about a minute.

The guard reported **a possible competing collector process**. Its predicate
conservatively screens Node/Python command lines for capture/repository terms;
it does not prove that a matching process contacts an official server. The guard
did not log the matching identity. A later read-only census found zero matches,
which cannot identify the earlier trigger or retroactively clear the stop. This
report therefore does **not** assert that another official collector was confirmed.

The guard wrote the permanent stop and asked our child to close. Both task sockets
closed; both archives were sealed as interrupted. The unchanged capture driver
then encountered `EEXIST` while trying to create the already-written stop record.
That secondary finalization diagnostic is retained outside Git. It happened after
archive sealing: both exact-prefix integrity receipts and source checks verify.
The interrupted invocation is unqualified and was not retried.

| Invocation | Split | Duration | Actor / observer maximum received count | Primary cells | Optional cells | Outcome |
| --- | --- | ---: | --- | ---: | ---: | --- |
| 1 | Development | 61.565 s | 26 / 11 | 0 / 4 | 0 / 2 | Post-launch process-exclusivity stop |
| 2 | Development | — | — | Not acquired | Not acquired | Not launched: permanent stop |
| 3 | Held-out | — | — | Not acquired | Not acquired | Not launched: permanent stop |
| 4 | Held-out | — | — | Not acquired | Not acquired | Not launched: permanent stop |

Pinned-client replay validates **8,513 setup/gameplay frames** and reconstructs
**2,633 snake anchors**, including **260 own anchors per recipient**. There were
8,515 incoming frames in the raw archives; the two challenge frames are excluded
from the native gameplay audit. The raw incoming/outgoing counts are 4,434/653 for
the actor and 4,081/842 for the observer. Received logical count ranges are 2–26
and 2–11. Physical anchors, dying points, fam and displayed length are retained
separately as bounded ranges in [selected-results.json](selected-results.json).
Those presentation diagnostics are not substitutions in the frozen rate formula.

All six planned cells are marked **not acquired**. No primary or optional interval
was scored, and no model residual or candidate rejection is reported. The final
decision remains `inconclusive-development-coverage`; there is neither the required
development union nor any held-out replication. Interrupted archive integrity
does not qualify an invocation or establish any server mechanic.

The complete discovery response and decoded list, private endpoint binding, both
attempts' stop/guard/process records, the invocation reservation, both full game
archives, derived rows and preparation failures stay external. Only bounded
counts, decisions and hashes are
published. Discovery-response SHA-256:
`144331b750c891b7ed6b75e8c7a1154b26e10affaca2b010ceb477ae3d1169dc`.
The public report omits addresses, account/machine details and other players'
identities. No official JS, assets, credentials or raw traffic were added to Git.

## Harness, verification and limits

The research adapter uses the existing headless Bot/Recorder, transport defaults,
forager, avoidance, login, control pacing, ping and reconnect behavior. Both actors
have cruise-only intent. `selection.mjs` supplies model-independent triplet and
decision gates; `score.mjs` applies the frozen candidates. `analyze.py` audits
archives and reuses the previous study's native harness with the locked client
revision `8d0772c015d6b9b73b80253db8c7349aa600b201`. It records logical/physical/dying
counts, fam and displayed length separately. No SlitherClient workspace dependency
or owner-checkout modification was made; the clean pinned clone remained clean.

Completed locally on one logical CPU, serially:

- **9 research Node tests**: exact-count/pair/control-age gates, overshoots and
  interruptions, both signed arcs, first chronological selection, positive observer
  qualification, fixed predictions, attempt budget and replicated decision rules.
- **104 existing collector tests**, offline doubles/loopback only.
- Locked offline native release build with `-j 1`; **8 previous native/Python
  geometry, formula and clock tests** using that binary; both interrupted game
  archives replayed through the pinned client without parser changes.
- **5 authored preflight policy cases**, with simulated monotonic time: immediate
  success, later success, four failed re-checks, no retry for generic failures,
  and a deadline-crossing clock sample without a negative sleep duration.
- Two separate offline-only guard probes: a source-preflight failure consumes one
  synthetic ordinal; with permanent-stop handling, a second launch is refused and
  the ledger stays at one. These are authored preparation tests, not official
  invocations. A missing-archive analysis reproduces an inconclusive decision.
- **7 documentation tests**, source-hash verification and diff/privacy checks.

Independent GPT 6.1 Sol review used source reads only. It found four acquisition
issues before any request: source failures needed a permanent stop, the deadline
needed checking before inventory after sleep, optional-cell agreement must not
gate primary results, and acquisition needed the scorer's stale-control exclusion.
All four were corrected before the implementation freeze; the same reviewer
rechecked them with no remaining actionable finding. It did not independently
execute tests or establish game-server acceptance. The review was not a browser,
rendering or packet-replay validation of new official traffic.

The amendment review found a two-sample timing edge that could produce a negative
sleep duration at a re-check boundary. The remaining wait is now sampled once;
the authored edge regression passes. Lock contention uses the same zero-socket
waiting path. Both corrections were committed before attempt 2 opened sockets.
The reviewer did not independently rerun tests; the final small corrections were
verified by the main agent. No further collection followed the interruption.

Reproduction of offline checks, with shell affinity set to one available CPU and
Cargo/Rayon/test thread counts set to one:

```powershell
$study = 'research/fidelity/2026-10-28-turn-offset-capture'
node --test --test-concurrency=1 "$study/capture.test.mjs"
pwsh -NoProfile -File "$study/preflight.test.ps1"
node --test --test-concurrency=1 tools/fidelity/slither-headless.test.mjs
uv run --offline --no-project python -B -m unittest discover -s tools -p test_documentation.py
# Build the prior standalone harness into a disposable external target.
$prior = 'research/fidelity/2026-10-28-turn-count-phase'
cargo build --offline --locked --release -j 1 --manifest-path "$prior/harness/Cargo.toml" --target-dir "$external/client-target"
$env:TURN_COUNT_BINARY = "$external/client-target/release/turn-count-research.exe"
uv run --offline --no-project python -B -m unittest discover -s "$prior" -p test_analysis.py
# $archiveRoot is the retained external attempt-2 directory; $output must be new.
uv run --offline --no-project python -B "$study/analyze.py" --archive-root "$archiveRoot" --binary "$env:TURN_COUNT_BINARY" --output "$output"
```

The external target and dispensable exported inputs were removed after validation.
The selected JSON preserves the unchanged scorer's invocation output and adds the
bounded attempt/preflight census and native count ranges; its base analysis hash
and that assembly step are explicit. The byte-identical attempt-1 JSON and all
original external root files were verified against their retained hashes.
No formula, candidate, interval duration, eligibility gate or scoring rule changed.

Collection remains permanently stopped after this post-launch failure. Any future
acquisition requires an owner decision; this report does not authorize a retry.
