# Regression fixtures

For a new case or changed expectation, start with
[selecting and updating cases](#selecting-and-updating-cases).

[manifest.json](manifest.json) inventories the existing wire/client protocol and
client-frame contract fixtures. Paths are relative to the repository root.
Generic mechanics fixtures are outside this inventory: their protocol must not
be guessed from the current decoder default. Recorded login challenge examples
live inline in `tests/client_challenge.rs`; there is no `login-challenges.json`.

`schemaVersion` versions the inventory format. Each entry separately pins
`wireProtocol` (the actual binary wire version, currently literal `19`) and
`clientContract` (`slither-client-v2` for the synthetic frame, otherwise `null`).
The client frame ID is distinct from the `slither-client-json-v1` request/reply
envelope and the geometric `slither-sim-json-v2` controller; see
[interface versions](../../docs/client-api.md#interface-versions).

`dataOrigin: captured` identifies retained packet bytes, with
`expectedOrigin: reference-decoder` identifying decoded expectations. Source
locations, hashes, reference implementations and limitations remain in each
fixture. Those expectations check decoder consistency; they are not official
server state or proof of server origin. The contract example instead has both
origins set to `synthetic`; its protocol field is example metadata, not captured
wire evidence. This inventory does not duplicate source hashes.
`client-food-interest.json` contains selected decoded receipt/pose witnesses,
with source identity and selection limits, rather than full raw frames.

## Changing a protocol or contract

The runtime default may advance independently. Do not derive historical tags
from `protocol::PROTOCOL_VERSION`, rewrite old packet bytes, or relabel old
expected values to make a new default pass. The inventory checks compare literal
historical tags with setup bytes and explicit fixture metadata, without importing
the runtime decoder or its current default.

Add a new named fixture, inventory entry and tag-validation test for a new wire
version or contract, with its own source/limitations or explicit synthetic label.
Retain older cases and their evidence. Decide explicitly whether the current
decoder supports or rejects each older version: keep replay assertions for supported cases, and add
explicit unsupported-version rejection tests for rejected cases. Retaining a
fixture does not promise runtime compatibility. Update tests deliberately if the
supported set changes; do not turn rejection tests into implicit support.

Currently the default remains 19. Rust's `setup_order_and_protocol_are_enforced`
tests protocol-11 rejection; Python's
`test_other_protocols_are_rejected_before_setup_changes_state` tests rejection of
11, 18 and 20. No additional decoder support is implied by this inventory.

## Checks

From the repository root, the inventory check uses only the standard library:

```sh
uv run --locked --only-group tools python -m unittest discover -s tools -p 'test_fixture_versions.py'
uv run --locked --only-group tools python -m unittest discover -s tools/fidelity -p 'test_slither_protocol.py'
```

The first is also discovered by the tools group's `test_*.py` check. For changes
to runtime decoding or client contracts, additionally run the relevant Rust
tests (these compile the networking client):

```sh
cargo test --locked --features networking --test client_protocol --test client_hud --test client_contract --test client_challenge
```

## Selecting and updating cases

Keep bounded selected cases in `tests/fixtures/`; keep full recordings and study
bundles outside Git. Add a case for each new scenario and preserve valid old
regressions. Avoid wholesale fixture reformatting. Record the source study/window
and distinguish raw recorded packets from decoder- or model-derived expectations;
label observed and synthetic inputs explicitly. Synthetic cases test a specified
contract, not an empirical server claim.

Expected results must have an independent justification: recorded bytes, a
specified contract, a checked calculation or supporting evidence. Regenerating
expectations from the changed implementation cannot by itself approve a regression.
If an expectation was wrong, document its independent correction and preserve the
original evidence. Deprecating a case requires a reason tied to the supported
contract or invalid evidence, with a replacement or retained rejection test where
applicable. A passing decoder cross-check is not independent proof of its model.

Keep a measured calibration gate tied to its recorded corpus. Test configurable
mechanisms with deliberately synthetic parameters, so a new supported default
does not require changing many unrelated lifecycle tests. Protocol bytes,
rejection cases, invariants and frozen-corpus selection checks stay explicit.
New conflicting observations need a new corpus and explained regime/supersession;
replacing old data merely to make the current model pass hides a regression.

For a new regression, retain a small selected raw packet/window with the archive
SHA-256, record indices, wire version, decoder identity and selection rule. Derive
expected fields with the existing decoder or replay exporter, label that derivation,
then have the focused test consume the fixture. Keep the full audited study outside
Git; the [worked study](https://github.com/NikitaMGrimm/slither-sim/blob/main/tools/fidelity/worked_study.py) demonstrates this path.

The standalone replay exporter hashes and decodes one temporary archive snapshot.
Its schema-1 `provenance` retains the archive, selected window/connection, wire
protocol and decoder/exporter source digests. These additional metadata fields
leave replay values unchanged; older fixtures keep their original provenance.
The exporter checks gzip completion and window coverage, but does not perform
the full capture/study audit. Its output labels that limit explicitly; audit the
study separately before treating the derivative as evidence.
