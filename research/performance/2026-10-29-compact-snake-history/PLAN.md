# Compact recipient snake histories

Baseline: current main after #269,
`5f88748f6ee4d8192111292d0578b327a8812fea`. Production behavior must remain
exact. The change concerns only recipient-owned `KnownSnakes` storage; no
physics, numeric precision, release opportunity, protocol, affinity or CPU
feature change is permitted.

Compare two compact designs in a disclosed pilot: separate ascending slot keys
and contiguous records with binary lookup; and a single sorted `(slot, record)`
vector with linear lookup. All values and ownership remain unchanged. Preserve
ascending candidate merge order, replacement, returned removal values, clone
independence, reconnect reset and slot/life reuse. Use the old optional-slot
implementation as a test oracle, including arbitrary field bits, signed zeros,
NaN payloads, high slots, repeated operations and independent recipients.

All builds, tests, traces, reductions and measurements run on the SSH guest;
nothing CPU-intensive runs locally. Builds/proofs may share the guest. Notify
the maintaining agent before each timed stage, then keep the guest otherwise
idle with no status commands inside counted windows. Record full process
commands at measurement boundaries, redacting the account before writing.
Use one bounded target and task-owned temporary storage: persistent free space
is small. Never remove another study's data. No installs, guest/system changes,
outside game servers or cloud-resource operations.

Use portable release code generation, thin LTO and one codegen unit, without
`target-cpu=native` or affinity. Reuse the existing full-backend fixture and
warm once at full speed per arm/protocol to tick 193,000. A physics checkpoint
cannot carry its presentation/recipient state. Fork the frozen backend for
every window. Compare complete warm streams live with bounded buffers rather
than retaining disk-sized copies. Keep byte counts and digests, and preserve
any mismatch. Check two child continuations per arm and direct-parent
continuations against each other, with terminal records and wire payloads.

Proofs for the selected design: all 27 existing catalog/host traces, both mature
wire histories and full warm streams; the focused old/new random-operation
oracle; runtime-failing controls for wrong lookup/removal, lost replacement and
reversed enumeration; restored source and passing focused checks; the full
offline Rust group. A deliberate wire perturbation must change host-frame
payloads for both stock/protocol 13, and a short deliberate stream-byte flip
must fail the complete-stream comparator. Retain commands, failures and restore
checks explicitly; do not rely on an unexplained prerequisite receipt.

Before any timing, run focused oracles and short wire smoke proofs for both
designs. The pilot uses the mature stock fixture, one discarded warmup and
three alternating base/design pairs per design, 1,080 advances each. Select the
lowest median whole-window paired CPU ratio, breaking ties with fewer
instructions. Report every pilot design, including losses. Commit the selected
design and its immutable build/source pins before collecting primary results;
do not reuse pilot observations as primary evidence.

Primary rows use five AB/BA/AB/BA/AB pairs after one discarded AB, separately
for stock and protocol 13. Count 1,080 advances per window, identifying ordinary
and maintenance ticks by actual food-epoch transitions. Report instructions,
cycles, process/thread CPU, L1d and dTLB events per thousand user instructions,
with paired ratios and ranges. No profiler samples run during timing. Try a
four-event group (instructions, cycles, L1d, dTLB); if unsupported or scheduled
below 99%, preserve the failure and use separate instructions+cycles and
instructions+L1d+dTLB groups for all arms. Gate/read the same events around each
advance, keeping policy, admission preparation, draining and serialization out.
Counter/clock overhead is identical and not subtracted.

Short controls include the young 500-seat fixture and 1/10/50-seat worlds for
both protocols, also five alternating pairs. The smaller worlds use the same
fixture policy and natural respawns, omitting the 500-seat-only round-robin
disconnect recipe; warm them for 600 ticks and count 1,080. Exact trace checks
cover these added rows too. They are synthetic regression controls, not capacity
or population-scaling claims.

Adopt only if every primary mature whole-window paired CPU ratio is below one
in both protocols, both maintenance CPU medians are lower, and no short control
has all five paired CPU ratios above 1.02. Report small/mixed shifts honestly.
If this bar fails, reject the runtime change regardless of instruction or
footprint savings; keep it as a patch in a research-only PR with all results.
No extra timing retries for an unfavorable outcome. Infrastructure/quiet failures
remain recorded and require an explicit pre-data correction before a repeat.

Keep selected/cited values and what recomputes the tables. Reuse earlier
scripts by reference; keep instrumentation/rejected designs as patches and no
patch for code that lands. Preserve raw data locally, verify archived transfers
before pruning, remove the sole target and task processes, and announce guest
release. One PR, footer-only body naming actual models/roles; do not merge.
