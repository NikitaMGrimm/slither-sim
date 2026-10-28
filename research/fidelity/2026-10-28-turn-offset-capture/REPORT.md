# Small-count turn-offset capture: final inconclusive result

**Decision: inconclusive. Attempt 4 completed both development invocations but
acquired zero qualifying cells.** Each ran for 1,190 seconds and closed both
sockets normally. The frozen development-coverage gate therefore prevented any
held-out invocation. This is the final attempt: **no attempt 5**, no mechanics
correction, and no runtime or fixture changes. PR #239 is pushed and reported;
the maintaining agent handles landing.

The mandatory offline rehearsal passed and was committed before official traffic.
It demonstrated more than five minutes of positively attributed paired traffic,
a deliberate third-connection stop, interruption and sealed recordings. Official
acquisition then ran in **a capped ephemeral Linux container on a separate machine**
under the unchanged scientific protocol. No discovery request or endpoint fallback
was added. Attempt 4 was authorized on the owner's behalf.

| Attempt-4 outcome | Result |
| --- | ---: |
| Official invocations completed / maximum | 2 / 4 |
| Primary cells acquired / planned | 0 / 4 |
| Optional 174 cells acquired / planned | 0 / 2 |
| Held-out invocations | 0 |
| New discovery requests | 0 |
| Maximum simultaneous capture connections observed | 2 |

There is **no new evidence rejecting D111 or any fixed offset k=6..10**. The
previous long-174 rejection of original-count D106 remains as reported in the
[count-phase study](../2026-10-28-turn-count-phase/REPORT.md), conditional on its
angle/grid and timing assumptions. Missing cells are not zero-error measurements.

Attempts 1–3, including their stops and full archives, remain unchanged outside
Git. Their bounded results are retained in [attempt-1-results.json](attempt-1-results.json),
[attempt-2-results.json](attempt-2-results.json) and [attempt-3-results.json](attempt-3-results.json).
The attempt-1 report remains at commit `d92f499a2ae4964ad7518ce2dc12a683dbd224f2`.

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

The [attempt-2 amendment](ACQUISITION.md#authorization-amendment-attempt-2-before-new-requests), authorized on the owner's behalf,
was committed at `20146991ad92bdc0de6548fe524216783dca62be`; the final preflight
policy corrections were committed at `8ab600c2d5bfe331938149456f1293c184ec1d0d`,
both before attempt 2's first game request. Only the outer zero-socket preflight
policy, its tests and policy hashes changed. Capture, native-client and analysis/
scoring sources, cells, durations, uncertainty and decision rules are unchanged.
The manifest retains the original policy hashes alongside their amended versions.

Attempt 3 was subsequently authorized on the owner's behalf in **a capped ephemeral Linux container
on a separate machine**, with connection-based exclusivity and a fresh ledger
because attempt 2 scored zero eligible cells. The amendment was pushed to PR #239
at `7d9a11b7`; the final guard deadline correction was pushed at `8a5f943a`, both
before any attempt-3 game request. Only the outer guard/coordinator and tests were
added. Previous capture, headless, native, analysis/scoring, cell/duration,
uncertainty and decision-rule sources remain byte-identical.

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
Attempt 2's other three ordinals were unused because its post-launch stop was
permanent. The distinct attempt-3 authorization on the owner's behalf permits a fresh ledger
with the same four-invocation bounds; it did not erase or resume attempt 2.

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

The maintaining agent clarified that replacement prevention applies after seeing
gameplay data and authorized attempt 2 on the owner's behalf. The amended policy checks exclusivity immediately
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
| 2 | Development | â€” | â€” | Not acquired | Not acquired | Not launched: permanent stop |
| 3 | Held-out | â€” | â€” | Not acquired | Not acquired | Not launched: permanent stop |
| 4 | Held-out | â€” | â€” | Not acquired | Not acquired | Not launched: permanent stop |

Pinned-client replay validates **8,513 setup/gameplay frames** and reconstructs
**2,633 snake anchors**, including **260 own anchors per recipient**. There were
8,515 incoming frames in the raw archives; the two challenge frames are excluded
from the native gameplay audit. The raw incoming/outgoing counts are 4,434/653 for
the actor and 4,081/842 for the observer. Received logical count ranges are 2â€“26
and 2â€“11. Physical anchors, dying points, fam and displayed length are retained
separately as bounded ranges in [attempt-2-results.json](attempt-2-results.json).
Those presentation diagnostics are not substitutions in the frozen rate formula.

All six planned cells are marked **not acquired**. No primary or optional interval
was scored, and no model residual or candidate rejection is reported. Attempt 2's
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

## Attempt 3: connection ownership unavailable

The source copy at the pushed PR head was verified against all **22 source
hashes** in the fresh container before requests. The same private binding was
reused without discovery or fallback. The existing image was used without a pull.
Raw recordings used a bind mount so container removal could not erase them.

The initial endpoint check passed. The connection coordinator subsequently saw
zero matching connections on the controller and two on the separate machine.
The latter records contained no socket-owner identities. The guard classified
them as unattributed, recorded the census privately and interrupted the unchanged
capture driver. It did **not** establish that either connection belonged to another
collector. Available process-tree command lines were retained privately, but no
positive socket-to-process mapping was available.

The permanent-stop rule was honored after this post-launch failure. No second
development or held-out invocation was launched within attempt 3. No amendment
cleared that stop or reran its recording. Parsing tests and simulated ownership
did not demonstrate that actual owner metadata was available. That capability had
not been established within attempt 3. Attempt 4 addressed it in a separate offline
rehearsal before any further official request; it did not resume the stopped
attempt-3 ledger.

| Attempt-3 invocation | Split | Duration | Actor / observer maximum received count | Primary cells | Optional cells | Outcome |
| --- | --- | ---: | --- | ---: | ---: | --- |
| 1 | Development | 4.483 s | 3 / 2 | 0 / 4 | 0 / 2 | Ownership unavailable; fail-closed stop |
| 2 | Development | â€” | â€” | Not acquired | Not acquired | Not launched: permanent stop |
| 3 | Held-out | â€” | â€” | Not acquired | Not acquired | Not launched: permanent stop |
| 4 | Held-out | â€” | â€” | Not acquired | Not acquired | Not launched: permanent stop |

Both clients received protocol-19 setup and one life each. Raw archives contain
334/45 incoming/outgoing actor frames and 439/21 observer frames. Both interrupted
prefixes verify. Pinned-client replay validates **771 setup/gameplay frames** and
**266 anchors**, including **18 own anchors per client**. The two challenge frames
are excluded from the native frame count. Logical count ranges are 2â€“3 and 2â€“2;
other presentation counters are retained separately in the selected JSON. No target
was reached, no interval was scored, and no candidate gains new evidence.

The archive bundle was copied home and its SHA-256 compared with the remote copy:
`63178f0cebfec2a0454434318a48ce2bf7e4873f19ee4bce5703898449a4d652`.
Both recording hashes also agree with their receipts. **The task scratch directory
was removed and no task container remains. No image was pulled; the pre-existing
image was preserved.** Task-owned packing and cleanup used capped ephemeral
containers. No host configuration, installation, existing container, image or volume
was changed. Full raw data and private connection/process records remain in the
external local archive; no machine or endpoint identifiers are published here.

## Attempt 4: mandatory rehearsal and final coverage gate

Attempt 4 was authorized on the owner's behalf after attempt 3 scored zero cells;
the fresh ledger therefore did not select favorable measured cells. The amendment
and guard corrections were committed before rehearsal. The successful
[rehearsal summary](REHEARSAL.md) and [proof](rehearsal-proof.json) were then pushed
at `0273234d`, before any attempt-4 official request. The runtime gate checked
that proof against the frozen source fingerprint. All **25 manifest hashes**
match both the local source and the retained source copy from the container.
Capture, headless, native, analysis/scoring, cells, durations, eligibility,
uncertainty and decision rules remain byte-identical.

The offline release-simulator baseline lasted **316.923 seconds**, with a
**315.671-second** span of two positively attributed sockets and **307** positive
samples. Both protocol-19 setups and paired control were observed. A deliberate
third local connection in the host namespace triggered permanent interruption;
both capture sockets closed and both prefixes verified in each rehearsal leg.
The Windows conflict branch was unit-tested because the local simulator was not
exposed to the controller. These authored local records never enter candidate
scoring. The rehearsal stop/ledger state was archived and cleared only in its
own directories; the official archive started fresh.

The dedicated bridge namespace attributed our connections by construction.
Optional inode/fd identity was diagnostic only. Connection-only censuses on the
host and controller found no endpoint conflict; missing optional identities did
not stop acquisition. This boundary relied on the maintaining agent's assurance
that other collectors were idle; unrelated container namespaces were not inspected.
Both initial preflights passed, with no waiting interval or re-check. The binding
was reused without discovery, scouting or fallback.

| Invocation | Split | Duration | Maximum received count, actor / observer | Primary cells | Optional cells | Outcome |
| --- | --- | ---: | --- | ---: | ---: | --- |
| 1 | Development | 1,190.002 s | 171 / 55 | 0 / 4 | 0 / 2 | Normal close-reserve deadline |
| 2 | Development | 1,190.006 s | 114 / 77 | 0 / 4 | 0 / 2 | Normal close-reserve deadline |
| 3 | Held-out | — | — | Not acquired | Not acquired | Development coverage gate failed |
| 4 | Held-out | — | — | Not acquired | Not acquired | Development coverage gate failed |

The actor reached both primary target counts in the retained receipts, but neither
invocation produced a complete eligible arc. Neither online completion nor the
offline selector selected a cell. Natural deaths, failed holds, count changes,
overshoots and reconnections remain in the full recordings. The two recipients
had 10/15 lives in invocation 1 and 11/11 in invocation 2; these are reconnects
inside the same two bounded invocations, not replacement invocations.

All **four complete archive prefixes** and collector-source receipts verify.
Pinned-client replay validated **475,880 setup/gameplay frames** and **204,771
snake anchors**. Own anchors were 4,884/4,786 and 4,864/4,857 respectively. The
raw incoming-frame total was 475,927; the 47 challenge frames are excluded from
the native gameplay audit. Logical/physical/dying counts, fam, displayed length,
scale and scang remain separate in the selected JSON's bounded exposure ranges.
They are not alternate counts substituted into the frozen candidate formulas.

The unchanged result is `inconclusive-development-coverage`: no cell was scored,
no candidate residual is available, and there is no held-out checkpoint or
replication. The scorer's `qualified` flag checks agreement and archive integrity;
with an empty cell set it does not mean complete primary coverage. The separate
coverage gate correctly remains closed. There is no new evidence for changing
D111, adopting an offset or reviving original-count D106.

The complete official bundle was copied home and hash-verified:
`0cd74834bf882a0783b43376a011d3b98a9fda5fa0dafe762d2bc9ca1f0f7866`.
Full rehearsal/official data and private connection records remain outside Git
(access: local-only).
**The remote task scratch directory was removed and no task container remains.
No image was pulled; the pre-existing images were preserved.** Packing and cleanup
used capped ephemeral containers; no existing container, image, volume, host
configuration or installed software was changed. The local native target was
cleaned. Automatic approval review blocked deletion of disposable local rehearsal
dependency inputs, which remain outside Git; its stated reason was “blocked by policy.”

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
  geometry, formula and clock tests** using that binary; all four final-attempt game
  archives replayed through the pinned client without parser changes.
- **5 authored preflight policy cases**, with simulated monotonic time: immediate
  success, later success, four failed re-checks, no retry for generic failures,
  and a deadline-crossing clock sample without a negative sleep duration.
- **10 container-guard tests**, run locally and in the capped container with game
  networking disabled: ownership, connection bounds, monitor lease, waiting budget,
  deadline clamping and authored child interruption/sealing.
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
verified by the main agent. No further collection followed within attempt 2.

The attempt-3 source review found fixed polling/drain sleeps could cross their
deadlines. Remaining-budget clamping and an authored regression were committed
before requests. That review used source reads only; neither it nor the authored
tests established live socket-owner visibility. The missing environment capability
remains an explicit failed preparation condition.

Reproduction of offline checks, with shell affinity set to one available CPU and
Cargo/Rayon/test thread counts set to one:

```powershell
$study = 'research/fidelity/2026-10-28-turn-offset-capture'
node --test --test-concurrency=1 "$study/capture.test.mjs"
node --test --test-concurrency=1 "$study/container-guard.test.mjs"
pwsh -NoProfile -File "$study/preflight.test.ps1"
node --test --test-concurrency=1 tools/fidelity/slither-headless.test.mjs
uv run --offline --no-project python -B -m unittest discover -s tools -p test_documentation.py
# Build the prior standalone harness into a disposable external target.
$external = Join-Path ([IO.Path]::GetTempPath()) 'turn-offset-capture-check'
$prior = 'research/fidelity/2026-10-28-turn-count-phase'
cargo build --offline --locked --release -j 1 --manifest-path "$prior/harness/Cargo.toml" --target-dir "$external/client-target"
$env:TURN_COUNT_BINARY = "$external/client-target/release/turn-count-research.exe"
uv run --offline --no-project python -B -m unittest discover -s "$prior" -p test_analysis.py
# $archiveRoot contains a retained ledger and captures; $output must be new.
uv run --offline --no-project python -B "$study/analyze.py" --archive-root "$archiveRoot" --binary "$env:TURN_COUNT_BINARY" --output "$output"
```

The external target and dispensable exported inputs were removed after validation.
The selected JSON preserves the unchanged scorer's invocation output and adds the
bounded attempt/preflight census and native count ranges; its base analysis hash
and that assembly step are explicit. The byte-identical attempt-1 JSON and all
original external root files were verified against their retained hashes.
No formula, candidate, interval duration, eligibility gate or scoring rule changed.

## Known tooling defects, not fixed

Review after attempt 4 found these defects in the retired acquisition tooling. The
sources stay byte-identical because their hashes are part of the frozen record and
no further attempt will run them. None changes the reported outcome:

- `guard.ps1` and `capture.mjs` accept any truthy `freezeCommit` for held-out
  admission instead of requiring the post-development analysis checkpoint. Held-out
  admission was never reached, because the coverage gate failed.
- `guard.ps1` computes the next ordinal and checks permanent-stop markers before
  waiting for the lock, not again after acquiring it. Two concurrent guards could
  therefore reserve a stale ordinal or start after another guard's stop. Only one
  guard ran at a time in every attempt.
- An unframed challenge can leave a turn receipt with a null server clock, which
  `score.mjs` would treat as zero in its stability gate. No cell was selected or
  scored.
- `container-guard.mjs` accepts the first census numbered after child exit as
  post-close, although that census may have started before the sockets closed. That
  could cause a false permanent stop. Both attempt-4 invocations closed without one.

Fix these before reusing the tooling.

Attempt 4 is complete and collection stops at the frozen development gate.
There will be no attempt 5. This report supports no mechanics change.
