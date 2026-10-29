# Reproduction and evidence access

Full receipts, source copies, binaries, process inventories and cloud requests
are **local-only**, outside Git. They include private endpoints and are not
publicly downloadable. Selected aggregates and the reducer are public; a digest
identifies an archive without granting access to it.

## Stage and build

Use a fresh external directory. Follow
[#258's linked-harness recipe](../2026-10-29-c7a-mature-capacity/REPRODUCE.md)
to stage `shared248/`, `shared256/`, `metrics.py`, `build.sh` and `setup.sh`.
Reuse that study's `prepare.py` as `prepare258.py`; do not copy earlier harnesses
into this study. The linked #256 archive recovers the earlier instrumentation;
this PR adds no patch files. Archive this study's runtime with:

```sh
git archive --format=tar --output=EXTERNAL/main.tar 165a5bb764aba6cbf39f42503e090f9b5e3c4d22
```

Place this study's `prepare.py`, `run_ladder.py`, `reduce.py`, `delivery.py` and `check.py`
beside those linked files. Run preparation with Python UTF-8 mode on Windows;
this is source extraction only, without a local build. Retain failed trees.
Cloud preparation starts in another fresh directory.

Use the #258 host agent with exactly these configuration changes: socket maximum
641 (640 bots plus one human), `simulation: {"world":{"preset":"battledome"}}`.
Omit `delivery`, preserving shipped responsive delivery. No other world override.
Record the exact private host-agent bytes and effective host config.

Build on the generator using the linked `build.sh`: Rust 1.99.0, locked release,
four build jobs and `RUSTFLAGS='-C target-cpu=x86-64'`, without inherited encoded
flags. No native Zen 5 instruction selection. Copy/hash the generated `bin/host`
to the c7a server through the private network. `pins.json` binds the runtime
archive, measurement overlay, client source, toolchain and both executables.
Only archived source copies receive measurement hooks; production files stay
unchanged.

## Provision and run

The [frozen plan](PLAN.md) owns the pair, budget, timer, access and stop rules.
Reuse the earlier private AWS controller only after inspecting it. Correct its
Spot cleanup matcher to use recorded instance/request IDs; a direct Spot launch
may omit the key name in `LaunchSpecification`. Do not refresh an expired login.
Check identity before any provisioning and throughout the task.

Install/verify 130-minute in-guest shutdown timers and terminate-on-shutdown
before building, plus one-time Spot/terminate-on-interruption and
DeleteOnTermination root disks. Use private IPs for game/control/artifact
traffic. Public management is source-restricted; public game ingress uses only
the supplied human /32. No endpoint or cloud identifier is public evidence.

Stage private `job.json` on both machines with `serverPrivate`,
`generatorPrivate`, `gamePort`, `controlPort` and launch time as `launchUnix`.
Run the linked host agent on the server, then this study's `run_ladder.py` on
the generator. Run `python3 check.py` before counting. The run records server
and generator command inventories around every measurement boundary.

The driver warms naturally, ascends the frozen seat ladder and takes a single
descending 80-bot confirmation. Every socket decodes received protocol-19
frames and invokes shared A4 with its received rules/view; it has no
authoritative input. Measure actual boost choice rather than forcing it.
The maintaining agent explicitly skipped the human hold for the final recovery
because the done signal exists. The private controller writes the requested
endpoint file after an authorized public probe. After counting and descending
confirmation, stop and package the complete evidence. Terminal readiness follows
archive close, gzip verification, SHA-256 and atomic archive/digest publication.
Only then download. Verify size, digest and required raw sources locally before
teardown. Retrieval failure preserves the source instances; the boot timer is
the final resource safeguard. Do not refresh an expired AWS session.

## Reduce and check

```sh
python -X utf8 check.py
python -X utf8 select_results.py PRIVATE_RAW EXTERNAL_STAGED_HARNESS selected-results.json IMMUTABLE_COLLECTOR_BUNDLE.tar.gz
python tables.py selected-results.json
```

`reduce.py` selects complete 100-tick interior blocks and takes quantiles of
aligned simulation-plus-socket CPU sums. Its quantile convention is the linked
harness's upper order statistic, index `ceil((n-1)*p)`. Nested phases stay nested;
socket CPU and simulation-other time complete the broad decomposition.
The selector reproduces every online result on its retained tick range, then
selects all complete interior blocks from the canonical server log. Buffered
stream draining may lag that log; retain original and canonical CPU aggregates
separately. Reconcile canonical mean CPU
against independently sampled task counters, verify configuration and binary
identity, ensure all workers decode/progress and retain human-affected windows.

Client receipt gaps use microsecond-floor histograms, with less than 1 us
quantization; frames during setup/death transitions are excluded. Primary
client reports require both their preceding report timestamp and end inside
the CPU interval. Preserve the original online end-only view, including mixed
boundary reports, separately. Client edges lie up to about one second inside
CPU endpoints. Waits include login enqueue
through first observed Playing frame and exclude the 28 ms reconnect delay
and that connection's preceding TCP/upgrade handshake. Nonblocking polling,
other connections' serialized handshakes and receipt scheduling remain included.
Pending requests are censored separately. Initial waits are cumulative completed
requests for logical IDs below each target; incremental joins are also retained.

The driver also accumulates the protocol elapsed-ms headers. Signed receipt gap
minus header increment separates normal sparse emission from arrival jitter.
Receipt time minus cumulative headers relative to a running minimum gives frame
excess; `RESET` resets that baseline before each counted window. Neither metric
identifies one-way or input-to-effect latency. The synthetic normal 210 ms gap
with only 5 ms excess must not count as a 100 ms delivery stall.

Expanded CPU/census values buffer until counting is disabled. Convert each JSON
value to one string before writing stderr, avoiding per-token writes. Keep all
raw deferrals, including uncounted flush intervals. The linked donor files
`build.py`, `client.rs`, `timing.rs` and `select.py` were checked byte-for-byte
against their #256 archived patch extraction before qualification.

The synthetic control uses deliberately anticorrelated CPU components: their
aligned sum remains 3 ms although component quantiles do not sum to that value.
Authored 72-tick spikes must dominate the slowest 1%, and synthetic overloaded
workers must fail the generator gate. These controls verify arithmetic, not
Battledome capacity or browser feel.
