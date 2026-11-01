# slither-protocol

Reads the slither.io wire protocol (versions 13 and 19) and writes version-19
client messages without allocating.

- `frame` splits a WebSocket message into commands and checks the whole
  envelope before it returns any of them.
- `v19` gives borrowed, typed views of server messages: snakes, food, prey, HUD
  rows, the minimap, and the extra messages slither-sim servers send.
- `v13` reads the catalogued legacy server messages, retaining explicit IDs,
  coordinate anchors, all appearance deltas and absolute food coordinates.
- `scalar` turns wire integers into positions, angles and speeds with the same
  floating-point steps as the desktop client.
- `challenge` answers the login challenge.
- `v19::client` builds client messages (login, input, ping) into a fixed array
  or a buffer you provide.

The crate is `no_std`, has no runtime dependencies or features, needs Rust 1.99 and
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

`v13::decode` and `v19::decode` parse one bare command. `decode_command` also reports errors
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

`Message`, `ErrorKind`, `Coordinates`, `Target`, `Direction`, `BodyDelta`, `Eater` and
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

`challenge::copy_loop` is a separate, opt-in solver for `var a=LIT;`, zero or
more `a+=LIT;` statements, then `for(var i=0;i<N;i++)idba[i]=a.charCodeAt(i);`.
The body may be braced, token whitespace is allowed, and each nonempty literal
may use either quote. Contents must be printable ASCII without its delimiter
or backslashes. N has at most three digits, no leading zero, and equals the
total literal length. The existing 256-byte decoded-source limit applies.
`CopyLoopReply::as_bytes()` exposes only the reply's initialized length from a
fixed array. The rotation is stock start-3/step-4 with JavaScript signed
remainder, over N bytes. This does not select the legacy reply algorithm or
broaden simulator admission. Tests use authored synthetic challenges.

## Client messages

`v19::client::write_login` has no defaults. You pass the version, the 20-byte
page value, the nickname bytes, the skin and the custom tail. It checks lengths,
not character sets. The other builders also check their input and the buffer
size before writing anything, and leave quantization and send timing to you.
`sim_heading` reproduces slither-sim's own heading arithmetic.

## Protocol 13 coverage

Use the same `frame` splitter and `scalar` conversions, then explicitly select
`v13::decode_command`. Neither consumer switches protocol automatically. These
are borrowed wire views; ownership of the local ID and relative-coordinate base
stays with the caller. In v13, `G` and `N` carry IDs for peers and self alike.

| Codes | Command shape (including opcode) |
| --- | --- |
| `a` | Shared setup prefix, version 13, complete optional fields and opaque extensions |
| `s` | Shared appearance header; optional first x24/y24 then **all** half-unit delta pairs; short removal |
| `e/E/3/4/5` | Explicit id16 and 1–3 heading8/wanted8/speed8 fields, selected by opcode and length |
| `g/G/N` | `g`: id16 + absolute x16/y16; `G`: id16 + relative x8/y8; `N`: relative plus fraction24 |
| `h/r` | id16 plus fraction24; `r` also supports id-only removal |
| `F` | Zero or more color8/x16/y16/size8 tuples, without a sector header |
| `f/b/c` | `f/b`: color8/x16/y16/size8; `c`: x16/y16 with optional eater-id16 |
| `w` | sector8/sector8 |
| `y/j` | Shared prey appearance/removal/motion layouts |
| `M/l/k/p/z/v/S/6` | Shared minimap, leaderboard, kill count, pong, radius, life control, session and opaque challenge |

The [official catalog](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-protocol13-catalog/REPORT.md)
observed all of those families but not every accepted shape. The retained
33-record fixture pins the observed layouts, not a complete wire specification.
Field scales, relative bytes minus 128, half-unit bytes minus 127, turn
directions and map meaning are protocol interpretations, not physics findings.
In motion packets, three fields mean heading/wanted/speed; two mean
heading/speed (`e`), wanted/speed (`E/4`) or heading/wanted (`3/5`); one means
heading (`e`), wanted (`E/4`) or speed (`3`). One-field `5` is rejected in v13.

Additional, unobserved support is explicitly derived: absolute growth `n`,
no-eater `c`, id-only `r` and the extra shared prey/motion lengths follow the
independent simulator encoder or protocol conventions. `V/U/u/L`, `R/m/~`
support local minimap and presentation output; the official catalog observed
only `M` among those maps. Unsupported `d/7/=/+/C/</o/W` and unknown opcodes
remain `Message::Unknown` with their exact bytes.

`DecodeAdmission` preserves the existing shared-layout differences: `Sim`
allows removal lengths 3–7, loose pong/life-control tails, zero sector count or
radius, and unchecked direction bytes; client admission requires removal 3/4,
exact pong/life-control lengths, positive setup sector count/radius and valid
prey direction. Only `ClientPresentation` checks appearance direction. The
same HUD limits and minimap admission rules apply as in v19. All admissions
require complete v13 coordinate tuples and explicit IDs. Version-19 consumers
continue rejecting version-13 setup until they explicitly adopt this reader.

## Testing

Run `cargo test -p slither-protocol`. The tests cover each consumer's admission
rules, the exact bytes of client messages and challenge replies, the bits of
every numeric conversion, and zero allocations while reading frames, messages,
HUD rows and iterators.

The crate's packet tests use authored synthetic inputs only. Catalog-backed
protocol-13 checks stay in the simulator's root integration test
`tests/protocol13_catalog.rs`, reading the historical fixture in place. Encoder
round trips live in `src/client/encode13/shared_reader_tests.rs`. Captured data
is not included in this crate's package.

The numeric expectations come from the desktop client itself.
`tools/protocol_client_oracle.py` in the repository feeds the same inputs
through the client and this crate and compares `f64` bits. Don't regenerate the
expectations to accept a change. Once the client uses this crate, run the
oracle with `--client-root` on a client checkout at
`8b36a91cafe6878ccedf0d7da70a4008ae7ef108`, the last commit before the switch;
otherwise it compares the crate with itself.

## License

MIT
