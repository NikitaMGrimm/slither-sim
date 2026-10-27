# Shared minimap packets, 2026-10-27

Keep. Base `ac85528` -> runtime `e49eed4`. [Pins, pairs, traces](results.json).

Rasterization was shared; M encoding, checked V candidates and bitmap copies
ran per recipient. Cache packets by immutable snapshot identity instead.
Wire-family caches stay separate from the raster; protocol 13 is unchanged.

## Burst diagnostic

Shared WSL2 Linux x86-64; Rust 1.99 locked release with debug=1,
thin LTO/one codegen unit; jobs/tests/Rayon one. Host CPU 0,
boosting socket shards 2/4/6, no bots. After 15s warmup, hold 45s per arm.
Admission through handoff includes clock overhead, excludes socket-thread/
kernel instructions. CPU/wall timing is contaminated. [Protocol](PLAN.md).

Millions of simulation-thread instructions/tick, base -> candidate:

| Workload | Map-due | Other | Minimap work | Map share of due tick |
| --- | ---: | ---: | ---: | ---: |
| official500 | 291.26 -> 18.61 | 16.03 -> 16.49 | 274.46 -> 1.46 | 94.2% -> 7.9% |
| dome64 | 112.06 -> 11.96 | 2.86 -> 2.86 | 109.41 -> 9.08 | 97.6% -> 75.9% |
| dome80 | 140.82 -> 13.28 | 4.31 -> 4.33 | 136.73 -> 9.23 | 97.1% -> 69.5% |

Observed worst CPU/wall ms (base -> candidate); these are **not speed claims**:

| Workload | Map-due CPU | Map-due wall | Any-tick CPU | Any-tick wall |
| --- | ---: | ---: | ---: | ---: |
| official500 | 64.5 -> 110.9 | 91.6 -> 176.5 | 64.5 -> 110.9 | 261.1 -> 176.5 |
| dome64 | 15.9 -> 1.56 | 20.4 -> 1.57 | 15.9 -> 16.0 | 20.4 -> 16.8 |
| dome80 | 32.9 -> 6.20 | 37.9 -> 6.29 | 32.9 -> 92.0 | 37.9 -> 92.8 |

Only 13/16 official map ticks, 18 per dome arm. No worst-latency/capacity claim.
At most one waiter. Dome retains shared rasterization.

## Pairs and parity

One untallied pair then five alternating instructions:u pairs, >=99% scheduled.
Whole process includes construction/warmup/native clients, excludes sockets/
kernel. All population/lifecycle summaries match.

| Fixture / decisions | Median reduction | Range |
| --- | ---: | ---: |
| official500 /100 | 0.309% | 0.304–0.317% |
| official500 /400 | 1.971% | 1.968–1.982% |
| dome40 /100 | 3.615% | 3.607–3.622% |
| dome64 /100 | 8.489% | 8.475–8.513% |
| dome80 /100 | 9.082% | 9.040–9.100% |

The predeclared supplement reaches six map opportunities; official100 reaches
only its first full map. Dense fixtures include 600 warmup decisions.
These are descriptive fixture results, not live CPU savings.

Rust group, harness 16+16, map regressions and isolated negative control pass.
Twelve complete traces match, including reconnect/decoder replay. Extended
windows contain official M998, dome64 M1/V63, dome80 M2/V78. The first census's
incorrect V-everywhere assumption failed; retained. No parity gate was relaxed.

## Rerun

Use [the prior runner](../2026-10-27-host-sector-union/reproduce.py) with
`--base ac85528 --candidate e49eed4`. Repeat its instructions helper with
official-only/400 decisions. Extended traces: official 120, dome64/80 60.

For diagnostics, extract each pin into a fresh Linux source directory. Run
`python burst.py SOURCE`; build its host_probe/host_capacity_load release
examples with the locked toolchain/settings above. Copy/hash the binaries.
Run generated `study_loopback.py HOST LOAD NEW_OUTPUT --count N --hold 45`
with `STUDY_TICKS=ABS_OUTPUT/ticks.json`; add `--preset battledome` for 64/80.
`python burst.py --reduce OUTPUT` summarizes the hold.

Raw data/all attempts are local-only.
