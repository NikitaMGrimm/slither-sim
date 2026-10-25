# Regression fixtures

Start new cases or changed expectations with
[selecting and updating cases](#selecting-and-updating-cases).

[manifest.json](manifest.json) inventories wire/client protocol and client-frame
contract fixtures, with repository-relative paths. Generic mechanics fixtures
are excluded; never guess their protocol from the decoder default. Recorded
login challenges are inline in `tests/client_challenge.rs`, not `login-challenges.json`.

`schemaVersion` versions the inventory. Each entry pins
`wireProtocol` (the actual binary wire version, currently literal `19`) and
`clientContract` (`slither-client-v2` for the synthetic frame, otherwise `null`).
The frame ID differs from the `slither-client-json-v2` request/reply
envelope (also pinned with removed-field requests in `client-contract-v2.json`)
and the geometric `slither-sim-json-v3` controller; see
[interface versions](../../docs/client-api.md#interface-versions).

[`serve-errors-v3.json`](serve-errors-v3.json) pins the geometric controller's
`slither-sim-json-v3` error envelope and a synthetic overflow reproducer for
Rust/Python lifecycle tests. Without a wire protocol or client-frame contract,
it stays outside this inventory. Error text is illustrative; tests check literal
protocol and `sessionEnded` independently of runtime defaults.

`dataOrigin: captured` labels retained packet bytes;
`expectedOrigin: reference-decoder` labels decoded expectations. Each fixture
owns source locations/hashes, reference implementations and limits. Expectations
check decoder consistency, not official server state or server origin. Both
contract-example origins are `synthetic`; its protocol is example metadata, not
captured wire evidence. The inventory does not duplicate source hashes.
`client-food-interest.json` holds selected decoded receipt/pose witnesses with
source identity and selection limits, rather than full raw frames.

[`transport-official-reference.json`](transport-official-reference.json) is a
separate wire-19 measurement summary (`schemaVersion: 1`), outside the packet
fixture inventory. It contains input paths/hashes, parse exclusions and
decoder-derived transport quantiles, without raw gameplay bytes. See its
[selection and limits](../../research/fidelity/2026-10-25-transport-parity/REPORT.md).

## Changing a protocol or contract

Runtime defaults advance independently. Never derive historical tags from
`protocol::PROTOCOL_VERSION`, rewrite old packets or relabel expectations to pass
a new default. Inventory checks compare literal historical tags with setup bytes
and explicit metadata, without importing the runtime decoder/default.

For a new wire version or contract, add a named fixture, inventory entry and
tag-validation test, with source/limits or an explicit synthetic label. Retain
older cases/evidence. Explicitly decide support or rejection for each older
version: keep supported replay assertions and unsupported-version rejection
tests. Retention promises no runtime compatibility. Deliberately update tests
when support changes; never turn rejection tests into implicit support.

The default is 19. Rust's `setup_order_and_protocol_are_enforced`
tests protocol-11 rejection; Python's
`test_other_protocols_are_rejected_before_setup_changes_state` tests rejection of
11, 18 and 20. The inventory implies no additional decoder support.

## Checks

From the repository root (the inventory check uses only the standard library):

```sh
uv run --locked --only-group tools python -m unittest discover -s tools -p 'test_fixture_versions.py'
uv run --locked --only-group tools python -m unittest discover -s tools/fidelity -p 'test_slither_protocol.py'
```

The tools group's `test_*.py` check includes the first. For runtime decoding or
client contracts, also run relevant Rust tests (compiling the networking client):

```sh
cargo test --locked --features networking --test client_protocol --test client_hud --test client_contract --test client_challenge
```

## Selecting and updating cases

Keep bounded selected cases in `tests/fixtures/`; full recordings/study bundles
stay outside Git. Add cases for new scenarios, preserve valid regressions and
avoid wholesale reformatting. Record source study/window; distinguish raw
packets from decoder- or model-derived expectations and label observed/synthetic
inputs. Synthetic cases test specified contracts, not empirical server claims.

Independently justify expectations with recorded bytes, a specified contract,
checked calculation or supporting evidence. Regeneration from changed code alone
cannot approve a regression. Document independent corrections and preserve
original evidence. Deprecation needs a supported-contract or invalid-evidence
reason, with replacement or retained rejection tests where applicable. A decoder
cross-check is not independent proof of its model.

Tie measured calibration gates to their recorded corpus. Test configurable
mechanisms with deliberately synthetic parameters to keep new supported defaults
from changing unrelated lifecycle tests. Protocol bytes,
rejection cases, invariants and frozen-corpus selection checks stay explicit.
Conflicting observations need a new corpus and explained regime/supersession;
replacing old data to pass the current model hides regressions.

For new regressions, retain a small raw packet/window with archive SHA-256,
record indices, wire version, decoder identity and selection rule. Label fields
derived by the existing decoder/replay exporter; consume the fixture in a focused
test. The [worked study](https://github.com/NikitaMGrimm/slither-sim/blob/main/tools/fidelity/worked_study.py)
demonstrates this path with the full audited study outside Git.

The standalone replay exporter hashes/decodes one temporary archive snapshot.
Schema-1 `provenance` retains archive, selected window/connection, wire protocol
and decoder/exporter source digests without changing replay values; older
fixtures keep original provenance. Export checks gzip completion/window coverage,
not full capture/study integrity, and labels that limit. Audit the study
separately before treating derivatives as evidence.
