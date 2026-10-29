# Post-compact-history attribution and portable layout

Freeze before collecting new data. Start from merged main after #274; record
its immutable runtime/archive pins before attribution. Do not profile the
pre-merge tree or modify #274's wording. One portable runtime change per PR,
chosen only after fresh attribution. Candidate 2 from #269, contiguous retained
sector keys/timestamps, is the initial lead, not a predeclared winner. Prefer a
different layout only if the fresh samples identify a clearly larger relevant
budget; document that selection before primary comparisons.

All builds, tests, traces, profiling, numerical reduction and timing run on the
authorized SSH guest. Local actions are limited to Git/gh, editing, transfers,
short integrity checks and guest control. Existing offline dependencies only;
no installs, system/security/limit changes, outside game servers or cloud
resource operations. Use one task-owned bounded target and remove it after
verifying copied binaries. Preserve failures and raw evidence in verified
local-only archives before pruning task-owned storage. Record full process
commands at measurement boundaries, redacting the account before writing.
Public host description: a 4-vCPU Zen 5 EPYC KVM guest.

## Fresh attribution

Reuse #269's collector, fixture and symbolization helpers by reference. Adapt
only the measurement overlay to current compact snake histories, record the
changes and keep the diagnostic binary separate from primary measurements.
Portable release/thin LTO/one codegen unit with debug symbols for attribution;
no native CPU targeting, affinity, SIMD or CPU/cache-specific runtime paths.

Warm a complete 500-recipient backend once per protocol to tick 193,000 at
full speed, without sockets or wall pacing, then fork it for each window.
Do not substitute a physics-only checkpoint. Collect three counter-only and
three IP-sampling windows of 1,080 advances for cycles, dTLB and L1d where
available, separately for stock and protocol 13. Instructions accompany each
event. Use the existing corrected L1d sampling period 10,007; cycles use
1,000,003 and dTLB 1,009. Event encodings remain those in #269. Record open
failures, scheduling fractions, lost/throttle records and unresolved symbols.
Do not interpret unavailable events as zero misses or IP samples as precise
load/data-address attribution. Counter-only CPU is diagnostic, not savings.
Do not use profiler CPU as primary CPU evidence.

Gate events around `advance_tick_with` only. Keep policy/admission preparation,
draining and serialization outside. Separate ordinary and maintenance ticks by
actual food-epoch transitions and validate the known cadence. Check fork
endpoint summaries and record mature start/end census. Keep the guest otherwise
idle during attribution; finish all other active work first. If profiles are
throttled/lost, retain them as diagnostic-only and use the counter/source
evidence without silently retrying a more favorable profile.

Rank relevant source/function budgets with sample counts and shares for each
tick class and protocol. Report container sizes/counts and access patterns
where they explain a portable candidate. No assumed speedup from footprint
alone. Commit the chosen design and primary source/build pins before timing.

## Exactness and primary rows

Preserve every wire byte, order, boundary, release opportunity and recipient
value. For retained sectors preserve ascending membership, revision identity
changes, oldest-sector witness semantics, outgoing-stencil `touched` rewrites,
strict `now - seen > RETENTION_SECONDS` arithmetic, epochs, reconnect/reset,
clone independence, re-entry and large jumps. Reuse the existing 48,000-step
random-walk/full-sweep oracle and directed rounding cases, comparing all stored
times as bits and revision-change behavior. Add only necessary layout-specific
edges. Mutation controls must fail at runtime, restore source bytes and pass
the restored oracle; retain their commands/exits. Run the full offline Rust
group and documentation checks, all 27 catalog/host traces and stock/13
wire-payload mutation controls.

Reuse #274's full-backend fork workflow and primary row definitions. Fresh
base/candidate warm streams must match every byte for both protocols. Compare
two child continuations per arm and both direct parents with complete records,
packet/frame bytes, cumulative counts and terminal summaries. Young-500 starts
at zero; 1/10/50 seats warm for 600 ticks and omit only the 500-seat disconnect
recipe. All counted windows contain 1,080 advances, with ordinary/maintenance
splits and endpoint equality checked.

Use one discarded AB followed by five AB/BA/AB/BA/AB pairs per row/protocol.
Report instructions, cycles, CPU, L1d/dTLB misses per thousand instructions,
means/p99 and paired ranges separately for ordinary and maintenance ticks.
Use the fully scheduled grouped events from #274 (at least 99% per interval),
with the same disclosed fallback to separate groups if unsupported before any
primary observations. Include identical wrappers without subtraction. Notify
the maintaining agent before timing, allow at least 60 seconds, and verify the
guest is otherwise idle. No builds, proofs, reductions or extra guest status
commands during counted windows. No outcome-driven exclusions, retries or
post-result layout/threshold tuning.

## Adoption gate

Apply the maintaining agent's pre-result rule to the five reported pairs:

1. Both mature-500 whole-window CPU paired-ratio ranges must be strictly below
   one: every pair improves in each protocol.
2. For each young-500 and 1/10/50-seat control in each protocol, the median
   paired candidate-minus-base whole-window CPU cost is at most +10 µs/tick.
   This is an absolute paired difference, replacing the old relative 2% gate.
3. Mature maintenance CPU must not regress in either protocol: each protocol's
   median paired candidate-minus-base maintenance CPU cost must be at most zero.

These definitions are fixed before results. If any condition fails, reject the
runtime attempt and retain it as a patch, without changing the gate. Keep the
study's verdict separate from any later maintainer decision. Results describe
these instrumented no-socket fixtures, not live capacity or other CPUs.

## Delivery

Keep lean selected data and table calculators, link earlier scripts instead of
copying them, and retain instrumentation/unlanded attempts as patches. No patch
for code landing in the same PR. Update the research index and one Unreleased
Performance changelog entry. Use the PR template's Change/Validation sections
with one accurate Agent-assistance footer last and no top AI note. Do not
merge. Verify archive transfers, stop task processes, remove the target and
announce guest release before the final report if delivery follows later.
