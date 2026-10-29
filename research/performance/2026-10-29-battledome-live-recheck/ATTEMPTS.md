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
