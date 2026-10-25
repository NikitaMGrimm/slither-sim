# Hosted spawn admission, 2026-10-25

Status: host policy change backed by retained official join evidence. No new
capture.

## Problem

A default `slither-sim host` (4 socket seats, 4 bots) disconnected every
player with `No clear spawn position`, in v0.4.0 and on main. Its world was
`Config::official(1)`: a 670-unit arena that grows toward `670·√living` at about
23 units/s. Every life needed 600 units of clearance from all living snakes, and
the host refused the join when 64 candidates failed.

## Official evidence

Source: `slither-arena-rs/docs/research/spawn-placement-audit.md` (SHA-256
`76e5bd5857b5e6c504590976bc705817b18c27769d1d7ed82f9d725e1b21b49a`, last changed
2026-10-02). It analyzes verified own joins from retained protocol-19 captures
(`outputs/ecology-20260927/spawn-placement-all.json`, SHA-256
`b2a5b7dbce147889f76f483b7ffef14561a21a596f6d412eabb4375adb8f2f19`;
`spawn-neighbors-all.json`, SHA-256
`ad8a415ab679af4b3e02ac4c8cae0f0b4efdf2906820f8525584700c0cc12536`). The
captures are local-only.

The audit uses every verified own join in the archives, deduplicated by SHA-256,
with no time-window selection; opponent appearances are excluded. Each join takes
the last radius received on its connection. Neighbor bodies are rebuilt from raw
packets. Known-body clearance runs from the new head to visible body centerlines,
corridor clearance covers the whole new two-segment body, and head clearance
measures to visible heads. Snakes outside the streamed view are missing, so true
clearances can only be smaller.

| Measure (197 joins, 22 servers) | Value |
| --- | ---: |
| Joins with a visible other snake | 181 |
| Known-body clearance, min / median | 567.5 / 925.4 units |
| New-body corridor below 600 units | 3 (550.4, 567.5, 599.1) |
| Known-head clearance, minimum | 632.2 units |
| Radial fraction, min / max | 0.086 / 0.845 |

No capture shows a refused or deferred official join. The captures cannot
identify the official rejection test: a 550-unit corridor and a 600-unit
head-only rule both fit. Nor do they show what an official server does when
crowded: official arenas hold hundreds of players and were never captured
nearly full.

## Decision

- **Admission:** keep 600-unit clearance as the first choice. It gives new
  players room, and official joins had a median clearance of 925. When none of
  the 64 candidates qualifies, the one with the largest
  `Simulator::spawn_margin` wins: the least centerline distance to a living
  snake beyond its contact range. That candidate must still clear every
  contact range, since an overlap can kill the newborn or its neighbor on the
  first tick; only then is a join refused. No official refusal was observed,
  and host arenas sized for their seats (below) avoid it. Uncrowded worlds draw
  the same candidates as before.
- **Host arena:** with `dynamicRadius`, a host world starts no smaller than
  `population_radius(maxClients + bots)`. The food target and ambient rate scale
  with the area, so `Config::official(1)`'s density is kept. This follows the
  [official baseline](../2026-10-23-official-defaults/REPORT.md): callers choose
  the initial population, and the host knows its seats. Prey keeps its count.
- **Battledome** keeps its spawn ring between pit and wall and its 400-point
  start. Only the crowded fallback applies there.

## Regressions

All synthetic:

- `src/client/server.rs`: `a_default_host_admits_every_seat` and
  `explicit_and_fixed_host_arenas_keep_their_radius`;
- `src/client/simulation/tests.rs`:
  `a_full_arena_refuses_a_join_inside_contact_range`;
- `tests/client_spawn.rs`:
  `crowded_rejoin_takes_the_most_open_candidate_without_replacing_the_neighbor`.

A default host with three `portable_bot --url` clients played 300 steps each.
Before the fix, every client was disconnected after the first frame.

## Open

- The official rejection rule (550-unit corridor or 600-unit head-only) needs
  many fresh joins in a crowded official arena.
- Accepted official spawns never exceeded 0.845 of the radius. The host's
  sampler can still pick bands nearer the rim.
- Ambient food targets do not follow the dynamic radius after creation.
- A full fixed arena refuses a join instead of holding it until room opens.
