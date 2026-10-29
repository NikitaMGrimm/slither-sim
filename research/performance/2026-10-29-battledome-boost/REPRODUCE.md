# Reproduce the high-duty boost follow-up

Start from [#262's reproduction](../2026-10-29-battledome-experience/REPRODUCE.md)
in a fresh external staging directory. Reuse its linked `shared248`, `shared256`,
portable build, host agent, metrics, canonical reducer, archive delivery and
synthetic controls. Keep its original runtime archive, host configuration,
affinity, clocks, decoder, policy and every-frame read behavior.

Stage #262's `prepare.py` as `prepare262.py`, and `run_ladder.py` under
`shared262/run_ladder.py`. Add this study's `prepare.py` and `run_ladder.py`.
Preparation changes only the archived research example's boost output and adds
a native heading/rule-gate regression. Run this before counting on the generator:

```sh
cargo build --release --locked --bin slither-sim --example aws_clients
cargo test --release --locked --example aws_clients high_duty_preserves_heading_and_rule_gate -- --exact
```

Use the unchanged portable flags and bounded target from the linked build.
The build target is deleted only after copying/hashing executables. Later local
code checks are separately limited to `-j 1`; no local collection build is needed.

The wrapper runs exactly 80/160/320, retaining every overload row and stopping
for generator/queue/treatment invalidity or process failure. No human hold.
The private controller uses planned 45-minute duration, verified 75-minute boot
timers, $2 cap and the linked source-preservation guard. It strips receipt
histograms only from remote progress responses to bound public management
traffic; complete remote summaries and raw histograms stay in the verified
artifact. Do not refresh expired credentials.

After verified retrieval and teardown, reuse #262's selector and helpers:

```sh
python -X utf8 select_results.py PRIOR_SELECT_RESULTS.py PRIVATE_RAW EXTERNAL_HARNESS selected-results.json COLLECTOR_BUNDLE.tar.gz PRIOR_SELECTED_RESULTS.json
python -X utf8 tables.py PRIOR_SELECTED_RESULTS.json selected-results.json
```

`select_results.py` invokes the earlier canonical selector and adds the received-rule
treatment, unchanged shared-policy hashes, >=70% boost screen and baseline
identity. It compares the actual host binary hashes rather than assuming they
match. The tables read #262's retained rows directly; do not copy its raw data
or earlier harness implementations into this study. Keep every failure and
inaccessible cleanup region explicit.
