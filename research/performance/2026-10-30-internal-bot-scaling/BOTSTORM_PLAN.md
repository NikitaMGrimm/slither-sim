# Added comparison: botstorm-12k

[AMENDMENT.md](AMENDMENT.md) supersedes the optional-row trigger below before
botstorm collection: run 6k/9k only if 12k misses the 142.86 ticks/s target.
It also requires timing the final guarded snapshot if the ecology screen fails,
with its unsettled label preserved.

The maintainer added this workload while the unchanged A4 series was running.
A community server's reported 12k-snake world was 64,800 units across, used
external vision-free center-seeking bots and an older, unspecified tick rate
on unknown hardware. Those are reported values, not measurements or an
independently reproduced performance result. Do not identify the source.

Primary row `botstorm-12k`: 12,000 occupied slots, fixed radius 32,400,
normal non-Battledome mechanics and official food/prey density at that radius.
Set both adaptive/population sizing flags false; recompute ecology with the
unchanged official density formulas. Core-only execution has no stock encoder
cap. Record resolved Config and any construction refusal rather than reducing
the radius or population silently.

Use the same public admission lifecycle as the A4 rows: 64 poses, preferred
600 clearance, positive-margin fallback, one-virtual-second refusal backoff.
Retain its already-disclosed full-margin-versus-private-host limitation.
Every living snake updates its heading to `atan2(-head.y,-head.x)` only when
`(tick+slot)%6==0`, from its own head. No observation construction, A4, intents,
boost, wire, sockets or worker pool. Physics, lifecycle and steering run on
CPU 0. Separate steering CPU because the reported comparison used external bots.

Warm for at least 300 virtual seconds. Every 1,000 ticks record population,
length quantiles, food, prey, deaths, admissions/refusals, RSS and virtual rates.
Over four complete 30-virtual-second bins require mean population within 1%,
mean length within 10%, positive deaths/s within 15%, admissions/s within 15%
of their respective four-bin means, and at least 99% of requested slots living.
Report food trajectory separately; do not claim complete ecological equilibrium
from these conditions. Preserve the first qualifying checkpoint, or label a
frozen-limit warmup censored. The default per-attempt guard remains 1,800 virtual
seconds or two wall hours; never relabel censored data as stabilized.

From that checkpoint, discard one 512-tick window, then count five more.
Run a separate 512-tick `step_profiled` diagnostic and require every final-state
digest and lifecycle count to agree. Report both physics-only and physics plus
admission CPU/wall p50/p99/max, fractions above 7 ms, steering cost, unpaced
throughput, phase bounds, memory, mean/p50/p90/p99/max length, deaths/s and
admissions/s (virtual second; wall rates labelled separately). Preserve failed
attempts. No comparison claim against unknown community hardware/tick rate.

If the complete 12k warmup/counting takes less than 30 wall minutes, add
`botstorm-6k` and `botstorm-9k`,
in that order, at the same snake density: fixed radii
`32400*sqrt(N/12000)` (about 22,910.26 and 28,059.22). Their normal ecology
scales by area. Use the identical stability/window definitions. These optional
rows are not substitutes if 12k fails. Build/run only after the primary A4
counting series ends, with its original binary and pins preserved unchanged.
