# Small-count turn-offset capture: stopped at exclusivity preflight

**Decision: inconclusive. No game connection or invocation was launched.** The
single approved discovery request succeeded, but the ensuing exclusivity check
found **one existing non-task connection to the bound server**. The frozen
protocol makes an interference/exclusivity failure a permanent stop. The study
retained that outcome and did not change endpoints, close somebody else's
connection, reserve a development invocation or attempt a replacement.

| Outcome | Count |
| --- | ---: |
| Official discovery requests | 1 |
| Listed / active entries in that response | 141 / 35 |
| Reserved official invocations / maximum | 0 / 4 |
| Task game-socket attempts | 0 |
| Primary cells acquired / planned | 0 / 4 |
| Optional 174 cells acquired / planned | 0 / 2 |
| Held-out invocations | 0 |

There is **no new evidence rejecting D111 or any fixed offset k=6..10**. The
previous long-174 rejection of original-count D106 remains as reported in the
[count-phase study](../2026-10-28-turn-count-phase/REPORT.md), conditional on its
angle/grid and timing assumptions. This preflight outcome adds no turn-rate
measurement and supports no mechanics correction. Simulator behavior and fixtures
are unchanged.

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
No favorable reruns or replacement invocations are allowed. No ordinal was
reserved in this execution because the discovery guard stopped first.

The frozen rejection rule requires **both** held-out invocations to have complete
primary coverage and agreement. D111 is rejected in favor of an offset only if
each invocation excludes D111 and the same k survives every primary cell across
both. Every k is rejected in favor of D111 only if D111 survives all primary cells
and each k is excluded in each held-out invocation. Optional 174 cannot gate or
replace primary coverage. Every other outcome, including this one, is inconclusive.

## Observed preflight and retained evidence

The exclusive guard found no matching other Node/Python collector at its initial
process screen. It allowed one inert discovery-list request using the existing
authored decoder, without requesting or executing official client JavaScript and
without discovery-time game sockets. The retained timing entry was present and
active, so the protocol's preferred binding was used; no fallback was attempted.
Listing as active does not establish admission, protocol 19 or current reachability.

After discovery, the guard found an established non-task connection matching the
bound game endpoint. It stopped before launching `capture.mjs`, wrote the permanent
study-stop record, retained stdout/stderr and the child process outcome, and released
its own lock. A subsequent read-only check confirmed the existing connection and
the absence of an official invocation ledger, task guard and task lock. No other
process or connection was altered. The model decision is therefore
`inconclusive-development-coverage`; there was no held-out checkpoint or collection.

All six planned cells are explicitly marked **not acquired** in
[selected-results.json](selected-results.json). There are no gameplay archives,
scored intervals, count observations, recipient comparisons, deaths, overshoots
or model residuals to summarize. Missing cells are not zero-error measurements.

The complete discovery response and decoded list, private endpoint binding,
request reservation, guard/process logs, stop record and preparation failures stay
in the external study archive. Only bounded counts, decisions and hashes are
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
  geometry, formula and clock tests** using that binary. **Zero new game replays**.
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

Reproduction of offline checks, with shell affinity set to one available CPU and
Cargo/Rayon/test thread counts set to one:

```powershell
$study = 'research/fidelity/2026-10-28-turn-offset-capture'
node --test --test-concurrency=1 "$study/capture.test.mjs"
node --test --test-concurrency=1 tools/fidelity/slither-headless.test.mjs
uv run --offline --no-project python -B -m unittest discover -s tools -p test_documentation.py
# Build the prior standalone harness into a disposable external target.
$prior = 'research/fidelity/2026-10-28-turn-count-phase'
cargo build --offline --locked --release -j 1 --manifest-path "$prior/harness/Cargo.toml" --target-dir "$external/client-target"
$env:TURN_COUNT_BINARY = "$external/client-target/release/turn-count-research.exe"
uv run --offline --no-project python -B -m unittest discover -s "$prior" -p test_analysis.py
```

The external target and dispensable exported inputs were removed after validation.
The selected JSON combines the unchanged scorer's empty-invocation output with
the bounded discovery/guard outcome; its base analysis hash and that assembly step
are explicit. No formula, candidate, interval duration, gate or scoring rule was
changed after discovery.

Collection remains stopped under this protocol. A future acquisition needs an owner
decision about the failed exclusivity gate; this report does not authorize a retry.
