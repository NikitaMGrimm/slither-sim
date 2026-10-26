# Retained official arena-radius study, 2026-10-26

Retrospective, exploratory, offline only. No acquisition or game-server contact.
Read only recordings/headless; never recordings/community or custom-server data.
Inventory every headless *.jsonl.gz, hash compressed bytes, fully read gzip/CRC,
retain failures and capture-end status. Only completed protocol-19 initialized
connections enter main results. Unfinished files remain in the exclusion ledger.
Deduplicate identical compressed archives. Concurrent recipients are not
independent server trials. Source archives remain unchanged.

Decode setup radius and every z broadcast, the l packet reported player count,
recipient snake-cache occupancy, marked deaths, leader segment-mass proxy, and
minimap occupancy. Cache occupancy is not global living population; l does not
independently identify connected sockets versus living players. Never count a
plain visibility removal as a death. Reset clocks/state at each connection/setup.

Use run3-large-radius-20260927 only for coefficient selection; every other
completed directory is a directory-level holdout. These archives have prior
analyses: this is not a blind or preregistered new confirmation. Five-second
direction windows require preceding and following samples within 3.5-6.5 s,
fresh l reports <=3.5 s; global proxy comparisons additionally require map age
<=5 s. No interpolation through connection boundaries. Fit k*sqrt(population),
EMA-population (tau 5,15,30,60,120,300 s), and fixed absolute-clock sine
(period 30,60,120,180,300,600 s; 24 phases); compare persistence and k=670.
Use existing frozen global/motion methods as additional cross-checks. Report
denominators, per-directory scores and failed alternatives, not a best-fit law.

Summarize cadence, signed update sizes, net/range/total variation, monotonic runs,
plateaus, and radius motion over minute/30-second bins. Measure server-header and
receipt elapsed times separately. Sparse z traffic is not a high-rate continuous
radius record. Hold-last gives only the last received radius between messages.

Run actual SlitherSim in a small offline Rust driver with 480 slots for 180 s,
the official radius controller and 7 ms dt. Use explicit synthetic recurring
deaths/respawns (fixed 480 slots, scheduled arrivals with mean lifetime 60 s,
2 s dead wait), plus fixed and connected-slot counterfactuals. Remove food/prey
work for the controller-isolation experiment. Label all outputs synthetic; do
not claim the death rate or comparison alternatives are measured official laws.

Resource envelope: one analysis process, no child agents, build jobs/test threads/
Rayon threads all 2, task-owned target and uv environment. Check QUIET-WINDOW.lock
before expensive runs; do not interrupt unrelated processes. No production edit
or PR unless evidence identifies a defensible correction rather than preference.
