# V4 attempts

The plan and three host-side scripts were initially committed in `f1f3ccf5`
before any provisioning. Fresh official on-demand rates passed the frozen caps;
all 17 enabled regions were inventoried, preserving unrelated resources. The
first tagged on-demand pair launched with +90-minute shutdown deadlines.

Before any world, the first two readiness checks reached bootstrap before the
shutdown-file check succeeded; no timing started. Both deadlines were then
verified against launch (<=92 minutes), alongside same-zone placement, one/eight
vCPUs and terminate-on-shutdown. No timer was extended or instance replaced.

AWS preflight verifies the unchanged v3 host/client binary hashes and the three
committed v4 source files. Syntax, targeted/protected service classification,
fail-closed quiet-state checks, live all-process/stat samples, summed headroom,
class/gate and nearest-rank controls passed. The documentation check found one
link to v3's report, not yet on main. Before tick zero it was changed to the
pinned v3 commit URL and rechecked; no method, script, workload or gate changed.

Final plan/source freeze is `89d74121a9eecdd7b73d10b16efeb710de7a892f`.
The [preflight receipt](preflight.json) binds it to the unchanged v3 binaries
and local-only proof archive. All seven AWS documentation checks pass after
the link correction. No new Rust build/test or Cargo target is used.

## Pre-start quiet-policy verification correction

No server world existed when initial quiet verification stopped. Actual timer
failure states, cached load states and snap SSM's higher-priority unit file
exposed verifier/setup errors. Preserve the first preparation source/log/states
and the subsequent reset-of-unloaded-service failure in separate local-only
archives. Correct before any world: stop timers before masking, reset only
failed units, verify UnitFileState masks, and record snap SSM as stop-only while
snapd and its socket remain stopped/masked. The final actual-state preparation
passes on the same instances; no deadline extension, replacement, acceptance
window, result or gate change. Final corrected freeze/proofs supersede the
initial pre-start receipt below, preserving the setup failures.
