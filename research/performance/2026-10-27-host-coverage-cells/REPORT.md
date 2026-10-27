# Incremental snake coverage cells, 2026-10-27

Keep: retain each living slot's integer cell rectangle and change only cells
entering/leaving it. Recompute bounds and rectangles every broadcast; exact
coverage tests still use current geometry. Sorted insertion preserves the old
world order; dead/truncated slots and empty cells are removed. A cell-size
change rebuilds the index. Interrupted broadcasts discard only scratch state.
No report/visibility rule, protocol, physics or socket-service changes.

Measured runtime: `aee5421` -> `dbc21e6`, after #219. Delivery integrates main
`339041f` (#220/#221); the four coverage files remain identical to `dbc21e6`.
[Selected results](results.json).
WSL2 Linux x86-64, locked Rust 1.99 release/debug=1, thin LTO/one codegen unit;
jobs/Rayon/tests one, at most two pinned CPUs. Offline official-500, seed 7,
no bots. Fifteen complete traces match, including frames/boundaries/times,
checkpoints and RNG endpoints: stock/half13/all13, U5/U250/U500, plus nine
historical cases. Raw fixtures run 1080 ticks, controls every 6, reconnect 720;
minimum 499 alive, terminal 500, three rejoins, zero refusals.

## Fresh profile and choice

Diagnostic user-instruction brackets include overhead. Same 853 steady ticks:
tick>=120, all 500 alive, forced 720 excluded, PMU>=99% scheduled. Every diagnostic
trace equals its unprofiled control. Million instructions/tick:

| Phase | Stock base | Half13 base | All13 base | All13 candidate |
| --- | ---: | ---: | ---: | ---: |
| World step | 2.354 | 2.361 | 2.374 | 2.362 |
| Coverage bounds | 0.045 | 0.045 | 0.045 | 0.045 |
| Grid build/update | 1.309 | 1.304 | 1.310 | 0.090 |
| Grid end-of-use | 0.070 | 0.069 | 0.069 | <0.001 |
| Snake updates | 1.898 | 1.896 | 1.893 | 1.889 |
| Shared food | 0.857 | 0.857 | 0.857 | 0.858 |
| Recipient food | 0.618 | 0.618 | 0.618 | 0.618 |
| Recipient prey | 0.922 | 0.922 | 0.922 | 0.922 |
| Emission, including conversion | 0.467 | 0.699 | 0.918 | 0.913 |
| Of which legacy conversion | 0 | 0.184 | 0.360 | 0.357 |
| Total | 10.567 | 10.823 | 11.073 | 9.708 |

The removable grid build/drop budget was about 1.38M/tick, versus the 1.89M
recipient-snake aggregate whose peer deltas are already shared. Integer
membership maintenance avoids rebuilding unchanged cells without caching
visibility decisions or repeating floating-distance math in a new form.
Remaining recipient snake work stays about 1.89M. Unbracketed/other work stays
about 1.3M; reports, candidate merging, HUD and light prep remain in selected data.
Other phase shifts can reflect compiler/allocation effects; no prey-code gain
is claimed. Exploratory fine hooks used #219's reviewed tree, identical to main.

## Alternating pairs

One untallied warmup then five alternating whole-process instructions:u pairs,
>=99% scheduled, no exclusions. Includes setup, all 1080 ticks and client drains;
excludes sockets/kernel. Arm medians and paired reductions:

| Official500 | Base/candidate billion | Reduction (range) |
| --- | ---: | ---: |
| Stock | 12.080 / 10.616 | 12.120% (12.113-12.132%) |
| Half13 | 12.432 / 10.974 | 11.730% (11.725-11.745%) |
| All13 | 12.784 / 11.304 | 11.572% (11.546-11.605%) |

Supplementary full-U5/250/500 reductions are 11.569/11.539/11.551%; selected
ranges are retained. Phase and whole-process denominators differ.

Full Linux Rust group and four focused coverage tests pass. The fresh-build
oracle covers 512 updates with slot/life/vision changes; float-boundary cases
include ULPs, signed zero, extremes and nonfinite inputs. A backend regression
checks edited geometry after queue interruption. Replacing sorted insertion,
ignoring scale changes or skipping broadcast refresh fails controls; restored
tests pass. The four coverage files match the tested archived candidate. Delivery also
passes the full Rust group and all fifteen traces against merged main; their
hashes equal the measured streams.

Descriptive loopback holds use the same warmup/five-pair plan, 10s warmup/12s
hold, host on one CPU and load/sampler on another. All 36 holds pass error and
PMU gates. Whole-host user instructions/tick fall 11.53% stock (range 7.66-13.88%),
9.07% half13 (8.30-12.77%), 8.78% all13 (6.31-11.36%). Measured playing means
span 499.026-499.447; generator busy stays below 67% of its assigned core.
Evolving worlds make these descriptive, not capacity evidence.

## Reproduce and limits

Export base.tar/candidate.tar at selected full pins. `reproduce.py MODE ARCHIVES
OUT`: build, trace, count (--perf PERF), sockets, pairs. `profile.py ARCHIVE OUT
--control BINARY` reproduces coarse brackets including explicit old-grid drop
or new-cache hand-back. Python 3.12+, Rust 1.99, patch and Linux user PMU required.
Separate arm targets/copied binaries; scripts refuse -O. Raw streams/arrays,
exploratory hooks and logs remain local-only outside /tmp.

Arena-vision overlap is CoverageGrid's constructor/update and grid construction
in broadcast.rs: its forthcoming all list must be refreshed by the update path.
covered, near, snake report logic and the prey lookup call site are unchanged.
Shared timing is contaminated; perf_event_paranoid stays 2, kernel instructions
unavailable. No CPU-time, latency or native capacity claim.
