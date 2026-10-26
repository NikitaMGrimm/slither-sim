# Arena radius: hold the initial boundary by default

2026-10-26. **Exploratory, retrospective offline study; not confirmatory.**
Selected results from the retained radius audit. No new capture or outside
game-server access was performed for this change. Raw recordings and full
derived series are **local-only, restricted, not published**.

## Decision and limits

The maintainer authorizes `Config::official(n)` to hold its population-sized
initial radius by default. Holding removes the existing controller's
death/respawn jitter. It is a practical scenario choice, **not an identified
official rule**: official radius has genuine sustained waves that holding omits.
The living-count controller remains available through `dynamicRadius: true`,
explicitly provisional and not faithful. No tested population, EMA, clock or
other load proxy supplies a predictive official replacement.

Host startup capacity is independent of subsequent radius motion:
`hostSizeToSeats` lets official worlds enlarge a smaller initial arena for
`maxClients + bots`, scaling food target/refill with area. The host then holds
that boundary by default. Sandbox, Battledome and explicit JSON radii opt out;
an explicit flag overrides the choice. This avoids coupling capacity to the
unfaithful controller or rewriting host JSON before world deserialization.
Resolved configs and checkpoints retain both settings. Rust callers selecting
a custom fixed radius must set `host_size_to_seats: false`.

For recorded fidelity replay, keep `dynamicRadius: false` and use measured
`radiusUpdates`. This reproduces received samples, not unseen continuous motion
or browser smoothing. The original audit recommended holding for a fixed-seat
scene but made no code change: it did not identify an official law. The default
change here is the subsequent maintainer-authorized product decision.

## Sources, exclusions and methods

- All 91 retained official-classified `recordings/headless/*.jsonl.gz` archives
  were inventoried: 72,230,171 compressed bytes. Owner classification supplies
  origin; this is not new independent server attestation.
- Main results use 53 completed, readable protocol-19 archives across 40 capture
  directories and 263 initialized connections: 21,088.518 recipient-seconds
  (5.858 recipient-hours). Overlapping recipients and serial windows are
  dependent; these are not independent server trials.
- The other 38 archives lack `capture-end`: run1-boost (2), run1-forage (1),
  run2-survey (33), run3-smoke-20260927 (1), smoke1 (1). No parse/CRC failure or
  byte-identical compressed duplicate was found. Before/after inventories match.
- Setup and four-byte `z` packets supply radius; `l` supplies reported players.
  A standalone framing/byte parser agrees on 42,210 radius values and 7,011
  reported counts across all 53 completed archives. Five duplicate timestamps
  have equal radius; one same-frame setup→`z` override is ordered (3,788→3,789
  at time zero), excluded from finite-time slopes. No conflicting live `z`
  values share a timestamp. See [verification](verification.json).
- Reported players do not independently distinguish living snakes from socket
  clients. Cache occupancy, minimap cells and leader mass are censored proxies,
  not global population/mass censuses. Plain visibility removals are not deaths.
- Parameter selection uses only `run3-large-radius-20260927` (51 direction
  windows); 39 other directories provide 2,444 holdout windows (1,166 expanding,
  1,278 shrinking). These previously examined archives are retrospective
  holdouts, not fresh blind validation. No directory is split.
- Five-second direction windows require neighbors 3.5–6.5 s away and a count
  report at most 3.5 s old. No reconnect is bridged. Candidate `k` is 450…850
  by 10; EMA tau is 5, 15, 30, 60, 120 or 300 s; absolute-clock sine periods
  are 30, 60, 120, 180, 300 or 600 s, with 24 phases. Training balanced accuracy
  (BA) selects parameters; ordinary accuracy breaks ties. BA averages expansion
  and shrinkage recall. Pooled windows have equal weight; directory means give
  each directory equal weight. No independence-based significance claim is made.

[PLAN.md](PLAN.md) preserves the original exploratory plan, including its
no-default-change conclusion. [Source inventory](source-inventory.json) pins all
archives, including exclusions; endpoint-bearing names of the 33 excluded survey
files are replaced with ordinal labels, retaining byte sizes and hashes.
[Results](results.json) preserve original source
hashes, all directory-level direction scores, motion summaries and selected
connection bins. [Tables](tables.json) retain figure summaries; the
[forecast summary](free-run-summary.json) retains all 114 connection scores,
including holding's 20 losses. No per-row series or endpoint identities are
included. Hashes establish consistency, not scientific validity.

## Figures 1–3 condensed: official motion

257/263 connections change radius; 194/263 contain both directions. The six
unchanged observations are short/sparse (longest 57.823 s with only two rows),
not evidence of permanently fixed arenas. Across 41,941 positive-time adjacent
observations, server-header cadence has median 128 ms, p95 1,506 ms and range
13–67,707 ms; receipt median is 128.72 ms. This is received reporting cadence,
not internal update frequency. Sparse reports cannot establish stationarity.

**Figure 1.** Of 32,890 adjacent intervals 100–300 ms apart, signed unit changes:

| Change | −6 | −5 | −4 | −3 | −2 | −1 | 0 | +1 | +2 | +3 | +4 | +5 | +6 | +7 | +9 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Intervals | 6 | 2 | 3 | 5,410 | 5,905 | 6,698 | 7 | 5,654 | 4,598 | 2,800 | 1,296 | 392 | 101 | 17 | 1 |

The run4 connection 4 witness at 292,116→292,243 ms grows 13,940→13,947
(`7a003674`→`7a00367b`): +55.118 units/s. A prey-transitions connection 3 witness
at 25,857→26,006 ms shrinks 4,844→4,839: −33.557 units/s. Both exceed the
23.4375-unit/s model cap even allowing one unit of endpoint quantization.
Neither is a universal maximum or a replacement cap.

**Figure 2.** `run4-radius-long-20260927/bot0.jsonl.gz`, connection 4, lasts
401.676 s with 2,135 radius rows: 12,926–14,611 (1,685 units, 12.4% of the
13,631 start), while reported players span 447–505. One observed shrink run
lasts 81.332 s and removes 1,685 units. Selected 30-second bins:

| Time (s) | First → last radius | Observed min–max | Reported players |
| --- | --- | --- | --- |
| 0–30 | 13,631 → 13,498 | 13,383–13,631 | 460–494 |
| 180–210 | 13,392 → 13,808 | 13,392–13,808 | 485–496 |
| 270–300 | 13,755 → 14,276 | 13,619–14,276 | 471–498 |
| 300–330 | 14,281 → 14,499 | 14,281–14,611 | 474–505 |
| 330–360 | 14,497 → 13,797 | 13,797–14,497 | 480–494 |
| 360–390 | 13,794 → 13,086 | 13,086–13,794 | 449–483 |

**Figure 3.** A descriptively selected accelerating/braking pulse in that same
connection has two-second endpoint slopes of 7.3 units/s at 280–282 s,
22.4 at 284–286, 39.2 at 288–290, 48.5 at 292–294, 33.2 at 300–302,
18.5 at 308–310 and 11.0 at 312–314. These are observed endpoint slopes,
not a fitted acceleration law. Motion state or a smoothly moving target is an
inference; its source, hidden target and browser presentation remain unknown.

## Figures 4–5 condensed: bounded model failures

| Predictor | Train BA (51) | Holdout BA (2,444) | Mean directory BA (39) |
| --- | --- | --- | --- |
| Always shrink | 50.0% | 50.0% | 50.0% |
| Previous five-second direction | 74.4% | 80.3% | 79.4% |
| 670 × sqrt(reported players) | 50.0% | 54.2% | 53.0% |
| Fitted count coefficient 610 | 64.5% | 51.9% | 50.5% |
| EMA tau 120 s, coefficient 600 | 61.1% | 52.0% | 51.2% |
| Absolute-clock 120 s sine, phase 12/24 | 66.3% | 50.6% | 51.7% |

Selected directory checks from Figure 5, with all directories retained in JSON:

| Directory | Windows | Persistence BA | Count 610 BA | EMA BA | Clock BA |
| --- | --- | --- | --- | --- | --- |
| run4-radius-long-20260927 | 176 | 80.6% | 54.2% | 51.7% | 50.5% |
| fidelity-followup-20260927 | 20 | 83.3% | 50.0% | 50.0% | 62.5% |
| fidelity-next-movement-20260927 | 66 | 80.3% | 53.0% | 59.1% | 56.1% |
| fidelity-motion-clock-20260927 | 77 | 80.5% | 52.5% | 50.0% | 40.2% |
| fidelity-prey-transitions-20260927 | 18 | 77.8% | 61.1% | 61.1% | 72.2% |
| prey-controlled-20260927-first | 33 | 84.9% | 44.3% | 50.0% | 27.0% |

These bounded families fail to generalize as useful controller replacements;
population/time influence is not ruled out. Persistence cannot initialize a
world, predict reversals or generate a full trace. Additional fitted cache,
minimap and leader-mass proxies score only 49.0–53.1% pooled holdout BA.
Short acceleration extrapolation often improves forecasts, but loses to velocity
on the separate followup directory (MAE 2.71 versus 2.08 units). Preserve that
failure: no universal acceleration, spring, damping or target law follows.

Within run4 connection 4, the same latest count 489 and radius 13,668 precede
opposite next changes: 13,667 after 127 ms at 119,299 ms, versus 13,672 after
140 ms at 200,238 ms. Count ages are 336 and 36 ms. A deterministic memoryless
function of only those two reported inputs cannot explain both; unreported
changes, history and hidden state remain possible.

## Figures 8–10 condensed: local jitter and forecast choice

**Figure 8. Synthetic experiment.** The real simulator at source commit
`8890b2ebd3370a2b98348b6620f9e419eb4a55a7`, seed 7, runs 480 slots for 180.005 s
at 7 ms. Schedule seed 20261026 prescribes independent exponential live durations
(mean 60 s), then a two-second dead wait. Food, prey and kiosk work are disabled;
two-segment snakes follow separated small circles. Lifecycle changes are
injected through public state and real respawn calls; an assertion detects any
unexpected physical death. This isolates the real radius controller. It is not
a measured official lifecycle law or a full ecology/host benchmark.

There are 1,406 synthetic deaths and 1,390 respawns; final living count is 464,
10 Hz mean 464.46, range 452–480. Radius spans 14,339.341–14,678.965; the
controller changes on 25,097/25,715 ticks (97.6%), with 363 tick-level reversals
and 4,114.386 units of variation. Its target varies by 41,092.141 units.
One death at 480 shifts the target 15.299 units; the cap allows 0.1640625 per
tick, requiring about 0.653 s for that isolated deficit. Fixed-radius and
fixed-480-slot counterfactuals both stay at 14,678.965; neither models waves.

**Figure 9. Descriptive equal-grid comparison.** Synthetic 0–180 s versus the
selected official run4 connection 4 segment 180–360 s, holding only the last
causally observed value and ignoring zero differences when counting reversals:

| Grid | Synthetic / official reversals | Synthetic / official variation |
| --- | --- | --- |
| 128 ms (1,407 samples each) | 181 / 5 | 3,747.2 / 3,108.0 |
| 1 s (181 samples each) | 71 / 5 | 2,849.3 / 3,108.0 |
| 5 s (37 samples each) | 17 / 5 | 1,151.1 / 3,076.0 |

At the one-second grid the ranges are 339.6 versus 1,220 units; official
observations can be 1.964 s old versus 0.100 s synthetic. This is an illustration,
not a matched experiment. Sparse official observations may miss fast reversals.
Official radius moves farther with fewer observed reversals in this segment.

**Figure 10. Causal forecasts.** Initialize at the first observed radius with a
fresh count, run the exact scalar capped rule in 7 ms steps with only the last
available reported count, and compare holding the same initial radius. Evaluate
received radius rows for up to 180 s; require at least 60 s per connection.
Later stale counts are carried causally. MAE weights rows within a connection,
then connections equally; true living/socket counts remain unobserved.

| Directory / selection | Connections | Count-controller MAE | Hold MAE |
| --- | --- | --- | --- |
| run4-radius-long-20260927 | 4 | 1,230.1 | 131.4 |
| fidelity-followup-20260927 | 2 | 547.7 | 139.7 |
| fidelity-next-movement-20260927 | 3 | 426.3 | 230.7 |
| All eligible connections | 114 | 519.6 | 211.3 |
| Excluding calibration directory | 113 | 515.1 | 210.9 |

Holding wins in 94/114 connections, or 93/113 excluding calibration. It loses
in 20 and suppresses real waves. These descriptive errors under this
initialization/receipt sampling do not validate a constant-radius official law.
No fitted parameters enter these two forecasts.

## Reproduction and verification

The source commit and source hashes are in [results.json](results.json);
compressed archive hashes are in [source-inventory.json](source-inventory.json).
The three [method scripts](methods/) are exact historical copies matching those
pins. `analyze.py` now reads those bundled methods and omits unused endpoint
group identifiers; `tabulate.py` takes an explicit output directory. Their
numeric methods are unchanged. The standalone [driver](simulation/src/main.rs)
now opts into `dynamic_radius` explicitly and uses a relative dependency path.
Selected artifacts are summaries of the original run, not a new capture.

Raw inputs are restricted; reproduction of official-data results requires
owner-provided access to the pinned archive tree. No download or collection
step is provided. Use `SOURCE` for its root (containing `recordings/headless`),
`REPO` for a clean checkout of the pinned source commit, and a **fresh external**
`OUT` directory. From this checkout, POSIX-shell example:

```sh
STUDY=research/fidelity/2026-10-26-arena-radius
export CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 RAYON_NUM_THREADS=2
export CARGO_TARGET_DIR="$OUT/target" UV_PROJECT_ENVIRONMENT="$OUT/venv"
mkdir -p "$OUT"
cp "$STUDY/PLAN.md" "$OUT/PLAN.md"
uv run --locked --offline --only-group tools python -B "$STUDY/analyze.py" --repo "$REPO" --source "$SOURCE" --study "$OUT"
uv run --locked --offline --only-group tools python -B "$STUDY/verify.py" --source "$SOURCE" --study "$OUT"
cargo run --locked --offline --manifest-path "$STUDY/simulation/Cargo.toml" -- "$OUT/simulation.jsonl"
uv run --locked --offline --only-group tools python -B "$STUDY/tabulate.py" --study "$OUT"
uv run --locked --offline --only-group tools python -B "$STUDY/select_results.py" --study "$OUT" --output "$OUT/selected"
```

The driver above targets the current checkout, with the historical controller
explicitly selected. For exact historical simulator reproduction, copy its
directory outside the repo and point its dependency to the pinned source commit;
the original driver, manifest and lockfile hashes are in
[artifact hashes](artifact-hashes.json). Source/driver packaging edits change
their hashes; compare numerical summaries separately. The scripts create outputs
exclusively and refuse file reuse. Keep raw/large outputs, targets and environments
outside Git. Use at most four logical CPUs; expensive stages run sequentially.

The original audit passed byte cross-checks on 53 archives, 10 protocol decoder
tests, the 790-sample radius replay regression and the synthetic driver's zero
unexpected-death assertion. These establish decoding/replay/driver consistency,
not a faithful controller. This implementation adds focused default/host and
checkpoint regressions; the PR records its actual build/test results. No new
official-data analysis or independent scientific review is claimed.

Placement verification reran the explicit-opt-in driver: all 1,802 saved rows
and the terminal summary match the original output exactly. The repackaged
tabulator reproduces every table value, and the selector reproduces all selected
JSON, including the redacted inventory. Every one of the 114 forecast row sets
was checked against its retained MAE summary. Full raw-data analysis and byte
audit were not rerun during placement; the checks above use retained outputs.
