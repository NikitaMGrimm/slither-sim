# slither-protocol

Reads and writes the slither.io wire protocol (version 19) without allocating.

- `frame` splits a WebSocket message into commands and checks the whole
  envelope before it returns any of them.
- `v19` gives borrowed, typed views of server messages: snakes, food, prey, HUD
  rows, the minimap, and the extra messages slither-sim servers send.
- `scalar` turns wire integers into positions, angles and speeds with the same
  floating-point steps as the desktop client.
- `challenge` answers the login challenge.
- `v19::client` builds client messages (login, input, ping) into a fixed array
  or a buffer you provide.

The crate is `no_std`, has no dependencies or features, needs Rust 1.99 and
builds for `wasm32-unknown-unknown`. It has no clocks, threads, I/O or sockets:
your code owns the connection and the game state.

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

## Reading messages

`commands()` can be called any number of times. Every pass yields the same
commands, indices and offsets. The two-byte prefix is returned raw, because a
server may use it for elapsed time or for a sequence number.

Choose limits and admission for your role:

- A client uses `Limits::CLIENT` (1 MiB, 4,096 commands) with
  `FrameAdmission::Bounded`, which requires command IDs of 32 or more outside
  setup.
- Simulator tooling uses `FrameAdmission::Historical` with explicit limits, or
  `Limits::UNBOUNDED` for direct callers.

`v19::decode` parses one bare command. `decode_command` also reports errors
relative to the whole message. Before this crate existed, the simulator and the
client each accepted slightly different lengths, values and directions;
`DecodeAdmission::{Sim, ClientReceived, ClientPresentation}` keeps each of those
behaviors instead of picking one. Errors are small `Copy` values holding a
kind, an opcode and an offset.

Views keep every transmitted field, raw integers included, and never fill in a
missing value. Bodies, food, HUD rows, team scores and minimap cells come
through iterators. Applying them to your game state, geometry and trigonometry
is up to you.

A minimap header reports the advertised size, the capped size and the layer
count. `selected(dimension)` checks every run it can reach before it yields any
cells. For a `V` (delta) map, pass the size you kept from the last full map.

`extensions::slither_sim` returns the raw fields of the simulator's rules
message. The simulator still checks its score range itself.

## Matching on results

`Message`, `ErrorKind`, `Target`, `Direction`, `BodyDelta`, `Eater` and
`MinimapFormat` are `#[non_exhaustive]`: later versions may add variants, so a
`match` needs a wildcard arm. That is separate from `Message::Unknown`, which
stands for an unknown opcode on the wire. The enums you pass in (`Header`,
`FrameAdmission`, `DecodeAdmission` and `Grammar`) are closed.

## Login challenge

`challenge::stock` solves the standard challenge. Use
`Grammar::TokenWhitespace` to parse it the way the desktop client does, or
`Grammar::ExactTemplate` to check it the way slither-sim's server does.
`literal_27`, `vlither_27` and `ntl_legacy_24` compute the different replies
that some other clients send, so a server can accept them too. The crate never
falls back to them on its own. Challenge error offsets count bytes of the
encoded payload after the `6` opcode. See the
[browser admission study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-27-browser-admission/REPORT.md)
for where these replies come from.

## Client messages

`v19::client::write_login` has no defaults. You pass the version, the 20-byte
page value, the nickname bytes, the skin and the custom tail. It checks lengths,
not character sets. The other builders also check their input and the buffer
size before writing anything, and leave quantization and send timing to you.
`sim_heading` reproduces slither-sim's own heading arithmetic.

Protocol 13 is not implemented yet.

## Testing

Run `cargo test -p slither-protocol`. The tests cover each consumer's admission
rules, the exact bytes of client messages and challenge replies, the bits of
every numeric conversion, and zero allocations while reading frames, messages,
HUD rows and iterators.

The numeric expectations come from the desktop client itself.
`tools/protocol_client_oracle.py` in the repository feeds the same inputs
through the client and this crate and compares `f64` bits. Don't regenerate the
expectations to accept a change. Once the client uses this crate, run the
oracle with `--client-root` on a client checkout at
`8b36a91cafe6878ccedf0d7da70a4008ae7ef108`, the last commit before the switch;
otherwise it compares the crate with itself.

## License

MIT
