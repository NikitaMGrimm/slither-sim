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
