# Attempts

- Confirmed the existing AWS CLI session before cloud work. Pushed the parked
  marker reference unchanged at `58145ae`; created no marker PR and used no
  SSH bench guest time. The new comparison excludes marker reuse.
- Verified #265's parent and froze `cc1bb1e9` versus `539b37f3`. Reused #263's
  received-frame boost treatment, phase/clock accounting, client gates and
  archive-readiness/source-preservation lessons. No collection is claimed yet.
- One local controller-generation command used shell quoting that PowerShell
  rejected before execution. Moved the generation into a private Python file;
  source staging and wrapper-composition checks then passed. No cloud resource
  was involved in that tooling failure.
- The first generator Spot launch was rejected for insufficient capacity. No
  instance launched. Its temporary key and security group were deleted, and
  all 17 enabled regions were scanned with zero instances, active/open Spot
  requests, volumes or Elastic IPs; 17 disabled regions were not claimed scanned.
  Kept the raw API error and cleanup receipts. A second placement attempt in a
  different AZ retains the same c7a.medium/c8a.2xlarge pair, order, timers and
  budget. This is not a replacement of a paid measurement pair.
- Both instances launched on the second placement. Verified boot poweroff
  deadlines, terminate-on-shutdown and DeleteOnTermination devices before work.
  Native portable builds for both arms and the heading/rule boost regression
  passed. Host binaries differ, shared policy hashes and client example source
  match, and one candidate-built client executable serves both arms.
- Clarified that fresh ladders match scheduled wall-time history, not identical
  virtual ages or world states under overload. No checkpoint restore, world
  editing or paused process is used. Census divergence stays visible.
- An early documentation check ran while collection was still in progress.
  Six tests passed; the link check failed because the reproduction recipe
  already linked the not-yet-generated selected results. Retained this result;
  rerun the full documentation check after assembling the evidence.
- The second placement completed base-160 and candidate-160; both full archives
  were locally verified and their canonical reductions passed. Spot then
  terminated the generator during candidate-320 warmup. The CLI session was
  still valid. Bounded SSH retries stopped, and the controller preserved the
  surviving source server under its boot timer. No 320 primary window completed.
- The first manual salvage command had an unterminated nested string and failed
  to parse before taking any remote action. Moved it to a private script. The
  restored command stopped the surviving host, closed/gzip-verified/hashed its
  available sources and locally size/hash/source-verified the recovered archive.
  The generator's root disk was already deleted by Spot; incomplete third-row
  client receipts are unavailable, not silently reconstructed.
- [The pre-repeat amendment](PLAN-ADDENDUM.md) keeps both verified 160 rows as
  interrupted-attempt evidence and repeats all six primary rows together on a
  fresh pair. No pooling or selective replacement. The combined full-timer
  ceiling is $2.51, including the interrupted pair and all noncompute reserve.
- The interrupted-pair cleanup removed both instances, volumes, key and group,
  but its first all-region scan still found one own fulfilled Spot request
  active. The zero-resource assertion failed rather than claiming success.
  Retained that scan and cancelled the request using its captured request ID;
  a fresh all-region scan is required before any replacement launch.
- Explicit own-request cancellation completed; the fresh scan verified zero
  instances, open/active Spot requests, volumes and Elastic IPs across all 17
  enabled regions. Both task keys/groups were removed; protected resources
  were untouched. The third placement then launched a fresh same-type pair in
  another AZ, with the same verified timer/deletion safeguards. Rebuilt host
  and client executables and source-manifest digests exactly match the previous
  paid attempt; both native treatment regressions pass. All six primary rows
  restart from the predeclared first row.
- The repeated pair completed its two 160-seat rows and candidate-320, then
  Spot reclaimed its generator during the base-320 ladder, before the 320
  primary. The target server remained running and the CLI session remained
  valid. Recovered and locally verified the surviving fourth-row host archive,
  stopped its process, released its control port and preserved the old
  directories. Its generator receipts were already deleted by Spot.
- [The second recovery amendment](PLAN-RECOVERY.md) retains the same target
  server across all primary rows. The requested c7a generator was rejected for
  capacity without launching an instance; its request/error are retained.
  An eight-vCPU c7i Spot generator then launched in the target's AZ. Verified
  its 120-minute boot timer, terminate behavior and DeleteOnTermination.
  Installed and verified a later target poweroff timer before cancelling the
  original, bounding target lifetime to 160 minutes. Combined ceiling: $2.73.
- Both replacement native builds and treatment regressions passed; every source
  manifest, host binary and shared client binary exactly matches the earlier
  builds. Repeat candidate-320 and base-320 together on the replacement, then
  both 480 rows. The earlier completed candidate-320 remains supplementary,
  without pooling or substitution. Primary 160 uses c8a; primary 320/480 uses
  c7i. Generator/queue/treatment gates remain mandatory, and cross-seat client
  timing carries this hardware limitation.
- The c7i candidate-320 archive completed and was retrieved/verified, but its
  generator did not qualify: worst input-lateness p99 was 59.197346 ms against
  the unchanged 42 ms gate. Maximum worker CPU p99 was 33.92%; socket errors
  were zero and queues passed. The offline strict selector independently
  rejected it. The collector stopped before base-320 and cleaned up; all 17
  enabled regions verified zero instances, active/open requests, volumes and
  Elastic IPs at 11:37:21 UTC, with own key/group removed. Retain the invalid
  row and its client receipts, without using it as live-tail evidence.
- [The final full-repeat amendment](PLAN-FINAL-REPEAT.md) bounds all prior paid
  compute by $0.65 at bid caps (calculated upper bound $0.626), final-pair timer
  compute by $0.84, and all-attempt noncompute costs by a $1.40 reserve: $2.89
  total. A fresh c7a.medium/c7a.2xlarge pair launched in another AZ. Verified
  generator topology is eight physical cores, eight vCPUs and one thread/core;
  both 140-minute boot timers, terminate behavior and DeleteOnTermination pass.
  Native source and binary pins match every earlier build. All six primary
  rows restart together; no earlier row supplies a final primary comparison.
- That eight-physical-core c7a generator completed base-160 but failed input
  lateness: 46.604952 ms against 42 ms, with worker CPU p99 23.34%, zero socket
  errors and passing queues. Both online qualification and the independent
  strict selector rejected it. Retrieved the complete archive before cleanup;
  all 17 enabled regions verified zero resources/active requests at 11:56:01 UTC.
  Core count alone is not evidence of sufficient client scheduling.
- [The Zen 5 pool amendment](PLAN-ZEN5-POOL.md) returns to the original CPU
  family, trying c8a.4xlarge with the same six workers to avoid the twice-reclaimed
  c8a.2xlarge pool. All prior paid compute is bounded by $0.75 at bid caps
  (calculated $0.716). With $0.50/h generator/$0.06/h target caps, 142-minute
  launch-to-timer bounds and a checked $0.90 all-attempt noncompute allowance,
  the combined ceiling is $2.975334, below $3. The noncompute reserve was
  reassessed from retained artifact sizes and conservative control-poll bounds.
- A quota metadata query returned Windows-encoded descriptive text, which the
  local UTF-8 subprocess reader rejected. The shell also dispatched the
  controller after that failed preparation; missing prepared choice data
  stopped it before resource creation. Its all-region scan was zero. Preserve
  both failures. Restricted the quota query to numeric values and separated
  preparation from launch so a failed preparation cannot dispatch provisioning.
- The first larger-pool placement was rejected for capacity without launching
  any instance. Deleted its key/group and verified zero resources/active requests
  in all enabled regions at 12:06:37 UTC. A same-type placement in another AZ
  launched successfully; quota/topology, boot poweroff, terminate behavior and
  DeleteOnTermination were verified. Native source and binary pins match every
  earlier build. Six workers remain on CPUs 0–5; extra CPUs are unused by clients.
- The larger-pool sequence verified both 160 rows and candidate-320, then Spot
  reclaimed its generator during base-320 warmup. Preserved all three primary
  rows, recovered/verified the surviving host sources and replaced only the
  generator with the same class/AZ. [The recovery amendment](PLAN-ZEN5-RECOVERY.md)
  split the existing generator timer budget between the terminated and replacement
  instances, with a verified target timer handoff and total ceiling below $3.
- That replacement completed base-320's measured window, but Spot reclaimed it
  during finalization before a verified row archive was downloaded. Recovered
  and verified its full server-side archive. Client sources were lost with the
  Spot disk; this attempt does not supply a primary comparison. This is the
  fourth generator Spot interruption. The original three primary rows remain.
- The maintaining agent explicitly approved one On-Demand c8a.2xlarge, only for
  the generator, while retaining the same Spot target and combined $3 cap.
  [The approved exception](PLAN-ONDEMAND-GENERATOR.md) records the verified
  $0.49196/hour retail rate, $2.973667 full-timer bound, source-preserving timer
  handoff, and base-320-first order. Both 480 rows must fit the remaining planned
  time/cap or 480 is reported not measured. Exactly one such instance launched;
  its market, timer, terminate behavior, volume deletion and identical native
  pins were verified before counting. No guest or local Cargo work.
- Canonical replay of the retained candidate-320 initially failed on one bit of
  a calibration queue-slope float: Windows `pow(x, 2)` gave a different square
  from the original Linux reducer. `x*x` exactly reproduced the Linux result.
  The selector applies only that equivalent analysis arithmetic substitution;
  the full replay equality assertion and every gate stay unchanged. Replayed
  all five qualified supplementary rows and the first three primary rows exactly,
  with all task clocks reconciled. Raw evidence is unchanged and earlier derived
  outputs are retained privately. The transformed reducer has one shared digest.
- The approved On-Demand generator completed base-320. Its full archive was
  closed/hash-verified before retrieval; local size/hash/source checks, exact
  online replay, canonical task clocks and every qualification gate passed.
  The four measured primary rows now form the complete 160/320 comparison.
- The CLI session expired at 14:09 UTC during the following ladder's initial
  80-seat calibration, before a 480 primary. The collector stopped immediately;
  no credential retry or manual renewal followed. Both 480 rows are **not
  measured**. Final task key/group deletion, timer-termination confirmation and
  the all-enabled-region scan remain **pending**, not successful. Verified
  poweroff deadlines and the $2.973667 all-attempt timer bound remain recorded.
- Published only the verified four-row prefix, with five earlier qualified
  windows kept supplementary and both generator gate failures excluded. The
  assembler derives missing 480 rows from the verified prefix without changing
  private collection state or rewriting the frozen six-row plans.
- A report-edit patch failed its context check before changing files; applied
  the corrected patch. The initial privacy scan also matched twelve decimal
  digits inside a SHA-256 digest. Restricted that account-number pattern to
  whole tokens, retaining the cloud-ID, address and exact private-name checks;
  the final scan passes. Seven documentation tests pass and all table text
  exactly recomputes from selected values. No build or long local job ran.
- The GitHub connector advertised PR creation but returned tool unavailable.
  Used the authenticated GitHub CLI instead, opened the authorized study PR,
  linked it to this thread and read back its base/head, ready state and
  footer-only body. No merge or external comment was made.
