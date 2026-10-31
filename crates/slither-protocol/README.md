# slither-protocol

Borrowed, allocation-free Slither framing and protocol-19 wire views, bounded
challenge algorithms, and fixed-array/caller-buffer client-message builders.
MIT, Rust 1.99, edition 2024, `no_std`, no dependencies or features. Works on
`wasm32-unknown-unknown`; contains no clocks, threads, I/O, sockets or bindings.

```rust
use slither_protocol::{frame::*, v19, DecodeAdmission};
let frame = decode_frame(b"p", Header::Absent, Limits::CLIENT, FrameAdmission::Bounded)?;
assert_eq!(frame.command_count(), 1);
for command in frame.commands() {
    assert_eq!(command.offset, 0);
    let message = v19::decode_command(command, DecodeAdmission::ClientReceived)?;
    assert!(matches!(message, v19::Message::Pong(_)));
}
# Ok::<(), slither_protocol::DecodeError>(())
```

`decode_frame` validates the complete envelope before exposing any commands.
`commands()` rescans immutable lengths with constant storage; every pass returns
the same command count, index and original message offset (including prefix).
The two-byte prefix is raw: a consumer decides whether it represents elapsed
time or sequence. Message/receipt IDs, atomic state application and setup
notifications belong to the consumer.

Use `Limits::CLIENT` (exactly 1 MiB / 4096 commands) and `FrameAdmission::Bounded`
for client ingress (command IDs >=32 plus setup boundaries). Historical sim
splitters use `Historical` with explicit limits, including `UNBOUNDED` for direct
callers. Both modes honor the supplied limits. Bare commands begin with >=32;
the first short-looking high byte never denotes a bundle length.

`v19::decode` parses one bare command; `decode_command` additionally translates
errors to message-relative offsets. `DecodeAdmission::{Sim, ClientReceived,
ClientPresentation}` preserve observed length/scalar/direction differences.
Generic framing keeps other bodies opaque; consumers must continue to select
which gameplay, session, HUD and minimap lanes they enable. Parsing a view does
not authorize a phase transition or change the consumer's unsupported count.
Errors are small `Copy` kind/opcode/offset records, without formatted strings.
Challenge error offsets refer to encoded payload bytes (decoded syntax offsets
are multiplied by two), excluding the `6` opcode.

Raw integers, optional fields, names/custom bytes, direction/color/eater
metadata and opaque setup extensions are retained. Body, food, HUD, team-score
and minimap traversals are concrete iterators. `BodyDelta::FinalDirection` is
distinct from half-unit offsets. Cache omissions are explicit; no missing
sector, eater, anchor or direction is invented. `scalar` retains the client's
exact operation order, including distinct prey and snake angle expressions.
Consumers retain centering, geometry, trig, score tables and state mutation.

Minimap headers expose advertised/capped dimensions and layer count.
`selected(dimension)` prevalidates reachable runs before yielding selected cells
and reports visited layers for work accounting. V requires the retained
dimension from the caller; header parsing alone does not validate those runs.
Zero dimensions, short grids and unreachable tails preserve client admission;
sim admission rejects zero U/L/M and oversized M. Bitmap retention, V baselines
and aggregate cell budgets remain caller policy.

`challenge::stock` explicitly selects `Grammar::TokenWhitespace` (client) or
`ExactTemplate` (sim solver and framing probe). `literal_27`, `vlither_27` and
`ntl_legacy_24` are separate simulator compatibility entrypoints, never fallback
selection. Legacy admits only the simulator's fixed authored source, not a
general JavaScript grammar. See the retained
[browser-admission evidence](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-27-browser-admission/REPORT.md).

`v19::client::write_login` has no profile defaults: version, 20-byte page echo,
nickname bytes, skin and custom tail are required caller inputs. Nickname
ASCII/Latin-1 policy stays outside; the builder only bounds lengths. Champion
normalization/truncation stays outside `write_champion`. All variable builders
validate input and capacity before writing. Input quantization/cadence stays
outside, except explicitly named `sim_heading` preserving simulator arithmetic.
Negotiation/ping constants supply bytes, without choosing when to send them.

The local `~` extension exposes raw fields/bits; its physics-owned supported
mass-floor score range must still be checked by the simulator adapter.
Protocol 13 is not implemented or silently aliased to 19.

## Validation and adoption

Run `cargo +1.99.0 test --locked -p slither-protocol`, strict package Clippy, fmt
and a wasm32 check. Synthetic tests pin consumer admission differences,
independent outgoing/challenge bytes, f64 bits and zero allocations for complete
bare/bundled frames, messages, HUD rows, food/body iterators and invalid inputs.
The allocator test first verifies its counter detects allocations.

`tools/protocol_client_oracle.py` in the Git checkout feeds the same synthetic
inputs through the actual client and shared scalar functions, compares
`to_bits()` and prints fixed expectations. `tests/numerics.rs` records the
client pin and generation method; do not regenerate its bits to accept drift.
The oracle also checks the eleven retained historical stock challenges without
copying their captures into this package. Only authored synthetic data ships here.

A0 introduces this independent package; neither consumer uses it yet. B adopts
it from the client through a path dependency, preserving the direct-login pin
at merged #391 (`8b36a91c`). A switches the simulator only after owner-approved
publication. No performance, consumer-switch or visual acceptance is claimed.
The owner will manually publish 0.1.0 after A0 merges; A0 only verifies a locked
publish dry-run and adds no release automation. See the
[release prerequisites](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/releasing.md#shared-protocol-bootstrap-owner-gated).
