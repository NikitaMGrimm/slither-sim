# Official-500 AWS acceptance v2: Spot interruption

**Incomplete: no eligible thirty-minute run, no acceptance claim.** AWS reclaimed
the generator during run 1's counted window (`Server.SpotInstanceTermination`).
The single infrastructure replacement was already consumed by the initial
pre-instance capacity allocation failure. No additional pair or run 2 was
launched. This is an infrastructure interruption, not a measured host-performance
failure. V1 remains failed; no result is rescued or pooled.

[Frozen plan](PLAN.md), [attempts](ATTEMPTS.md), [selected receipts](selected-results.json),
[preflight checks](preflight.json), [reproduction](REPRODUCE.md),
[cost](cost.json), [cleanup](cleanup.json).

## Method and prospective corrections

The maintaining agent authorized this successor to
[failed v1](https://github.com/NikitaMGrimm/slither-sim/blob/55f2921594aff00044624e47a8a1f90c4ee0b073/research/performance/2026-10-30-official-500-acceptance/REPORT.md).
Before v2 timing, its final class was corrected to living >=490, mean segments
>=45 and p90 >=156, without upper bounds. The original bounded three-endpoint
entry screen and all other gates, including input-lateness p99 <42 ms in every
selected one-second report, were retained.

Runtime `388be59aebf3108f5c510aa7269c08b67851b202`, Rust 1.99, locked portable
release `-C target-cpu=x86-64`, thin LTO and one codegen unit. One c7a.medium vCPU
for all host/observer threads; a separate same-zone c8a.2xlarge runs six full
received-frame A4 protocol-19 workers, 500 socket slots, zero host bots, default
official density and seed 7. Reuse the
[c7a ladder](../2026-10-29-c7a-mature-capacity/REPRODUCE.md) and
[#256 screen](../2026-10-28-capacity-checkpoint/PLAN.md), with the prospective
exceptions in the plan. No checkpoint or forced counted churn.

The generator moves reconnects, close handshakes and every report write off
decision loops. Histogram maps move to publishers before serialization, including
the endpoint publication that caused v1's six lateness violations. Background
threads inherit each worker's generator CPU, with bounded job queues. The host
binary hash is identical to v1's; no production runtime changes were made.

The plan/harness were committed as `fffdd8a8` after AWS preflight checks and
before tick zero. Pre-reduction correction `010be532`, recorded during warmup,
binds CPU/frame tails to the stated nearest-rank method via corrected v1 helpers
at `fa10b793`. It changes no gate or running world. The historical ladder's
headroom calculation is unchanged. No complete v2 result reached reduction.

## Attempt and evidence status

| Attempt | Outcome | Eligible runs |
| --- | --- | ---: |
| 1 | Spot capacity allocation failure before either instance launched; consumes the single replacement | 0 |
| 2 (run 1) | Generator reclaimed during counting; surviving host stopped and retrieved | 0 |
| Run 2 | Not launched | 0 |

The replacement world began at tick zero and passed entry at ticks
192,000/192,500/193,000: 500 living each; mean 54.622/55.144/55.782,
p90 187/188/189 and p99 246/246/246. It settled ten seconds and began counting.
The last retained control snapshot is tick 285,100: 500 living, mean 99.276,
p90 202, p99 287 and cumulative deferrals zero. That snapshot is **not** the
full-window endpoint and does not establish a thirty-minute gate pass.

AWS's instance-state receipt confirms the generator's termination reason. Its
root volume was deleted as configured; client log/telemetry and the complete run
summary/atomic archive are unavailable. The surviving host was stopped through
the existing source-restricted management SSH path and its archive retrieved;
the local digest matches the remote digest. Raw host and preflight archives and
cloud API receipts stay outside Git, with selected digests in the receipt.

World CPU p50/p99/p99.9/max, maintenance/ordinary costs, client frame tails and
all final gate results are **unavailable for an eligible window**. No favorable
partial interval is reduced or substituted. The async generator's live endpoint
publication behavior therefore remains unqualified by this acceptance.

Post-result harness limitation: the per-worker `cpuFraction` used only the
decision thread's CPU clock. Connector and publisher threads share that
worker's CPU but were omitted from its 70%/80% headroom gate. Process-wide
telemetry does not repair that per-worker omission. V2 reduced no result and
establishes no generator-headroom pass. The frozen executed harness is retained;
v3 must sum decision, connector and publisher CPU for each worker before
applying the unchanged thresholds.

## Verification, cost and cleanup

On the first fresh AWS generator before warmup: portable build, slow-connect/
slow-publication regression and deliberately synchronous mutation (failed as
intended), ladder/gate/token controls, seven documentation tests and repository
boundary checks. The bounded Cargo target was removed. During warmup, nearest-
rank and prospective-gate controls passed after the authorized method correction.
No Rust work ran locally, and the private bench host was not accessed. Final prose/receipt
changes were checked statically; documentation tests were not rerun after AWS
teardown. No independent review was performed.

The conservative launch-to-verified-cleanup compute bound is **$0.255592**;
including the full $1 noncompute reserve gives **$1.255592 < $3**. This is a bound,
not the final invoice. Final audit covers **all 17 enabled regions**, none
unavailable: no task instances, open/active requests or volumes remain, and the
task key/security group are deleted. Unrelated resources were preserved.

Only the research index changes outside this study. Root README and changelog
remain unchanged; there is no capacity sentence or runtime adoption.
