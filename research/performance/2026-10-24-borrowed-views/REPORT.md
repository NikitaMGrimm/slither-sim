# Borrowed observation views, October 24, 2026

Status: not adopted. Mixed timings on one Windows desktop; no API change.

## Question

Can a Rust bot read its observations faster if they borrow from the world
instead of being copied? `Simulator::observe_all_into` copies every visible
food, prey and opponent of every living slot into reusable `Observation`
buffers. If copying dominated, a borrowed view would make bot loops faster.

## Attempt

[`borrowed-views.patch`](borrowed-views.patch) (`git format-patch`, SHA-256
`d03f1219939f7f4a7ad7f0c017e0cdb95bd2b0841be448c2fe946f573bdb37b1`) holds both
commits on `a8a457e`: `c06bb39` adds the API and `6d51b5f` adds the benchmark rows.

- A reusable `Sightings` buffer holds each slot's visible world indices.
  `Simulator::sight_all` and `sight_selected` fill it and return a `Sighted`
  that borrows the world and the buffer. Each live slot's `ObservationRef` lists
  `&Snake`, `&Food` and `&Prey` in `Observation` order.
- Owned and borrowed batches share `Observer::visit`, mask validation and the
  parallel split. A `compile_fail` doctest shows the world cannot step while a
  view is alive.
- Tests compare both against `observe` and `observe_all_into` on 400 random
  worlds (masks, deaths, NaN/huge coordinates, vision edges, a 4-thread pool)
  and on stepped 96- and 128-snake worlds.

## Method

The `perf_baseline` rows `{dome-proxy-96,official-128}-nearest-{observe,sight}`
step the world untimed, then time filling the batch and reading every living
slot's nearest visible food and opponent head. Both rows use the same distance
function and tie handling, consume results through `black_box`, and produce
identical traces apart from their names.

Release builds ran on Windows 11 with 16 logical cores, on a shared desktop
without a quiet window. Each row ran 7 times at default iterations (900 for the
dome, 800 for `official-128`), with owned and borrowed runs alternating. The
measurements used the same two commits on `ba70de3`; `a8a457e` changed no file
under `src/` or `examples/`.

## Results

Median across runs of the mean time per operation, with the min–max range across
runs where it was recorded:

| World | Batch | `observe_all_into` | `sight_all` | Change |
| --- | --- | --- | --- | ---: |
| `dome-proxy-96` (dense) | sequential | 1.449 ms [1.412–1.475] | 1.250 ms [1.238–1.284] | −14% |
| `dome-proxy-96` | `--parallel` | 0.515 ms | 0.520 ms | +1% |
| `official-128` | sequential | 1.215 ms [1.205–1.223] | 1.257 ms [1.242–1.289] | +3.5% |
| `official-128` | `--parallel` | 0.424 ms | 0.495 ms | +17% |

A temporary fill-only run, not committed, split the cost. Borrowed filling saved
17% in the dense dome but about 4% in `official-128` (1.13 → 1.09 ms), where
visibility filtering of about 20,000 pellets dominates. Reading through world
indices took about 0.15 ms against about 0.07 ms for the contiguous copies.

## Review

An independent review found no correctness or lifetime defect and judged the
comparison fair for this workload, with these qualifications:

- Buffers are created after warm-up, so first-fill allocation and Rayon start-up
  fall inside the measured samples.
- The sequential borrowed path keeps its visibility marks between calls, while
  owned batches allocate fresh ones; the difference is not purely copying.
- Only filling runs in parallel; both read loops are sequential.
- The phase split comes from a separate run, and the parallel rows have no
  recorded spread, so the small differences are not firmly established.

Release assembly showed inlined accessors. Each borrowed item costs a dependent
index load and a bounds check, while owned scans are unrolled. Owned and
borrowed opponents share the same `Snake` stride, and reading `.head` through a
reference does not touch the body. Unchecked indexing would add an unsafe
invariant without removing the indirection.

## Decision

The API is not published. Its only clear gain, dense and sequential, disappears
with parallel filling, and the default official world gets slower. Four public
types, a second method family and a workload-dependent choice are not justified.
`observe_all_into` remains the Rust batch API, and `transport::Session::frame`
remains the typed in-process path.

Reconsider borrowing if a representative bot shows a sustained gain with warmed
operations, alternating paired runs and reported spread.

## Limitations

One machine and one bot loop (nearest food and head) were measured; bots that
read little, or that walk opponent bodies, were not. Peak memory was not
measured, and the reviewer did not reproduce the timings. The per-run reports
and traces were not kept and are unavailable; the table holds the recorded
summaries, and the commands below regenerate them.

## Reproduction

```sh
cp research/performance/2026-10-24-borrowed-views/borrowed-views.patch /tmp/
git switch --detach a8a457e
git am /tmp/borrowed-views.patch
cargo build --release --locked --example perf_baseline
target/release/examples/perf_baseline dome-proxy-96-nearest-observe 900
target/release/examples/perf_baseline dome-proxy-96-nearest-sight 900
```

Add `--parallel` to both commands for the parallel rows, and use `official-128`
with 800 iterations for the other world. `perf_baseline --list` prints the catalog.
