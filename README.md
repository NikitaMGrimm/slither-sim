<p align="center">
  <img src="assets/logo.svg" alt="SlitherSim" width="420">
</p>

<p align="center">
  A headless Slither.io simulator in Rust for bot development, training and custom servers.
</p>

<p align="center">
  <a href="#quick-start">Quick start</a> |
  <a href="docs/client-api.md">Bot API</a> |
  <a href="docs/raw-server.md">WebSocket server</a> |
  <a href="docs/server-rules.md">Rules and limits</a>
</p>

## Quick start

Install [Rust](https://www.rust-lang.org/tools/install) 1.86 or newer and its
platform build tools, then run a food-seeking bot:

```sh
git clone https://github.com/NikitaMGrimm/slither-sim.git
cd slither-sim
cargo build --release --locked
cargo run --release --locked --example portable_bot -- --steps 200
```

The bot runs locally, prints a summary and exits. No game window opens.

For the same bot in Python:

```sh
python examples/portable_bot.py --steps 200
```

Run from the checkout after building. Python 3.11+ is optional; the example uses
only its standard library. On Windows, use `py -3` if needed. Rust users can
embed the simulator directly; see [the basic example](examples/basic.rs).

## Try the WebSocket server

Start a shared world in one terminal:

```sh
cargo run --release --locked -- host
```

From the same checkout in another terminal, connect the bot:

```sh
cargo run --release --locked --example portable_bot -- --url ws://127.0.0.1:8766/slither --steps 200
```

The host binds loopback by default. Stop it with Ctrl+C.
[Server configuration](docs/raw-server.md) covers custom worlds and remote hosting.
[All settings](docs/configuration.md) lists defaults, limits and timing behavior.

## Choose an interface

| Goal | Interface |
| --- | --- |
| Run a bot in simulation or on protocol-19 servers | [`client` JSON API](docs/client-api.md), or Rust `BotRunner`. |
| Connect WebSocket clients to a shared world | [`host`](docs/raw-server.md). |
| Control all slots with direct simulation observations | [`serve` JSON API](docs/bot-api.md), or Rust `Simulator`. |

The portable backends share a Rust packet decoder; bots in other languages use
JSON. The geometric `serve` API provides direct observations without network delay.
See [interface versions](docs/client-api.md#interface-versions) for compatibility.

## Development

- [Contributing](CONTRIBUTING.md): changes, tests and packaging.
- [Research workflow](docs/research-workflow.md): capture, audit and replay tools.
  These separately require uv, Python 3.12 and Node.js 22+ (24 recommended).
- [Provenance](docs/provenance.md): source hashes and adaptations.
- [Verification](docs/verification.md): completed checks and coverage limits.

Report bugs in [Issues](https://github.com/NikitaMGrimm/slither-sim/issues), with
the command, configuration and expected behavior. Build from source; no crate or
release binaries are published yet.

[MIT](LICENSE). Independent of Slither.io.
