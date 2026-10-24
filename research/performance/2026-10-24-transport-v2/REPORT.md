# Serve transport cost at the controller, v1 and v2, October 24, 2026

## Question

How much per-step time does a `serve` controller spend when it receives
`slither-sim-json-v1` replies, compared with `slither-sim-json-v2` columnar
frames ([PR #61](https://github.com/NikitaMGrimm/slither-sim/pull/61))? This
includes waiting for the reply and parsing it, as well as the simulator itself.
A second question: how much does v2's parallel world stepping help a 16-world
session?

These are synthetic, local scenarios on one Windows desktop. They do not measure
official servers, bot playing strength or other platforms.

## Method

`tools/perf_transport.py` (see the [performance guide](../../../docs/performance.md#controller-transport))
generates one request script per scenario. Every script uses seed 7 and six
decision ticks. Its headings are fixed in advance and never read observations,
so every mode, client and protocol simulates the same worlds. Boost is always off.

| Scenario | Worlds × agents | Config | Steps | Headings |
| --- | ---: | --- | ---: | --- |
| `small` | 1 × 4 | default | 1,000 | steady turn |
| `official-128` | 1 × 128 | radius 20,000, fixed radius | 300 | drift outward |
| `dense-64` | 1 × 64 | radius 3,000, fixed radius | 300 | steady turn |
| `heavy-food` | 1 × 128 | radius 6,000, fixed radius, 40,000 food | 50 | steady turn |
| `worlds-16` (v2 only) | 16 × 32 | radius 6,000, fixed radius | 200 | steady turn |

Each scenario runs in three modes:

- `server` pipes the whole script into `serve` and drains stdout in 64 KiB reads.
  It times from the end of the reset reply to EOF.
- `python` and `node` are closed-loop reference controllers. Python uses stdlib
  `json`; Node uses `JSON.parse`. Per step, they time **wait** (write the request,
  block for the reply line), **parse** and **decide**. Decide finds each living
  slot's nearest visible food and folds it into a checksum. Decisions are not sent.

The reset reply, process startup and request encoding are excluded from per-step
times. Each run passed the tool's checks: every mode and repeat received the same
reply bytes, and the Python and Node checksums agreed. The v1 and v2 scripts send
identical headings; the v2 reset adds only `"worlds":1`. Both protocols reported
the same living snakes and the same total visible food per scenario, for example
591,202 food rows over `dense-64`. That confirms that both protocols simulated
the same worlds. Checksums differ between protocols because v2 folds row indices
and rounds coordinates.

Five repeats per session, one fresh process per mode and repeat, no warmup. The
mode order rotates between repeats. Tables give medians in ms per step.

## Pins

| | v1 | v2 |
| --- | --- | --- |
| Runtime source | `0d367f6f98b94c046684bc8bf70d17e860fea5e9` | `64a368c4a4bcff032fb359b9befc03d5e6003e2f` (`feat/serve-v2`) |
| Release binary SHA-256 | `0496255da103d135ec025fd2d782f212ac49cd8fc81aab6e0eb0bffcc41344de` | `6c9833c5954c33d5a1d547a7355938b628c05ca811083f8f1f78389671c7a2aa` |
| `perf_transport.py` blob | `329a4eec73ce408282b22de460cde87a3c55545e` | `219d68048055d4fc1fc7b0649d55e768f9b32d0c` |
| Client blobs (`.py`, `.mjs`) | `26da369c…`, `aa0415d7…` | `d720ca82…`, `7c2900bd…` |

The v1 tool is the v1-protocol version of this tool, with the reader fix
described below. The v2 tool is the committed version. The `toolCommit` field in
the raw reports shows the uncommitted working-tree base, so use the blob hashes
above. Both binaries were built with `cargo build --release --locked --bin slither-sim`
using rustc 1.99.0 (`b940084d7`) and Cargo 1.99.0, with the locked release profile.

The host was an AMD Ryzen 7 9800X3D (16 logical CPUs) running Windows 11
10.0.26200, with Python 3.12.9 from the locked uv `tools` environment and
Node.js v24.15.0. It was a shared desktop, not an isolated host.

## Results

### Original v1 baseline

This session ran from 14:21:59 to 14:24:36 (+02:00) with tool `28b6401`. Total
CPU during it was 11–19%; the benchmark itself uses about 12%. Its server-only
reader requested 1 MiB per `read1` call. Python allocates the full size on every
call, which cost about 0.13 ms per read. Server-only rows here therefore
overstate `serve` time for small replies. For `small`, the same binary took
about 24 ms in total writing to NUL, against about 155 ms through this reader.
Client rows do not use this reader.

| Scenario | Mode | KB/reply | Wait | Parse | Decide | Total | Total min–max |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| small | server | 33.9 | | | | 0.161 | 0.153–0.174 |
| small | python | 33.9 | 0.145 | 0.243 | 0.021 | 0.405 | 0.386–0.429 |
| small | node | 33.9 | 0.135 | 0.094 | 0.001 | 0.229 | 0.224–0.264 |
| official-128 | server | 241.1 | | | | 0.720 | 0.682–0.773 |
| official-128 | python | 241.1 | 0.906 | 2.094 | 0.055 | 3.037 | 3.017–3.066 |
| official-128 | node | 241.1 | 0.935 | 0.674 | 0.005 | 1.614 | 1.583–1.671 |
| dense-64 | server | 610.6 | | | | 1.425 | 1.353–1.430 |
| dense-64 | python | 610.6 | 1.838 | 5.084 | 0.176 | 7.097 | 7.005–7.137 |
| dense-64 | node | 610.6 | 1.921 | 1.645 | 0.010 | 3.586 | 3.515–3.638 |
| heavy-food | server | 21,985.1 | | | | 63.46 | 62.62–64.15 |
| heavy-food | python | 21,985.1 | 79.20 | 173.01 | 17.23 | 269.61 | 262.01–271.59 |
| heavy-food | node | 21,985.1 | 76.79 | 63.89 | 1.02 | 141.66 | 140.37–143.01 |

### Same-session comparison

These runs used 64 KiB reads and ran back to back. The v1 rerun took
14:37:58–14:40:45, v2 14:40:45–14:41:10, and v2 with one thread
14:41:10–14:41:18. Total CPU was 12–33% during the v2 runs, but spiked to 70–96%
several times during the v1 rerun because of other work on the host. That
explains the wide spread of the v1 `heavy-food` Python row. Apart from that row,
the v1 client medians stayed within 5% of the original baseline.

v1 rerun:

| Scenario | Mode | KB/reply | Wait | Parse | Decide | Total | Total min–max |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| small | server | 33.9 | | | | 0.107 | 0.090–0.108 |
| small | python | 33.9 | 0.151 | 0.240 | 0.021 | 0.414 | 0.398–0.421 |
| small | node | 33.9 | 0.144 | 0.095 | 0.001 | 0.240 | 0.235–0.247 |
| official-128 | server | 241.1 | | | | 0.780 | 0.739–0.805 |
| official-128 | python | 241.1 | 0.877 | 2.050 | 0.055 | 2.978 | 2.967–3.059 |
| official-128 | node | 241.1 | 0.951 | 0.683 | 0.005 | 1.638 | 1.629–1.819 |
| dense-64 | server | 610.6 | | | | 1.545 | 1.468–1.809 |
| dense-64 | python | 610.6 | 1.857 | 5.112 | 0.179 | 7.154 | 7.089–7.557 |
| dense-64 | node | 610.6 | 1.914 | 1.686 | 0.011 | 3.623 | 3.562–3.660 |
| heavy-food | server | 21,985.1 | | | | 62.62 | 61.65–63.90 |
| heavy-food | python | 21,985.1 | 85.70 | 195.80 | 20.04 | 302.07 | 263.30–341.86 |
| heavy-food | node | 21,985.1 | 78.47 | 65.17 | 1.10 | 145.03 | 144.02–147.82 |

v2, default thread pool:

| Scenario | Mode | KB/reply | Wait | Parse | Decide | Total | Total min–max |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| small | server | 4.9 | | | | 0.025 | 0.022–0.030 |
| small | python | 4.9 | 0.039 | 0.054 | 0.017 | 0.111 | 0.106–0.123 |
| small | node | 4.9 | 0.053 | 0.014 | 0.002 | 0.068 | 0.059–0.081 |
| official-128 | server | 20.8 | | | | 0.254 | 0.241–0.283 |
| official-128 | python | 20.8 | 0.315 | 0.199 | 0.038 | 0.540 | 0.526–0.605 |
| official-128 | node | 20.8 | 0.316 | 0.061 | 0.004 | 0.381 | 0.361–0.391 |
| dense-64 | server | 22.6 | | | | 0.161 | 0.160–0.173 |
| dense-64 | python | 22.6 | 0.220 | 0.244 | 0.114 | 0.574 | 0.572–0.611 |
| dense-64 | node | 22.6 | 0.210 | 0.056 | 0.009 | 0.274 | 0.273–0.308 |
| heavy-food | server | 1,878.6 | | | | 6.658 | 6.622–6.698 |
| heavy-food | python | 1,878.6 | 7.878 | 14.683 | 12.499 | 35.25 | 34.26–35.55 |
| heavy-food | node | 1,878.6 | 7.608 | 2.643 | 0.378 | 10.67 | 10.45–10.72 |
| worlds-16 | server | 132.8 | | | | 0.320 | 0.258–0.370 |
| worlds-16 | python | 132.8 | 0.436 | 1.387 | 0.369 | 2.190 | 2.094–2.348 |
| worlds-16 | node | 132.8 | 0.464 | 0.362 | 0.031 | 0.857 | 0.804–0.902 |

### v1 → v2 ratios

Each ratio is the v1 rerun median divided by the v2 median, so higher means v2 is
smaller or faster.

| Scenario | Reply size | Server | Python wait | Python parse | Python decide | Python total | Node wait | Node parse | Node total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| small | 6.9× | 4.2× | 3.8× | 4.4× | 1.2× | 3.7× | 2.7× | 6.8× | 3.5× |
| official-128 | 11.6× | 3.1× | 2.8× | 10.3× | 1.5× | 5.5× | 3.0× | 11.1× | 4.3× |
| dense-64 | 27.0× | 9.6× | 8.4× | 21.0× | 1.6× | 12.5× | 9.1× | 30.3× | 13.2× |
| heavy-food | 11.7× | 9.4× | 10.9× | 13.3× | 1.6× | 8.6× | 10.3× | 24.7× | 13.6× |

`dense-64` gains most: 64 snakes in a small arena see largely the same food and
snakes, which v1 repeated per observer and a v2 frame lists once. In
`official-128` the 128 snakes together see about 82 food items per step, so its
v1 replies are mostly repeated opponent bodies. Decide gains least because it
walks the same number of visible food items in both protocols. In `heavy-food`
under v2, Python decide (12.5 ms) is nearly as large as parse (14.7 ms) out of
35 ms.

### Parallel world stepping (v2, `worlds-16`)

Repeat medians in ms per step. The one-thread runs passed `serve --threads 1`.

| Mode | Default pool | `--threads 1` | Slowdown with one thread |
| --- | ---: | ---: | ---: |
| server | 0.320 | 1.289 | 4.0× |
| python total | 2.190 | 3.172 | 1.45× |
| python wait | 0.436 | 1.380 | 3.2× |
| node total | 0.857 | 1.838 | 2.1× |
| node wait | 0.464 | 1.430 | 3.1× |

512 snakes stayed alive throughout, and slots saw 4,441 food rows per step in
total. With the default pool, a Python controller spends about 80% of each step
parsing and deciding, so the 4× server speedup shortens its step by only 1.45×.

## Attempts not used

- A first v2 recording (14:35) still used the 1 MiB reader. Its server-only
  `small` median was 0.148 ms, against 0.025 ms after the fix. It led to the
  reader change. Its raw files were deleted; only its summary table remains in
  the task notes.
- The first v1 rerun (14:37:06) failed: the tool resolved scripts against the
  caller's directory, while clients ran from the repository root. The committed
  tool resolves `--output`. A v2 pair recorded with that tool version at
  14:37:06–14:37:40 was superseded. Both are retained locally.

## Limitations

- Wall-clock timing on a shared desktop: five samples, one process each, no
  warmup. Medians are robust to single spikes, but the host was not idle.
- Server-only includes the pipe to a Python reader. With 64 KiB reads it matched
  writing to NUL within about 1 ms per run for `small` and within 5% for
  `heavy-food`.
- Request encoding is not timed; scripts are pre-encoded. v2 step requests are
  also smaller.
- Only one decision rule and only stdlib parsers. A bot with a different
  decision step, or a faster JSON parser, sees different proportions.
- `worlds-16` has no v1 equivalent, so it is compared only across thread counts.
- Windows only. Cross-platform float equality is not promised; checksums are
  only compared within one protocol and platform.

## Reproduction

Build each runtime in its own checkout, copy the binary, and record its hash:

```sh
cargo build --release --locked --bin slither-sim
uv run --locked --only-group tools python -B tools/perf_transport.py --repeats 5 --output outputs/transport/v2-default
uv run --locked --only-group tools python -B tools/perf_transport.py --repeats 5 --scenarios worlds-16 --threads 1 --output outputs/transport/v2-threads-1
```

The v1 runs used the tool from commit `28b6401`, with `CHUNK_BYTES = 1 << 16` and
`TAIL_CHUNKS = 1024` (blob above), run against the `0d367f6` binary with
`--repeats 5`. That commit was a local pre-rebase commit and was not published;
to recreate it, port the v2 tool's request encoders and client
`decode_reply`/`decide` back to v1 observations.

The raw `report.json` files, request scripts, binary copies, logs and CPU samples
are local-only, under `outputs/transport/{v1-baseline,v1-rerun,v2-default,v2-threads-1}`
in the task worktree. They are not downloadable from this repository.
