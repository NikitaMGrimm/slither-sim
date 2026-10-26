# Host transport with 500 background bots, 2026-10-26

The pooled 500-bot arm passes all four unchanged gates, but **two of its three
captures fail**, including the held-out repetition. This study therefore does
not establish consistently official-like transport at the target load. The
sparse baseline fails median frame cadence in all three repetitions. No host
behavior or tolerance is changed.

## Question and scope

Does the protocol-19 host meet the existing near-server official transport
gates while configured with 500 background bots and two bot workers? This is
a packet-timing study using a stock-style headless client, separate from CPU
qualification and stock-browser renderer testing.

Data origin: **local-simulator**. No outside game servers were contacted.
The official comparator is the existing six-capture, unfiltered
[`transport-official-near-server.json`](../../../tests/fixtures/transport-official-near-server.json),
whose [reference audit](../2026-10-26-server-timing-references/REPORT.md)
records calibration, failed attempts and limits. No tolerance was changed.

## Frozen method

[Plan](plan.json), [plan digest](plan.sha256), [source pins](source-pins.json),
[runner](run.py), [bootstrap](bootstrap.sh) and [offline analysis](analyze.py).
The [analysis source pins](analysis-source-pins.json) were saved before
inspecting held-out timing results; the published bootstrap matches the actual
instance user-data byte for byte.
The measured source is origin/main at
`1ba386f543422e1df9e1a60b0a1ce229d40055a8`, version 0.5.0.
Before publication, origin/main advanced to
`503ca4df8104321c7b0e5c28cc1f628c1dd6cc48` with only the separate CPU
qualification study and its index entry. The
[Git object comparison](selected/main-equivalence.json) verifies identical
Rust/build inputs, tools, tests, fixtures, schemas and lockfiles. These results
therefore cover the current main runtime and measurement tool bytes; collection
itself remains labeled with its actual measured commit. The PR is based on the
new main and carries neither stale local test commit.
Rust 1.99.0, locked release build, two Cargo jobs; no desktop Rust build or
timing collection. The unchanged `local_capture.py` accepts full host configs,
so no shared tool or runtime extension was needed.

Six serial five-minute captures, alternating 0 and 500 background bots for
three repetitions. Each capture restarts the host. Both arms use official
delivery, four reserved socket seats, zero added player/bot latency, two bot
workers and seed `20261025 + repetition`. The collector is the unchanged
single-client forage flow, including ordinary deaths and rejoins. No startup,
stall or telemetry interval was removed; the tool's
existing anchor speed-window eligibility remains unchanged. There is no
responsive arm.

The third repetition of each arm was reserved as held out before acquisition.
Development summaries were hashed after repetition two, before either held-out
capture started. No parameters were fitted or changed using development or
held-out outcomes. Pools combine underlying samples, never per-capture
quantiles. All captures, gate failures and sample shortages are retained.

One disposable eu-central-1 one-time Spot c7i.4xlarge, 16 vCPUs / eight physical
cores. The runner selects six separate physical cores for housekeeping,
simulation, sockets, bot worker zero, bot worker one and collector. A study-local
Node launcher sets and verifies host thread affinity after readiness and before
starting collection; Node and its workers inherit the separate collector core.
The sequence runs independently under cloud-init, with short retryable SSH
status/retrieval operations. Instance shutdown means terminate, the root volume
deletes on termination, and a hard 120-minute shutdown timer bounds outages.
SSH is restricted to the operator address; game traffic binds loopback only.

## Results

Values are rounded to six decimals for display; all comparisons use full
stored precision. Column headings give the unchanged upper limits. Bold
values fail that gate. Every gated statistic exceeds the minimum **100** samples.
Anchor n applies to both anchor gates; frame n applies to cadence and delay.
Development pools contain repetitions one and two; full pools contain all three.
See [full-precision selected results](selected/results.json) and
[raw archive inventory](selected/raw-manifest.json).

### Official delivery, 0 background bots

| Capture / pool | Anchor n | Frame n | Anchor p50 <=0.952024 wu | Anchor p95 <=2.494097 wu | Frame p50 <=9 ms | Delay p90 <=1.183000 ms | Grade |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| r1 | 1259 | 8609 | 0.496230 | 1.424062 | **35.000000** | 1.031000 | FAIL |
| r2 | 1260 | 8714 | 0.488859 | 1.407530 | **35.000000** | 0.998000 | FAIL |
| r3 (held out) | 1263 | 8917 | 0.497732 | 1.408713 | **35.000000** | 1.027000 | FAIL |
| Development pool | 2519 | 17323 | 0.493955 | 1.417980 | **35.000000** | 1.021000 | FAIL |
| All-three pool | 3782 | 26240 | 0.494648 | 1.414182 | **35.000000** | 1.023000 | FAIL |

### Official delivery, 500 background bots

| Capture / pool | Anchor n | Frame n | Anchor p50 <=0.952024 wu | Anchor p95 <=2.494097 wu | Frame p50 <=9 ms | Delay p90 <=1.183000 ms | Grade |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| r1 | 8018 | 28160 | 0.522945 | 1.469076 | 7.000000 | 1.147000 | PASS |
| r2 | 4560 | 22133 | 0.565376 | 1.543243 | **14.000000** | 1.154000 | FAIL |
| r3 (held out) | 7196 | 26663 | 0.542764 | 1.440729 | 7.000000 | **1.209000** | FAIL |
| Development pool | 12578 | 50293 | 0.544125 | 1.489151 | 7.000000 | 1.150000 | PASS |
| All-three pool | 19774 | 76956 | 0.543793 | 1.475517 | 7.000000 | 1.168000 | PASS |

### Exposure and collector diagnostics

| Arm | Repetition | Setup epochs | Leaderboard samples | Living population min / median / max | Collector late ticks | Collector maximum tick gap, ms |
| --- | --- | ---: | ---: | --- | ---: | ---: |
| 0 bots | r1 | 1 | 99 | 1 / 1 / 1 | 0 | 28.443157 |
| 0 bots | r2 | 1 | 99 | 1 / 1 / 1 | 0 | 28.362097 |
| 0 bots | r3 | 1 | 99 | 1 / 1 / 1 | 0 | 28.182553 |
| 500 bots | r1 | 2 | 98 | 501 / 501 / 501 | 0 | 27.056100 |
| 500 bots | r2 | 2 | 98 | 500 / 501 / 501 | 0 | 28.595464 |
| 500 bots | r3 | 1 | 99 | 501 / 501 / 501 | 0 | 28.392549 |

Population counts include the socket player when alive; they are periodic
leaderboard observations, not continuous minimum-population telemetry. All
500 background seats remain configured throughout each loaded capture.
The collector tick-gap diagnostic is its own approximately 25 ms control
loop, not the 7 ms server tick or a one-way delay estimate. No diagnostics
were used to mask timing samples. Each native capture passed the existing
gzip, terminal-integrity and provenance audit; ordinary reconnect warnings
are retained in the selected results.

The acquisition ledger contains **eight** reservations: six valid captures
and two pre-capture errors. An exit 1 marks a retained transport-gate failure,
while exit 2 marks acquisition failure. The ledgers retain each plan identity
and a portable rendering of each command. No valid capture was replaced.

## Interpretation and limits

At 500 bots, r1 passes all four gates; r2 fails server frame-gap p50
(14 > 9 ms); held-out r3 fails excess-delay p90 (1.209 > 1.183 ms).
The held-out excess is about **0.026 ms** above the unchanged limit: it remains
a failure, not evidence of a visible 26-microsecond pause. Pooling the three
captures gives 7 ms and 1.168 ms respectively and hides both repetition-level
exceedances. Anchor p50/p95 pass in every valid capture of both arms.
The baseline's 35 ms cadence failure also persists in its development and
all-three pools. The frozen all-gates hypothesis is not supported.

The four gates are exploratory, one-sided upper limits, with at least 100
samples per gated statistic. Lower values are permitted. Passing them means
the sampled host does not exceed these particular correction, median cadence
and excess-delay envelopes. It does not show distributional equality or recover
the official server's internal scheduler.

An anchor-correction failure means larger interpolation reset corrections than
this reference envelope: a stock client could have more smoothing work and
visible jitter. A frame-header median failure means the sampled release cadence
is slower than the gate permits. An excess-delay failure means more variation
in receipt-minus-header timing, potentially producing compressed updates or
pauses. These outcomes would also limit confidence in the timing of observations
used for training, without measuring policy quality or training transfer.
Gate failures alone do not identify simulation overload, OS scheduling,
collector work or an encoder schedule as their cause.

The frame gate measures aggregate transmitted frames, not executed physics
ticks. The pinned [broadcast implementation](https://github.com/NikitaMGrimm/slither-sim/blob/1ba386f543422e1df9e1a60b0a1ce229d40055a8/src/client/simulation/broadcast.rs#L401)
does not emit an empty packet set. Sparse and populated worlds therefore have
different opportunities to produce a frame. This source fact limits causal
interpretation of their median difference: a sparse-arm cadence failure alone
does not demonstrate slow physics or show that adding bots improves scheduling.

The headless client does not render the stock browser's smoothing, animation,
input loop or death fade. No renderer, input-to-display, absolute one-way
latency or multi-client fairness parity is established. The delay gauge is
the unchanged centered +/-600-frame minimum, not the reference study's separate
slope-corrected +/-10-second characterization. Correlated overlapping anchor
windows and frames are not independent trials. The reference's false-reject
control is only its six-capture LOO calibration; there is no population error
rate guarantee or independent held-out validation of the reference envelope.
Our held-out repetitions test repeatability on this machine, not that envelope's
population validity.

The default arena is sized to reserved seats, so the arms have different world
sizes and food targets. This measures the intended full configuration rather
than isolating a causal CPU cost of adding bots to one fixed world. Five-minute
captures on one VM/date do not establish long-run tails, other hardware,
congested networks or an unpinned production host. Configured background seats
and periodically observed living population are reported separately.

## Reproduction and evidence access

On a disposable Linux machine with six available physical cores, Rust 1.99.0,
Node 24.10.0, Python and the repository's locked tools environment:

```sh
git checkout 1ba386f543422e1df9e1a60b0a1ce229d40055a8
# Overlay this dated study directory from the study PR.
CARGO_BUILD_JOBS=2 cargo build --release --locked -j 2
uv run --locked --only-group tools python -B -m unittest discover -s tools/fidelity -p test_transport_parity.py
python3 research/fidelity/2026-10-26-host-transport-500-bots/analyze.py /external/retained-results /external/new-analysis
```

The analysis command expects the original external result directory and checks
its frozen source pins, plan identities, raw archive integrity, development seal
and exact equality with both instance-produced pooled reports. To repeat a
single arm/capture, save its `config` object from `selected/results.json` as a
new JSON file, and run the unchanged flow on that config:

```sh
uv run --locked --only-group tools python tools/fidelity/local_capture.py --config capture-config.json --seconds 300 --out /external/new-capture
```

Use the distinct published ports, original alternating order and physical-core
pinning for a comparable new series. The initial `run.py`/`bootstrap.sh` preserve
the original failed port-reuse sequence; they are historical acquisition source,
not a corrected all-in-one command. `continue2.py` resumes the retained external
directory after the two failed acquisition reservations; it is not a fresh-run
entry point. A new collection needs its own frozen plan and output directory.

Raw archives, native run/config/measurement files, software metadata and cloud
operation records remain **local-only**, outside Git. Their digests identify
retained bytes and do not make them downloadable or authenticate observations.
The committed bounded summaries contain no raw gameplay frames, cloud resource
identifiers, public addresses, hostnames or personal paths. The frozen plan and
selected results explicitly label synthetic data as local-simulator; the
unchanged native local-capture flow does not pass a plan into the collector, so
the raw capture provenance does not itself embed this study's plan digest or
origin label. External frozen pins, native run metadata and the durable attempt
ledger provide that association; this is not a claim of a verified v1 evidence
bundle with collector-embedded plan binding.

## Retained acquisition failures and amendments

The initial baseline finished and failed the cadence gate. The immediately
following 500-bot attempt stopped before acquisition with
`[Errno 98] Address already in use`. Repeated `local_capture.py` invocations
using one port can encounter the prior connection's port-reuse window: its
preflight probe binds without `SO_REUSEADDR`. This is an acquisition-tool
failure, not a sampled loaded-host timing failure. The original runner stopped
at exit 2, preserving its baseline and failed attempt.

The [first continuation plan](continuation-plan.json) kept the valid baseline
and assigned distinct ports to the remaining captures. Its detached shell
launcher first failed on CRLF line endings before starting an attempt; after
that correction, a capture attempt exited 2 before host startup because
`sudo`'s restricted PATH could not find `cargo`. The
[second continuation plan](second-continuation-plan.json),
[runner](continue2.py) and [launcher](continue2-run.sh) preserve those failures
and execute as the study user, setting PATH inside that service. A root-owned
diagnostic log also blocked one launcher start before any capture reservation;
the corrected launcher writes a new log. These setup failures produced no raw
capture and never justify dropping or replacing the valid baseline gate
failure. No shared tool, tolerance, seed, capture duration, held-out allocation
or timing formula changed. Both frozen continuation plans predate their
respective replacement attempts. The old [continuation runner](continue.py)
remains as historical source.

The initial frozen plan/source-pin files retain their exact CRLF bytes through
the scoped Git attributes; normalizing them would invalidate their pre-capture
digests. New scripts and analysis outputs use LF.

## Verification and cloud cleanup

The AWS locked release build, all **18** transport regressions and all three
plan-schema validations passed. All **7** documentation checks passed after
adding the required research-index link. Local analysis independently reproduced
both instance-produced pooled summaries exactly and verified every raw gzip,
terminal receipt, provenance record, source pin and development seal. The final
analysis source matches its pre-held-out-inspection pin. All timing collection
ran on AWS; local work used single-process analysis and no Rust builds.
No shared Rust/tool behavior changed, so the full Rust and tools groups were not
rerun. Scientific status remains unreviewed; integrity checks do not approve a
fidelity claim.

The downloaded external result archive matched its instance-produced SHA-256
`7c8c6a469980e590cea343ea2424a1dffad914fae937e1716ca92da32e3c0000`.
The retained native directory also has byte-identical `raw/<attempt>.jsonl.gz`
aliases matching the ledgers' export-member names. The selected raw inventory
uses native acquisition paths. Logs from the failed initial/continuation runners
and cloud-init remain local-only with the captures.

Observed Spot price was **$0.3719/hour** throughout the run. A conservative
40.50-minute runtime bound gives **$0.251 compute**, plus a **$0.10 ancillary
allowance**, for approximately **$0.36 total**, below the **$8 cap**. See
[spend estimate](selected/spend.json); this is usage-based, before credits/tax,
not a settled invoice. The $0.60/hour bid ceiling and hard two-hour timer kept
the planned resource envelope below the cap even under a controller outage.

After verified retrieval, terminated only this study's instance, removed its
key pair and security group, and confirmed root-volume and primary-interface
deletion. The first regional inventory still saw the Spot request during its
termination transition; a later read confirmed cancellation. The final
[regional cleanup inventory](selected/cleanup-verification.json) checks
eu-central-1, us-east-1, us-west-2 and eu-west-1. No resources from the separate
CPU qualification study were modified. Terminated-instance/cancelled-request
history is retained by AWS and is not a running or billable resource.
