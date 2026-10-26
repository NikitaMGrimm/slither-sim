# Collision capture tooling: local verification; official pilot not executed

No outside game server was contacted. No simulator rule changes are proposed.
The [pilot plan](plan.json) is exploratory and remains unexecuted; its two
ten-minute invocations and population scouting need later collection
authorization. This record distinguishes synthetic simulator outcomes from
future official evidence.

## Design and qualification

The coordinator starts two identities in verified appearance order and waits
for both sockets to close before replacing either. It meets near the first
identity's spawn, stages a short tandem body pass, and approaches lateral
offsets at factors 1.3, 1.1, 0.9 and 0.7 of `0.975 × body radius`, swapping probe
identity. It selects the probe's already occupied side before contact to avoid
crossing the reference body during staging. Four subsequent head-on cells swap
probe direction/identity and actual first-join identity. Every failed slot is
retained; the schedule advances without favorable-only retries.

Qualified encounters require both own streams and mutual body observations,
anchor ages at most 350 ms, own/peer head agreement within 25 units, matched
speed and size within 5%, unchanged lives/size/mass, no boost, prescribed heading
within 0.16 radians, control gaps at most 100 ms, and four live samples. Ladder
trials require longitudinal overlap with the reference body. Survival requires
a continuous 1.5-second hold within 0.6 units of the requested offset; contact
requires own game-over. Head-on launch additionally requires heading errors
under 0.06 radians and cross-track under 2 units. The 350 ms anchor bound allows
the ordinary 42-unit anchor cadence at cruise speed; it does not certify pose
accuracy. All raw incoming frames and actual sent bytes retain shared monotonic
timestamps, source/plan pins, and the existing gzip integrity receipt.

Any observed third-party body edge/head within 600 units aborts and restages;
the guard checks edges, not just anchors. Staging, run, arena and capture
deadlines censor trials. Peer removal may await own game-over for at most
100 ms after the last qualified sample; disappearance alone is not a contact.
Post-event recording retains both own death flags for one second. Full pose,
body, speed, size and control metadata remain linked to the raw streams.

## Local outcome curve

The deterministic `collision_curve` example uses the production `Simulator`
API with seven-segment straight bodies, matched cruise speeds and 7 ms steps.
It varies both probe identity and actual respawn/join order. It predicts:

| Offset factor | Absolute offset (world units) | Probe | Reference |
| --- | ---: | --- | --- |
| 1.3 | 19.245672 | survives | survives |
| 1.1 | 16.284800 | survives | survives |
| 0.9 | 13.323927 | dies | survives |
| 0.7 | 10.363054 | dies | survives |

Matched collinear head-on contact kills the oldest life and leaves the newer
alive; reversing join order reverses the dead identity. This tests the current
inferred order, not an official rule. `3.666` gates candidate checks and is not
the physical contact radius. A tandem ladder primarily probes the point band;
it does not independently recover that broad-phase constant or every swept
intersection case.

The final production localhost host regression completed **12/12 qualified
trials**: eight ladder trials (four survivals, four probe deaths) and four head-on
contacts, with both probe roles and all four join/direction cells. Each ladder
factor had two qualified observations matching the table. Contacts on the way
to smaller offsets are interval bounds, not deaths at the requested final
offset. [Selected results and source/archive hashes](local-results.json) retain
achieved head offsets, exact denominators, a repeated head-on counterexample,
and concurrent-run aborts. The host used the normal wire driver with no
teleports, debug state, artificial deaths or outside connection. Full archives
are local-only, outside Git, and are not published artifacts.

Verification passed: the complete tools group (260 Node tests; 198 Python
tooling tests; 237 Python fidelity tests; transport/bot/C ABI smoke checks;
wheel build and 28 wheel tests, one existing skip), the focused oracle with
default and core-only features, Clippy for the new example, formatting and
documentation checks. CPU affinity was restricted to four logical CPUs with
two Cargo build jobs and two Rayon threads. No independent agent review was
performed in this research sub-agent task.

## Reproduction and retained development failures

```sh
# Set CARGO_BUILD_JOBS=2 and RAYON_NUM_THREADS=2 in the invoking environment.
cargo build --locked --bin slither-sim
cargo test --locked --example collision_curve
cargo run --locked --example collision_curve
node --test tools/fidelity/slither-paired-collision.test.mjs
node --test tools/fidelity/slither-paired-collision.local.test.mjs
uv run --locked --only-group tools python -B -m unittest discover -s tools/fidelity -p test_collision_pilot.py
```

The localhost test retains its complete temporary directory on both success and
failure and prints its location. Early development runs exposed uncontrolled
staging contact, forced-side approach failure, heading quantization/clearance
error and head-on alignment failure. Those failures motivated the current
controller; they were not official evidence and do not count as pilot attempts.
One otherwise qualified repeated localhost head-on pass killed the newer life.
This is a retained counterexample to treating every approximately aligned
crossing as mutual contact: the 251 heading codes cannot express exactly
opposing rays. The initial test incorrectly asserted oldest death for every
wire crossing. It now asserts one death/survivor and records identity, while the
exact mutual-contact fixture independently asserts oldest-first resolution.
Neither qualification nor a death receipt establishes mutual geometry; the
pilot analysis must perform the frozen candidate/uncertainty screen.
The first full tools-group run, under concurrent tests on four logical CPUs,
retained a staging timeout and a lost-body-overlap abort (10/12 qualified).
It still supplied qualified contacts/survivals for every ladder offset, both
probe roles, and all four head-on cells. Requiring zero acquisition aborts made
the initial test overstrict. The regression now retains every scheduled slot,
requires those complete coverage witnesses, and checks every qualified outcome;
it does not loosen any driver qualification or retry selected offsets.
A subsequent harness startup failure exposed duplicate stderr buffering: a port
split across chunks could be read with repeated digits. The harness now buffers
each received chunk once and retains the host transcript even on startup failure.
Development captures whose source changed during collection remain quarantined
by the ordinary source-unchanged receipt, rather than relabelled as clean data.

Pilot preparation and the exact collection command are in [README.md](README.md).
No online collision law, fitted threshold, held-out validation or statistical
power is claimed from the local engineering checks.
