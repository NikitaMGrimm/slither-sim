# Attempts

- Required guides and prior reports read; initial AWS identity check succeeded.
  Worktree was clean at the requested source pin. No separate SSH guest used.
- Before provisioning, the local archive preparation failed under Windows'
  default legacy text encoding. Reproduction: `python bundle/prepare.py`;
  `UnicodeDecodeError` while reading the archived server. Use `python -X utf8`
  locally; Linux uses UTF-8. The failed source tree is preserved privately.
- The next preparation caught a nonunique marker instrumentation anchor (also
  present in tests). Narrowed it to the owning method's preceding line. The
  failed source tree is preserved privately; no paid run occurred.
- The corrected archived-source preparation completed locally without a build.
  The frozen plan precedes cloud resource creation.
- First native 80-seat window completed with valid clients, but expanded JSON
  phase serialization every 100 ticks dominated the slowest 1%: 11.520 ms mean
  world CPU there, including 9.999 ms simulation remainder (86.8%). This is a
  measurement artifact, not game stutter evidence. Its primary p50/p99/max were
  1.168/3.147/13.334 ms; zero deferred ticks. The 120-seat window was interrupted.
  Both partial raw streams are preserved; no outcome is silently discarded.
- Stopped collection and removed the public endpoint temporarily. On the same
  pair, moved expanded per-block CPU and census serialization outside counted
  windows, retaining timestamps and CPU-clock edges. The revised overlay buffers
  values and flushes after measurement is disabled. No game rule, policy, ladder,
  gate or production source changed. Restart from tick zero is required because
  the executable changed; no checkpoint used. The original artifact/control
  processes were stopped before reusing their ports. Moving the preserved
  directory required sudo because its parent is root-owned; the first move
  refused with Permission denied and changed nothing.
- The restart controller initially failed before making AWS calls because a
  scratch file named `select.py` shadowed Python's standard-library extension
  when `socket` imported `selectors`. Renamed the reducer entry point to
  `select_results.py`; no credential refresh or resource replacement occurred.
- The same-pair restart built successfully, then the local controller raised
  `NameError: access is not defined` before publishing its endpoint. Its
  unconditional cleanup terminated the pair. The corrected world reached about
  1,000 warmup ticks, with no counted window; unretrieved warmup logs were lost.
  This controller error is retained separately from the first logging artifact.
- After verifying complete first-pair teardown, permit one fresh replacement
  pair with the original full controller and corrected buffered overlay. There
  is no overlap of running pairs. The first pair is bounded below $0.12 compute;
  replacement full timers cost at most $0.78, plus the shared $2 noncompute
  reserve, below $3. No refreshed login, additional concurrent instance, game
  change, lowered gate or selected-speed repeat is authorized by this amendment.
- Before the replacement's first counted window, source inspection identified
  legitimate ~200 ms straight-anchor gaps. Added elapsed-header-relative client
  telemetry and retained raw gaps, with a synthetic normal-gap control. Preserve
  its warmup-only stream; restart on the same pair before any counted window.
  Corrected same-pair resume initializes access explicitly. No replacement
  measurements were discarded.
- The first failed attempt's reducer comparison initially differed only because
  JSON round-trips integer cadence-map keys to strings. Canonical JSON comparison
  reproduces values exactly. Its missing generator command inventories are
  retained as an additional limit; completed primary windows require both.
- The header-telemetry restart's first read-only SSH status call timed out.
  The old fail-fast controller entered cleanup immediately. CloudTrail identifies
  its own termination call; this is not labeled a Spot interruption. No counted
  replacement window existed. That warmup's retrieved pre-telemetry archive and
  launch/teardown receipts are retained; later unretrieved build logs were lost.
- A fresh all-region preflight again found all 17 queryable regions empty,
  including Spot requests; 17 unavailable regions remain labeled. Both earlier
  pairs are terminated and their keys/groups deleted. The final attempt adds
  bounded reconnects for read-only SSH status checks, without replaying process
  launches. Prior compute is bounded below $0.15; final full timers plus the
  original $2 noncompute reserve keep the aggregate maximum below $3.
- Serialize buffered JSON to one string before writing, avoiding thousands of
  stderr fragment writes. Client excess baselines reset before each counted
  window, so uncounted collection flushes do not accumulate into later delay
  statistics. Raw gaps and the full deferral history remain retained.
- The 320-row online frame-excess maximum (787.794 ms) was traced to six mixed
  one-second reports ending 0.304–0.518 s after the CPU interval began. That edge
  began 0.419 s after RESET, so these reports include pre-reset delay history;
  no >20 ms gap-residual event accompanied them. Preserve original online
  aggregates. Public selection additionally uses whole worker reports whose
  preceding and current timestamps are inside each CPU interval, uniformly for
  every row. CPU windows and online stopping rules remain unchanged. A synthetic
  500 ms pre-window history remains in the original view and disappears from
  the whole-report view. Report all uncounted deferrals separately; this is not
  an uninterrupted-session smoothness qualification.
- The third pair completed its 640-seat window, then the online reducer read a
  client log mid-write and raised `JSONDecodeError` on an unterminated last line.
  The stop status was published before final archiving finished. The controller
  downloaded a still-growing archive, failed to find its digest, and entered
  cleanup. The retained archive is truncated inside `load-run/client.log`;
  remaining raw data were lost. Its 80–480 online summaries remain private and
  are not treated as independently re-reduced accepted results. The 640 window
  and descending confirmation cannot be validated from that failed attempt.
- Fixed the research reader to consume newline-terminated records while
  preserving the raw partial tail, with a regression. Terminal readiness now
  follows archive close, gzip verification, SHA-256 and atomic publication;
  tests inject verification failure and check readiness stays nonterminal.
  The controller verifies size, digest and required source files locally and
  refuses source cleanup until that succeeds, including on earlier failures.
- A host restart interrupted local preparation. At 03:42 UTC the maintaining
  agent confirmed all three pairs terminated, no active Spot requests, valid
  login and absent endpoint. The maintaining agent explicitly authorized one
  more collection within the original $3 cap, including all earlier costs,
  and instructed skipping the human hold because the done signal exists.
  Fresh agent verification checks prior keys/groups and all regions before
  relaunch. Three earlier pairs' bid-cap compute upper bound is $0.2923;
  new 90-minute full timers add at most $0.54 and noncompute reserve is $1.50,
  giving a $2.3323 conservative aggregate maximum. No login refresh.
- At 480 in the final recovery, the online streaming reader had only 81.2 of
  the 90.2 marked seconds when it reduced the buffered logs (116 complete blocks;
  final endpoint inset 8.424 s). Preserve that online range and its stop decision.
  Qualified offline selection uses every complete interior block from the
  original server log after verified retrieval, as the frozen 90-second design
  intended. Reproduce each online result on its retained tick range, then
  independently reconcile the full canonical window; retain both aggregates.
  No workload, host clock, threshold or collected window changes. No claim of
  complete-window CPU is made from a partially drained live stream.
- Final recovery completed all seven ascending windows and the descending
  80-seat confirmation. It stopped ascending at 640 for deferrals, median CPU
  and frame excess. No human hold ran, as instructed. The 145,764,712-byte
  completed archive passed remote gzip verification and local size/SHA-256 and
  required-source checks before source teardown began at 04:17:35 UTC.
- The first cleanup scan again observed recently canceled requests as active.
  A separate final verification at 04:20:11 UTC found zero instances, open/active
  Spot requests, volumes and Elastic IPs in all 17 enabled regions, with 17
  unavailable regions labeled. All eight task instances, four created keys and
  four created groups are gone; all task volumes and the endpoint are absent.
  The protected pre-existing key/group were never targeted. No session refresh.
- Offline selection initially applied the long-lived-thread CPU-sum guard to
  the Python observer, whose short-lived HTTP request threads are absent at
  boundaries. It failed that observer-only sum. Observer overhead now uses
  interpolated process CPU counters, including terminated request threads.
  Simulation/socket and generator thread identity, CPU reconciliation and the
  original max(0.03 ms/tick, 2%) tolerance remain unchanged.
