# Host efficiency comparison plan, 2026-10-26

Frozen before AWS timing. Synthetic local-simulator traffic only; no outside
game servers. The desktop is reserved for correctness builds/tests, with two
Cargo jobs and at most four test/Rayon workers. No desktop timings are used.

## Sources and candidates

- Base: current main at `9480608` (includes #116 and #117). The capacity study
  in #118 measured `6bcb6f0`; its profile motivates candidates but is not the
  comparison baseline. Read report and selected results at study head
  `3d10f2922b5c51ec4229b937929137392c256528`.
- `e9abf60`: retain food subscription storage and Arc handles when subscriptions
  are unchanged; preserve the existing ordered merge when membership changes.
- `128ca12`: one broadcast-local peer delta per snake, keyed by the complete
  prior `KnownSnake`; own updates bypass the cache. No report gates change.
- `08b1fad`: transfer owned delivery frames to tungstenite, overwrite sequence
  headers in place and slice bare headers from owned bytes.

## Correctness gate

Before any timing, require focused regressions and the CONTRIBUTING Rust group,
including client/server/transport tests. Compare complete decompressed traces
from independently built base/candidate binaries, with identical harness bytes:

- `perf_baseline` portable, protocol, client-official-500 and
  client-battledome-64, each at its catalog trace window, using `--trace`.
- `perf_client_retained` step_all and next_frame, 384 decisions with `--trace`,
  including the manual reconnect at decision 192 and every slot's wire records.

Trace identity is a finite deterministic regression, not proof over every input.
Retained traces do not expose private backend queues/RNG/subscription state;
existing scheduling, food, decoration and socket tests cover those invariants.

## Measurement

One one-time Spot c7a.medium server and c7a.4xlarge generator, same availability
zone in eu-central-1. Use Rust 1.99.0, locked release builds, thin LTO, one codegen
unit and line-table debuginfo. Record source, harness, binary and raw data hashes.

On the quiet generator, pin the parent/children to one core and run
`tools/perf_compare.py --base 9480608 --cases client-official-500
client-battledome-64 official-100-step step-default --repeats 5 --no-profiles`.
The tool checks traces before adjacent alternating pairs; the last two rows are
unaffected physics controls. Timings are descriptive; mixed/inconclusive effects
support no speedup claim.

Then use #118's unchanged probe, generator, telemetry and analysis tools:

1. Base confirm-250: 15 s warmup, 300 s hold.
2. Candidate confirm-250: 15 s warmup, 300 s hold.
3. Candidate sweep-500: 15 s warmup, 120 s hold.
4. Base sweep-500: 15 s warmup, 120 s hold.

Report total host CPU seconds per executed tick, ticks/s, new deferred ticks,
pooled excess p99, active population and generator errors. Do not infer a
capacity gain from CPU alone or compare these sources directly to #118's older
runtime. One hold per variant/size is not a long-run tail guarantee.

## Bounds and cleanup

17 vCPUs total, one-time Spot only, root volumes delete on termination, no extra
volumes or other infrastructure. Price caps and a 120-minute self-termination
timer bound compute plus ancillary allowance below the $3 target and $5 ceiling.
The planned window is 90 minutes plus 30 minutes safety. SSH ingress is limited
to the operator's current IP; game ingress is restricted to the generator group.
Tag resources Project=slither-sim, Task=host-opt, ExpiresAt=UTC deadline. Record
credits before/after. On error terminate before debugging. Always wait for
termination, remove key/groups and verify regional resource inventories.

Raw cloud metadata, keys, addresses, traces and samples stay under local-only
`C:\Users\Nikita\.t3\scratch\host-opt-aws\`; publish only sanitized selections.
