# Collision delay study

The [frozen plan](plan.json) and [digest](plan.sha256) precede every official
game connection in this study. [Offline validation](OFFLINE.md) separates
synthetic sensitivity, wire acquisition and timing limits. The execution and
verdict are in [REPORT.md](REPORT.md) and [selected results](execution-results.json).
The earlier [failed collision pilot](../2026-10-26-collision-pilot/EXECUTION.md)
remains a separate failed study.

These commands make no outside connections:

```sh
# Restrict the invoking process and its children to two logical CPUs.
# Also set CARGO_BUILD_JOBS=2 and RAYON_NUM_THREADS=2.
cargo build --locked --bin slither-sim
cargo test --locked --example collision_delay
cargo run --locked --example collision_delay
node --test tools/fidelity/collision-delay-preflight.test.mjs
```

For the real-time **loopback-only** regression, set `SLITHER_DELAY_E2E=1` and
`SLITHER_DELAY_MS=450`, then run:

```sh
node --test tools/fidelity/slither-collision-delay.local.test.mjs
```

The test retains its complete temporary directory on success and failure.
It uses the production host, two socket clients, no background snakes, no
teleports and no injected deaths. `DelayBot` records raw socket times before
decoding and applies ordered queues in both directions. Its undelayed raw tap
is for safety and independent analysis; the delayed controller does not use it
to steer. The existing raw capture auditor checks these recordings.

`collision_delay` moves the two synthetic snakes in separate production
Simulators and scores their complete histories using `physics::strikes`.
The post-hoc scorer can rewind only the reference, both states, or neither;
the hosted simulator never rewinds. [Selected synthetic inputs and outputs](synthetic-controls.json)
include every contrast, both anchor phases and all uncertainty sensitivities.

Historical official scouting used the private, retained raw list response:

```sh
node tools/fidelity/collision-delay-scout.mjs PRIVATE_LIST plan.json NEW_PRIVATE_OUTPUT
```

That command makes official connections and requires the frozen offline gate,
list digest and source digests to match. It never runs the official client
JavaScript or opens its cluster-ping sockets. The research plan is specific to
this direct collector; it is not a new managed `collect.py` scenario. Raw members
use the existing gzip JSONL format and `capture_quality.audit_captures`;
source/transfer tarballs are not `evidence.py`-verified bundles. A future
collection needs a new frozen plan under the research workflow.

Full raw bytes, sent bytes, rejected attempts, private discovery response,
original source exports and complete audits are retained outside Git with
hashes. Access is local-only, not a public download. Published aliases omit
game endpoints and personal/infrastructure details.
