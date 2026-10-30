# Mature broadcast fixture feasibility, 2026-10-28

**Mature host measurements are unavailable.** A bounded synthetic 500-seat core
world reaches mean/p90/p99 lengths **55.33/176/247**, but the existing core
checkpoint cannot reconstruct its food-presentation history. Host restoration
rejects the scene before accepted measurement. Consequently neither #249's
mature saving nor mature broadcast/caller budgets are established.

Keep the successful core checkpoint and both failed construction screens;
retain the explicitly **short-body** results below for provisional next-step
ranking. This study changes no production code or checkpoint contract.
[Plan and amendments](PLAN.md), [selected results and pins](results.json).

For later mature-host measurements, see [mature body coverage bounds](../2026-10-28-mature-covered-bounds/REPORT.md).
That study warms the complete backend from tick zero; it does not make this
checkpoint-restoration attempt succeed.

## Construction and retained failures

The comparison target is the 2,281-seat world's screened mean/p90/p99
63.39/174/250 in [#248's report](../2026-10-28-official-density-capacity/REPORT.md).
That report's separate 501-seat reference has a different length distribution.
The present 500-seat scene approximates the former's lengths, not its complete
ecology, density history, observation path or statistical stationarity.

Seed 7, `Config::official(500)`, original host admission, normal mechanics,
ordinary death/respawn, direct-core A4-mean observations/actions every six
7 ms ticks. No geometry or mass edits. The unchanged screen is living >=490,
mean 55–90, p90 150–230, p99 220–350, tested every 1,000 ticks after 10,000.
Quantiles select sorted living segment counts at `ceil((n-1)*p)`.

| Construction attempt | Ticks | Process CPU seconds | Living | Mean | p90 | p99 | Max | Screen |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Uniform-age birth cohort | 60,000 | 110.664 | 500 | 78.984 | 105 | 135 | 143 | Failed tail |
| 100 resident slots; 400 staggered retire/rejoin slots | 180,000 | 321.282 | 500 | 42.420 | 170 | 246 | 265 | Failed mean |
| Exact core continuation of second attempt, no forced churn | +13,000 | 25.993 | 499 | 55.331 | 176 | 247 | 271 | First passing census |

The second attempt retires one ordinary slot every 50 ticks, a 20,000-tick
lifetime for the rotating 400. The final stage explicitly resets the external
admission controller to seed 7. This is exact authoritative-core continuation
under a newly prescribed controller, not a full-host resume. Each bounded
amendment preceded its new construction run; failed screens were not relabeled.
The total construction cost, including failures, is about 458 process CPU
seconds, excluding builds. It is not a long mature capacity hold.

All three exported core checkpoints pass exact canonical roundtrip comparison
of the whole envelope except producer provenance. The selected scene has
27,610 body anchors, 28,897 food and 20 prey. Its SHA-256, frozen before host
comparison, is
`1ce17570637d9f168202210215763898f8f3dec7adfd176b0c15a0854ea010b2`.
The raw checkpoints and full censuses are local-only, not downloadable;
their hashes identify retained inputs, not independent biological validation.

### Why host restoration stops

The [core checkpoint contract](../../../docs/bot-api.md#checkpoints) preserves
physics RNG/timers/ordered bodies, but excludes sessions and host delivery.
It also does not store `FoodPresentation`'s source palette cursors, color
lifetime queues or private RNG. Current core food records contain an origin,
but cannot recover the source appearance/birth history of extant snake food.

The first stock-host attempt installs the checkpoint into a fresh backend and
fails during advance with `Picked-up food is missing presentation provenance`.
The retained correction recreates presentation, registers profiles, and calls
`sync`/index rebuild before advancing. `sync` itself correctly fails with
`Snake food is missing birth provenance`. Its assertion that restoration leaves
the core unchanged lies after that guard and **does not execute**.

These errors come from the intentional guards in
[`FoodPresentation`](../../../src/client/presentation.rs). The existing
`missing_or_unknown_source_is_rejected` regression passes (1 passed, 0 failed)
on the candidate's source copy. No fallback origins, invented source slots,
guard weakening or ad hoc backend serialization was used.

The exact **existing core-to-full-host checkpoint path** is insufficient for
this scene. This is not a claim that mature fixed work is impossible: a future
fixture could retain authentic presentation/birth history while warming a
complete backend, or use an explicitly designed full-host checkpoint. That
additional work was outside this bounded feasibility attempt.

The failed gzip is partial evidence only. No mature trace parity, protocol-13
trace, mutation control, five-pair comparison, phase profile or #249 saving
completed. Experimental unexecuted branches in [mature.py](mature.py) are not
a supported benchmark or successful evidence.

## Mature and short-body budgets side by side

Post-#249 **short-body** current-thread `instructions:u`, million/tick. One
separate instrumented run per row, each 1,080 ticks. Parentheses are shares of
that same instrumented tick, not shares of a plain tick or elapsed time.
All mature cells are unavailable, not zero.

| Scope | Short stock-500 | Short protocol-13 500 | Short stock-2,281 | Mature 500 |
| --- | ---: | ---: | ---: | --- |
| Host tick | 7.852 (100%) | 8.307 (100%) | 40.123 (100%) | Unavailable |
| World physics | 2.008 (25.57%) | 2.027 (24.41%) | 11.159 (27.81%) | Unavailable |
| Food-interest advance | 0.294 (3.74%) | 0.294 (3.54%) | 1.332 (3.32%) | Unavailable |
| Broadcast inclusive | 5.400 (68.77%) | 5.799 (69.81%) | 27.061 (67.45%) | Unavailable |
| Snake coverage | 0.962 (12.26%) | 0.955 (11.50%) | 4.546 (11.33%) | Unavailable |
| Snake encoding/history | 1.264 (16.10%) | 1.248 (15.02%) | 5.908 (14.72%) | Unavailable |
| Recipient food | 0.652 (8.30%) | 0.653 (7.86%) | 3.884 (9.68%) | Unavailable |
| Recipient prey | 0.598 (7.62%) | 0.598 (7.20%) | 3.698 (9.22%) | Unavailable |
| Whole `compact_food` | 0.116 (1.48%) | 0.116 (1.40%) | 2.493 (6.21%) | Unavailable |
| Collision bounds/index build | 0.258 (3.29%) | 0.277 (3.33%) | 1.008 (2.51%) | Unavailable |
| Collision query/exact tests | 0.242 (3.08%) | 0.242 (2.91%) | 1.111 (2.77%) | Unavailable |

Coverage/encoding/recipient food/prey nest inside broadcast; compaction and
collision brackets nest inside physics. Do not add children to their parents.
Counter reads, changed inlining and the diagnostic coverage vector add overhead.
These parent budgets locate work; they are not predicted or measured savings.
In particular `compact_food` includes survivor search, rotation and truncation;
the entire budget cannot be assigned to vector rotation. No accepted new
calibrated libm breakdown was obtained from the mature scene.

The earlier [baseline caller table](../2026-10-28-broadcast-distance/REPORT.md#attribution-before-implementation)
remains the accepted short-body libm attribution: shared libm 9.813% at 500
and 8.785% at 2,281, with static compiler-builtins `fmod` outside that total.
It does not explain #248's 45.1% mature-world shared-libm sample. The rejected
external sampler's apparent food-rotation share is not used here.

### #249 saving, by fixture maturity

These short-body numbers are retained from #249, **not new measurements**.
Five adjacent alternating pairs after discarded warmup; full plain host ticks,
excluding outer construction, controls and drains. Reductions are median paired
ratios; ranges are descriptive, not confidence intervals.

| Workload | Short-body instruction reduction | Mature reduction |
| --- | ---: | --- |
| Stock-500 host | 3.2899% (3.2889–3.2910%) | Unavailable |
| Protocol-13 500 host | 3.0907% (3.0902–3.0911%) | Unavailable |
| Stock-2,281 host | 3.0228% (3.0227–3.0245%) | Not attempted |
| `client-official-500 24`, entire fixture call | 2.5573% (2.5567–2.5575%) | Not attempted |

#249's 23 catalog and four host/wire traces match all decompressed bytes;
stock/protocol-13 perturbations and the geometry-guard mutation are detected.
The new diagnostic summaries match the accepted plain candidate's complete
stderr summaries for all three short rows. This summary check is not a new
full-wire parity gate. No new production optimization is proposed in this PR.

## Next candidates, provisionally ranked for the 500-seat target

1. Conservative body-segment/chunk bounds for `covered`: the measured parent
   costs 0.962M/tick at 500 and 4.546M at 2,281. Preserve original edge order,
   exact tests and boundary fallbacks. Measure the segment-only fraction and
   maintenance cost before implementing. Mature lengths may change this ranking
   substantially; no mature saving is inferred.
2. Remaining recipient food delta/sector bookkeeping: 0.652M/3.884M parent
   budget. Shared listings already exist; isolate per-recipient history,
   membership and delta work before selecting an exact narrower change.
3. Bulk-copy survivor ranges in `compact_food`, now `Food: Copy`:
   0.116M/2.493M whole-function budget. Replace range rotations only if overlap
   and stable insertion order are preserved; retain existing exhaustive masks
   and add the required full-trace/mutation/instruction evidence for adoption.

Encoding/history is a larger 1.264M/5.908M parent, but no corresponding exact
memoizable subset is established. Short-body incremental-anchor shared-libm
cost was only 0.128%/0.113% of the original plain tick; assigning the whole
encoder budget to anchor trig or angle normalization would be unjustified.
Collision work is separately measured above. All candidates concern host world
work paid by remote players, rather than A4's construction-only observation cost.

## Pins, reproduction and limits

This PR starts from main `7d9a5d863c092bafe423a6045ed19b6a8e15268d`, after #249.
Measured runtime archives remain baseline
`d127acd1f0e9db2100c8a1dd5c2fa43e2deff506` and #249 candidate
`9c3b29da359586341af83f0fa45aef9f63b30967`; `git diff` confirms the candidate
and merged main have identical `src/`, Cargo manifests and lockfile.

A 4-vCPU Zen 5 EPYC KVM guest, Linux x86-64, Rust 1.99, locked offline release,
portable CPU, thin LTO, one codegen unit, debuginfo=1. One bounded Cargo target,
one build worker, one Rayon worker and CPU-0 affinity. Short diagnostic runs
were sequential with full before/after process command-line inventories
retained locally and the account redacted in memory. No concurrent task build
or timing worker, guest setting changes, package installs or outside servers.

Keep both study directories as siblings. The shared build/profile primitives
are imported from the previous study. Export the two pinned Git archives as
`base.tar`/`candidate.tar` into fresh absolute task directories `MATURE` and
`REMAINING`; share one bounded target by task-owned symlinks. Then:

```sh
# Expected failed screen; retains MATURE/warm/unqualified-checkpoint.json.
python3 mature.py build "$MATURE" --arm base
python3 mature.py warm "$MATURE"
# Run after inspecting the expected failed mean screen, not via &&.
python3 extend.py "$MATURE"
# Expected birth-provenance rejection; NOT a passing trace comparison.
python3 mature.py trace "$MATURE"

# Separate short-body diagnostic, using original accepted plain summaries.
python3 remaining.py build "$REMAINING"
python3 remaining.py profile "$REMAINING" --reference "$ORIGINAL"
python3 remaining.py reconstruct "$REMAINING"
python3 select_results.py "$ARTIFACT_ROOT" > selected.json
```

The selector reads the retained original artifact layout (`mature/`,
`remaining/`, original pair summaries), checks hashes, expected failures and
summary identity, and emits the selected JSON. The uniform-age pilot's original
source/binary/full census remain local-only with selected receipt hashes; the
current warm recipe is the separately identified churn attempt. Driver import
paths/arguments were cleaned for publication; historical driver hashes remain
in the receipts. Reconstruction verifies every generated Rust source of the
published short diagnostic against its measured receipt without rebuilding.
The two published Rust fixture files also match their measured receipt hashes;
all four Python files pass syntax parsing, seven documentation tests pass, and
the selected-result extraction completes with its assertions enabled.
After verification, the single bounded Cargo target and its two task-owned
symlinks were removed with no task worker running; copied measurement binaries,
source trees and raw evidence were retained.

Full checkpoint/source/binary/trace access is local-only, including failed
attempts. No independent review, real-socket capacity, elapsed-time saving or
long-run ecology qualification is claimed. The remaining mature attribution
gap is explicit; short-body rankings must not be presented as mature rankings.
