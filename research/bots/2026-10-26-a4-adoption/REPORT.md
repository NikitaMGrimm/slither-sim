# A4-mean adoption — 2026-10-26

The maintainer adopted A4-mean as the default public Forager policy, `PolicyId::A4`,
while retaining `PolicyId::A1`. `PolicyId::default()` and `Forager::default()` select
A4. Host, background bots and client runtime selection are outside this change.

## Decision and source limits

The private ledger entry `ADOPT-A4-MAINTAINER` sets the cost gate to **live mean
decision time < 15 µs**, superseding the earlier mean/p99/p99.9/stress limits.
This is a maintainer decision after seeing the results, not a rerun or a pass
under the original frozen rule. The original no-adoption conclusions and failed
or contaminated cost attempts remain in the private records.

These selected results come from private training tooling: C4
(`runs/c4/REPORT.md`) and K5 (`runs/k5/aws-x86/REPORT.md`). Access to the full reports,
raw worlds, timing records and training tooling is **restricted**; they are not
published here. This note transcribes existing results, with no new gameplay or
cost measurements. All results are from offline simulation, not outside game
servers or current-main runtime benchmarks.

Both studies use frozen kernel `be3a035dfab0c5717bef90daf88c6d3ada0e0ffe`.
Gameplay used Linux ARM64, a release build with thin LTO and one codegen unit,
and private Arena source `14e9fc47`. Cost used Linux x86-64, Rust 1.99.1, release
with thin LTO, one codegen unit and debug level 1, generic x86-64 with no Rust
flags. The [port report](../2026-10-26-forager-port/REPORT.md) and
[resolved manifest](../../../src/bot/data/resolved.json) pin the public kernel
port, exact parameter bits and selected conformance fixtures. Neither parameter
files nor fixture expectations change with adoption.

## Held-out paired gameplay (C4)

The full confirmation contained 6,400 worlds across five policy cohorts, each
600 s, at pings 0/50/100/200 ms with jitter 5/5/10/20 ms. The centre-biased and
Battledome tracks used 128 seeds each (20320–20447); the unmodified official
track used 64 (20448–20511). Candidate and reference each occupied a quarter of
the seats, with the frozen opponent pool unchanged. Baseline worlds shared the
same seed, ping and seat layout. Deferred spawns remained in the results.

Score delta is the paired difference of log(subject/reference) score AUC rates
against A1. Kill delta is the paired difference of own kills per scheduled
seat-minute. The copied 95% Student-t intervals are over seeds, averaging the
four pings within each seed before pooling.

| Track | Score log-ratio delta vs A1 [95% CI] | Own kills/seat-minute delta vs A1 [95% CI] |
| --- | --- | --- |
| Unmodified official worlds (U) | +0.892 [+0.822,+0.963] | -0.090 [-0.112,-0.068] |
| Centre-biased training worlds (O; food centre bias 0.25) | +1.052 [+1.003,+1.101] | -0.130 [-0.148,-0.112] |
| Battledome (B; 12 seats, shared pit guard) | +0.369 [+0.333,+0.405] | +0.073 [+0.022,+0.123] |

The score gain comes with fewer own kills on both official tracks. Battledome's
pooled kill gain is positive, but its individual-ping interval excludes zero
only at 50 ms. These are held-out simulator results under the study's mechanics
and opponent pool, not evidence of official-server playing strength.

## Decision cost (K5)

One clean session interleaved A1 and the candidates on the same Linux x86-64
platform. Wall times below are microseconds: the median of each statistic across
three live passes and five fresh-bot stress passes. Live inputs used official
seeds 30000–30001 at all four pings for 120 s and seed 30100 at all four pings for
600 s; stress used an unchanged 50-frame official-derived corpus. No Battledome
timing was measured. Live complete-action/head/score checksums matched across
repeats for each policy; this does not establish cross-platform identity.

| Policy | Live mean | Live p99 | Live p99.9 | Live max | Calls/live pass | Stress p99 | Contaminated |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| A1 | 7.246 | 22.240 | 28.709 | 219.998 | 319530 | 77.419 | No |
| A4-mean | 12.855 | 49.489 | 67.769 | 189.388 | 319853 | 177.889 | No |
| Original upper limits | 10 | 20 | 50 | Ungated | — | 60 | None allowed |

A1 itself missed the old live p99 limit (22.240 > 20 µs), as well as stress p99.
Under the frozen control rule this made K5 inconclusive for adoption; A4 also
missed all four old absolute limits. A4 costs about 1.77× A1 on the mean and
roughly 2.2× on the tails (live p99 2.23×, p99.9 2.36×, stress p99 2.30×).
Its 12.855 µs live mean satisfies the maintainer's revised < 15 µs gate; the
larger tails remain a deployment consideration. These timings measure the
private frozen-kernel harness on one platform, not this public port's runtime.

Public decision conformance can be checked with
`cargo test --locked --lib client::forager::conformance`: A1 and A4 both use the
public constructor and the unchanged platform oracle fixtures. Exact oracle
coverage exists for Windows x86-64 MSVC and Linux x86-64 GNU; other targets,
including macOS, have determinism checks only. Full gameplay/cost reproduction
requires the restricted private tooling and inputs.
