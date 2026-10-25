# Protocol-19 transport reference and local baseline

## Selection fixed before measurement

Source access: **local-only** retained recordings under
`C:/Users/Nikita/Source/slither-arena-rs/recordings/headless/`. Do not use
`recordings/community/`. No new outside-server collection is part of this study.

Enumerate every `**/*bot*.jsonl.gz` under that headless root, sorted by relative
path. Include every archive that passes the existing census's complete gzip,
JSONL, receipt clock, literal protocol-19 setup and capture-end validation, and
the transport tool's elapsed-header decoder/state reconstruction. Retain each
excluded path, SHA-256 and exact error in the reference. Do not select on metric
values, scenario, duration, snake size or network quality. Missing metrics remain
missing. The reference's `captures` and `excluded` arrays are the complete input
manifest, with relative filenames and SHA-256s; raw traffic stays outside Git.

Of 58 archives, 53 passed; five older captures lacked `capture-end`. Their exact
paths and SHA-256s are in `excluded`; no numerical gate influenced exclusions.

The 58 input headers name these IPs, all from the retained official headless
collection: `94.72.180.82`, `162.19.235.91`, `103.4.30.88`, `198.244.231.35`,
`95.216.38.155`, `57.129.37.42`. This is historical collection provenance, not
verification of who operates those addresses today. The report also retains
server addresses seen in connection metadata. No community captures were read.

Each gate uses the maximum eligible per-capture official quantile. Eligibility
requires at least 100 samples for that metric. No additional spread allowance;
the tolerance JSON records the formula and rationale. Lower values are allowed.
This is an exploratory observed envelope, not a confidence bound, held-out fit
or proof of stock-renderer equivalence. The reference is calibration data, not
independent validation of itself. Short smoke captures remain in the manifest.

## Reproduction

From the repository root in PowerShell (the raw captures require local access):

```powershell
$captureRoot = 'C:\Users\Nikita\Source\slither-arena-rs\recordings\headless'
$captures = @(Get-ChildItem -LiteralPath $captureRoot -Recurse -Filter '*bot*.jsonl.gz' | Sort-Object FullName | ForEach-Object FullName)
uv run --locked --only-group tools python tools/fidelity/transport_parity.py measure @captures --relative-to $captureRoot --summary-only --skip-invalid --output tests/fixtures/transport-official-reference.json
```

The output path must not exist. `measure` produces the fixture directly; there
is no hand-edited selection or statistic. It pins tool, decoder and census
SHA-256s. The summary-only option omits per-connection rows, header histograms
and packet-family rows; ordinary measurements retain them. Sample pooling happens
before quantiles, never by averaging quantiles. Comparison calibrates its limits
across captures, not pooled samples.

## Limits

Adjacent anchor windows and simultaneous snake observations are correlated.
Receipt time includes network jitter and collector stalls. Server-header time
can expose batching but is not a synchronized wall clock. Delay is excess above
a local rolling minimum, not one-way latency; it includes host and collector
effects and can remove slow drift. Setup frames are excluded from delay samples.
The tool reports `collector-timing` events and final `collectorTiming` cumulative
counters when available; older captures without them are marked unavailable.
They are not silently screened out. Frame rates include quiet/post-death periods
after setup. Motion gaps include zero and long intervals, unlike the exploratory
prototype's trimmed display. Unknown/malformed state packets fail measurement
rather than being silently skipped.

The anchor formula and 53-frame correction interpretation were adapted from
local exploratory scripts in `C:/Users/Nikita/.t3/scratch/jitter-study-20261025/`
(`tailjitter.py`, `delayspread.py`, `rotgaps.py`, `REPORT.md`). Synthetic regression
captures establish the measurement contract, not official mechanics.

## Local before baseline

The localhost-only 60-second run against task-start main commit
`4f8d926b85daaf3f056da32453dcab42eaef421d` exited **1**, the expected gate failure.
The host implementation is unchanged on this tooling branch. Raw capture,
host/collector logs, frozen config, Git head, collector hash, measurement and
comparison are retained in `outputs/transport-parity-before-final/` (local-only).
This Windows run inherited affinity to four logical CPUs, with Cargo `-j 3`
and `RAYON_NUM_THREADS=2`; no tests/builds ran concurrently with acquisition.
It used one client, 64 bots, seed 7, fixed radius 5000, 4000 food pellets,
89.34 ambient placement attempts/s, 7 ms ticks, one prey, `broadcastTicks: 6`.
This bounded load is not a 480-bot production workload.

```json
{"bind":"127.0.0.1:18766","seed":7,"maxClients":1,"bots":64,"simulation":{"world":{"radius":5000,"dynamicRadius":false,"foodCount":4000,"ambientReplenishPerSecond":89.34,"dt":0.007,"prey":{"count":1}},"broadcastTicks":6}}
```

Save that JSON as `outputs/transport-host.json`, then run:

```sh
uv run --locked --only-group tools python tools/fidelity/local_capture.py --config outputs/transport-host.json --seconds 60 --out outputs/transport-parity-before-final
```

| Metric | Official range | Before | Upper limit | Result |
| --- | ---: | ---: | ---: | --- |
| Anchor receipt absolute p50 (world units) | 0.495–0.941 | 2.495 | 0.941 | FAIL |
| Anchor receipt absolute p95 (world units) | 1.739–7.599 | 6.221 | 7.599 | PASS |
| Server frame-gap p50 (ms) | 7–14 | 42 | 14 | FAIL |
| Excess delay p90 (ms) | 2.749–13.771 | 7.190 | 13.771 | PASS |

There were 1617 valid anchor-correction samples and 1604 post-setup frame/delay
samples. The collector reported 58 timing snapshots, zero late ticks and
maximum tick gap 42.2303 ms (its late threshold is 75 ms). All 53 accepted
official captures predate collector timing metadata; that absence remains
unknown. The full official spread makes p95 and delay gates permissive: two
passes here do not refute the p50 correction and batching failures.

Source identities for this acquisition:

- Capture SHA-256: `c1aeb55c5bac059caf353d94ed78aff3a51d78ec4c4de0dd8135f7314661db35`.
- Release executable SHA-256: `d261d9a064ec6cc386b875280464018209f75f437ebcc043a67d2db034487d0b`.
- Frozen config SHA-256: `c9ceb956a12e107469c3c9681957fa9795c6794fe977743f1ad9667d2505c961`.
- Collector SHA-256: `965acd0bda786ab1fc05a2c46f20b1bfb70de091dfea543d24bfa35d7b153ba3`.

The acquisition's measurement pins the then-current tool SHA-256
`07c6e693227457e998f662b234d2c764c6cbc18d3c30d023432177b766c28ff4`.
The final tool adds corrupt-deflate error handling without changing metrics;
the reference fixture pins the final tool independently. This baseline is a
single local observation, not a repeated or held-out transport study.

## Tool development attempts

The first single-capture smoke command used an incorrect census field name
(`headerFree`) and failed with `transport parity failed: 'headerFree'` before
writing output. The census exposes `framing`; corrected and the same capture
measured successfully. No capture data was altered.

The first local command saved its complete 60-second capture and measurement,
then comparison failed because the reference path was temporarily absent during
compact reference regeneration. It is preserved in
`outputs/transport-parity-before/`. The runner now reads reference/tolerances
before starting the host, so missing inputs fail before acquisition. A fresh
end-to-end run is recorded above.

The first full tools-group attempt ran while the compact reference was being
regenerated. Its documentation link check correctly failed because the fixture
path was temporarily absent; both fidelity suites and all 253 Node tests passed.
The complete group was rerun after reference generation finished. This was a
task orchestration race, not a loosened check or an omitted fixture.
