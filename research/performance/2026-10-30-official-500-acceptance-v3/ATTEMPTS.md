# V3 attempts

Before any provisioning/timing, verified official on-demand quotes for the same
c7a.medium/c8a.2xlarge types and sufficient standard on-demand vCPU quota. Frozen
rate caps, 90-minute shutdown timers with a 92-minute launch bound and a $0.40
noncompute reserve bound three possible pairs at $2.976. No Spot request is used.

The source reuses the pinned v2 implementation and corrects the per-worker CPU
input to sum all three thread clocks. Tests/builds will run on the first fresh
AWS generator before warmup, inside the same resource ceiling. No access to the private
bench host or local Rust work. Outcome/replacement records will be appended without erasure.

The first fresh on-demand pair launched with tags, same-zone placement, one/
eight vCPUs and verified shutdown deadlines <=92 minutes from launch. Before
any world: portable locked build passed; all three actual-module runtime tests
passed; synchronous dispatch, omitted helper CPU and decision-only gate mutations
each failed as intended. Strict 70%/80%, inherited ladder/gate/token and nearest-
rank controls passed. Seven documentation tests and repository boundary checks
passed. The target was removed. Host binary equals v2 byte-for-byte. These
results precede the plan/harness freeze and tick-zero startup.

## Run 1: complete, passes

Plan/harness `bbc7d18c` was committed before tick zero. Linked-source equality,
frozen delta bytes and copied host hash passed. Natural warmup passed the three
bounded entry endpoints, then ten seconds settling and a full thirty-minute
window. All gates pass: zero deferrals, final class, client/queue/generator
headroom and CPU reconciliation. The total worker CPU includes all three live
thread clocks. Worst one-second input-lateness p99 is 1.477652 ms. Exact online/offline replay includes nearest-rank tails and maxima; the closed archive was
retrieved and hash-verified. First-pair instances/volumes were verified gone
before launching run 2. No retiming or replacement.

## Run 2: complete, fails zero-deferral gate

The independent fresh on-demand pair used identical first-run host/client
binaries and unchanged parameters, with no new build or target. Source/hash,
same-zone, one/eight-vCPU and shutdown-deadline checks passed before tick zero.
Entry, settling and the full thirty-minute window completed. The closed archive
was retrieved/hash-verified and full online/offline replay matched exactly.

Run 2 records **126 deferred ticks**, all inside the canonical interval. All
other frozen gates pass; worst one-second input-lateness p99 is 1.294105 ms.
Post-result retained-data diagnosis locates all increments in four consecutive
census intervals around +746 to +749 seconds. It neither identifies an external
root cause nor reclassifies/rescues the failed gate. No completed run is removed,
no additional pair is launched and the infrastructure replacement stays unused.
The two-run acceptance fails. Both results and all available raw evidence remain
separate; final teardown/cost receipts accompany the report.

Final audit verified all 17 enabled regions with none unavailable; task
instances/requests/volumes, key and security group are gone. No infrastructure
replacement was used. Conservative capped compute plus full noncompute reserve
is $1.541553, below $3. Root README/changelog remain unchanged.

## Post-result review verification (no new timing)

The inherited selector omits declared report tails from its comparison. The
v3 `check_replay.py` now compares the complete result with no omitted fields
and was run separately on both retained archives; both match exactly, including
all tails/maxima. Each run detects deliberate p50/p99/p99.9/max perturbations.
Receipts: [run 1](run-1-replay.json), [run 2](run-2-replay.json). The original
standalone full-comparison command/receipt was not retained; these are explicit
post-result checks, not a claim that the inherited selector enforced it.

The tested study modules bind to source commit
`bbc7d18c9d5052903bf3b07ba4067a48e8f040d5`: all four retained pre-start source
digests match that commit. It was committed after checks, before tick zero;
linked inputs remain pinned in the plan. The retained preflight pins bind the
instrumented Rust aggregate to
`e63fb3faff687b8742305335211329c85a097a3c6a86961e1e395e37f911378d`,
host binary to
`d7ddbc80efcd0c0b5fb8e1d61d3931eb3aaf3c64b40171fb8687b533a184c780` and
client binary to
`f8424b4bd4e997c06d9d648a356f9d2ea07f3a651953474d4ae68f25ba3387fe`.
Retained host/client files match those digests. The local-only preflight archive
(logs and pins, outside repositories) is 7,580 bytes, SHA-256
`3d5258b23edebd7df000720b5c957db60345f2deab0f9bd1857f408d7a63bf25`.
Preflight test-executable hashes are unavailable: neither the executables nor
their hashes were retained. See [structured binding](preflight.json).

No selected values, frozen plans, gates or outcomes changed.
