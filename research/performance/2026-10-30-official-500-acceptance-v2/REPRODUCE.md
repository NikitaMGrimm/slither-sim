# Reproduce v2

Reuse [v1's recipe](https://github.com/NikitaMGrimm/slither-sim/blob/55f2921594aff00044624e47a8a1f90c4ee0b073/research/performance/2026-10-30-official-500-acceptance/REPRODUCE.md)
and its linked c7a ladder/#256 files. Stage the original v1 `prepare.py`,
`acceptance.py`, `run_acceptance.py` and `check_acceptance.py` in `sharedv1/` from
commit `55f2921594aff00044624e47a8a1f90c4ee0b073`; use its `select.py`,
`host_agent.py`, `marker_tokens.py`, `check_markers.py`, `assemble.py` and
`tables.py` unchanged at the staging root. Add this study's files there. Do not
copy v1's raw results, source-hash maps or cloud identifiers into Git.

On the first fresh AWS generator, before warmup, run the portable locked build
on the pinned main archive. `prepare.py` applies v1 and this study's generator
changes. Retain one bounded Cargo target and remove it after build. Run:

```sh
rustc --edition=2021 --test background.rs -o background-test
./background-test
rustc --edition=2021 --test --cfg synchronous_mutation background.rs -o background-mutant
# Must fail: synchronous dispatch stalls the 42 ms decision schedule.
./background-mutant
python3 check_harness.py /srv/slither-study
python3 check_gates.py
python3 check_markers.py
```

The deliberate 500 ms connect and publication jobs must leave at least fifteen
other-seat decisions in 650 ms with no lateness reaching 42 ms. The mutation
executes those jobs synchronously and must fail. These are synthetic generator
contract checks, not host capacity evidence. Commit the final plan/harness after
these checks and before starting any host world. Verify the staged files match
that commit. No instabox or local Rust work is permitted.

Follow the frozen plan for both fresh pairs and the unchanged one-second gate,
including boundary-overlapping reports. Run the metric normalizer before
counting. Each worker keeps the same A4 policy, full received-frame path and
42 ms decisions. Its connector and publisher background threads inherit that
worker's generator CPU; the host and observer remain on the host's sole vCPU.
Connection completion records include close/connect elapsed time; ordinary and
histogram output is serialized away from decisions, with bounded queue depth.
Queue overflow or a worker failure fails collection rather than hiding load.

After collection, use the unchanged v1 selector/assembly/tables recipe. Keep
both runs separate, retrieve and verify each raw archive, and verify teardown
before launching another pair. The selector calls this study's `acceptance.py`,
which changes only final maturity to the prospective lower bounds; start screen,
deferrals, queues, input lateness, accounting and report-only tails are retained.
Finish by scanning every enabled AWS region and deleting the task key/group.
