# Regression fixtures

Start new cases or changed expectations with
[selecting and updating cases](#selecting-and-updating-cases).

[manifest.json](manifest.json) inventories wire/client protocol and client-frame
contract fixtures, with repository-relative paths. Generic mechanics fixtures
are excluded; never guess their protocol from the decoder default. Recorded
historical protocol-19 login challenges are inline in `tests/client_challenge.rs`,
not `login-challenges.json`; the new legacy-login selection is described below.

`schemaVersion` versions the inventory. Each entry pins
`wireProtocol` (the actual binary wire version; most cases are literal `19`) and
`clientContract` (`slither-client-v2` for the synthetic frame, otherwise `null`).
The frame ID differs from the `slither-client-json-v2` request/reply
envelope (also pinned with removed-field requests in `client-contract-v2.json`)
and the geometric `slither-sim-json-v5` controller; see
[interface versions](../../docs/client-api.md#interface-versions).

[`serve-errors-v5.json`](serve-errors-v5.json) pins the geometric controller's
`slither-sim-json-v5` error envelope and a synthetic overflow reproducer for
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

[`client-legacy-login.json`](client-legacy-login.json) retains a bounded official
`[99, 4]` / selector-12 login observation: the existing stock challenge format
was accepted with its 27-byte reply, then setup advertised **protocol 13**.
The inventory pins 13, with no client-frame contract. The setup is a selected
25-command-byte prefix excluding server identifiers; challenge/reply and tiny
gameplay witnesses have archive hashes and record indices. The transmitted
login is explicitly synthetic. Tests check the existing challenge solver and
protocol-13 rejection; they do not enable protocol 13 or interpret its gameplay.
See the [selection and stop decision](../../research/fidelity/2026-10-26-legacy-login/REPORT.md).

[`transport-official-reference.json`](transport-official-reference.json) is a
retained **home-route/collector** wire-19 measurement summary (`schemaVersion: 1`), outside the packet
fixture inventory. It contains input paths/hashes, parse exclusions and
decoder-derived transport quantiles, without raw gameplay bytes. See its
[selection and limits](../../research/fidelity/2026-10-25-transport-parity/REPORT.md).

[`transport-official-near-server.json`](transport-official-near-server.json) is
the default server transport reference, derived offline from all six completed
#111 batches **unfiltered**, measured identically to candidates. It pins protocol
19 and source/analysis hashes. The common spread allowance 1 is the smallest of
{0.5, 1, 2} passing all six LOO comparisons; frame-gap p50 alone uses a 1 ms
header-resolution spread floor. False-reject control is only that six-capture
check, with observed batch drift and no population guarantee. Masked development
and held-out summaries remain separate characterization; the original dev-only
envelope and held-out exceedance stay in the report as the first attempt. No raw
gameplay bytes or absolute one-way-delay claims are included.

The additive `viewStrata` fields pin **`transport-view-v1` / protocol 19** for
those same six unfiltered captures, replayed from complete setup prefixes.
`viewStrataProvenance` identifies the original fixture and measurement sources;
all original pooled/per-capture summaries, masked/heldout cases and their labels
remain unchanged. The new strata follow the
[sparse-view reconstruction](../../research/fidelity/2026-10-26-sparse-view-cadence/REPORT.md)
and [residual activity proxies](../../research/fidelity/2026-10-26-cadence-residual/REPORT.md).
Per-bin counts, exposure, <=9 ms fractions and complete-window counts reconcile
with the retained study outputs; 6–10-peer speed/boost/turn fractions also
reconcile with the residual replay. This justifies the additive expectations;
it supplies neither new empirical gate calibration nor independent server-model
validation. Underexposed bins stay in the fixture and remain inconclusive.

To reproduce with local access, run `transport_parity.py measure` on each pinned
archive with `--summary-only`, check the capture SHA-256 and exact original
`summary` equality, then append only that capture's `viewStrata`. Keep historical
tool/provenance fields intact and identify the new tool, view module, decoder and
census hashes separately in `viewStrataProvenance`. No raw bytes, endpoints or
local paths are needed in the additive fields. The focused transport tests pin
the unchanged original fixture digest and reconcile the committed study's
per-capture selections, so regeneration alone cannot approve changed values.

[`client-reporting-cadence-near-server.json`](client-reporting-cadence-near-server.json)
pins clean server-header prey/HUD gaps from all six completed batches. Client
simulation tests compare nominal prey/HUD periods with these medians, while
[`scheduler-near-server-witnesses.json`](scheduler-near-server-witnesses.json)
pins selected held-out 24 ms motion, 84 ms mass, motion-before-mass and 3044 ms
prey cases. The decoder/client tests consume their setup, appearance and logical
packet bytes, with synthetic reframing and explicit clock differences. These
witnesses are protocol 19 / `slither-client-v2`; cadence summaries have no frame
contract. Fixed simulator cadences are model choices, not universal bounds.
See the [audit, selection and reproduction](../../research/fidelity/2026-10-26-server-timing-references/REPORT.md).

The original `client-reporting-cadence.json` remains a home-capture regression
with its original labels. `death-jitter-fresh.json` tests spatial death-food
scatter and tail-first ordering, not network jitter; it remains unchanged.

## Changing a protocol or contract

`forager-conformance.json` contains full selected synthetic/local client frames
with binary64 bit encoding. Compact independent Windows x86_64 MSVC and Linux
x86_64 GNU outputs live in `forager-oracle-windows-x86_64-msvc.json` and
`forager-oracle-linux-x86_64-gnu.json`. Exact target tests compare every
Timing/Intent/action bit after verifying a SHA-256 of each shared canonical frame.
Each oracle case contains only its ID, input hash and byte-preserved expectations;
raw output hashes and projection instructions are in the port record. The
obsolete A1 expectations were removed when A4-mean became the sole policy; all
140 A4-mean expectations per platform, frame hashes and metadata are unchanged.
To verify this projection, delete only each case's `expected.A1` from the prior
JSON and compare the full parsed result with the current oracle.
Its historical contract is `slither-client-v2`, protocol 19. The
[port record](../../research/bots/2026-10-26-forager-port/REPORT.md) describes
selection, provenance and four retained last-bit platform differences. macOS
conformance is unverified because no macOS oracle exists; macOS and other targets
without an oracle run explicit determinism and structural invariant tests. `forager-native-parity.json` remains
the separate twelve-case historical default-policy regression.

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

`serve-errors-v5.json` is the current geometric error-envelope fixture and
pins structured cause codes/locations and lifecycle. `serve-radius-v5.json` is
a synthetic initial/live radius split fixture shared by Rust transport, C and
Python tests; it includes time-zero and next-tick samples.
`serve-rules-v5.json` is a synthetic customized rules fixture shared across Rust,
C and Python. `serve-errors-v3.json`, `serve-errors-v4.json` and
`serve-radius-v4.json` stay unchanged for the released historical contracts.

`arena-seat-following-v2.json` is a synthetic rules-v2 mechanics fixture, outside
the wire/client inventory. Its 500→100 shrink ticks, duration and final radius
come from the stated scalar formula, not from regenerated simulator output.
The measured `arena-radius-updates.json` fixture and historical study stay intact.
