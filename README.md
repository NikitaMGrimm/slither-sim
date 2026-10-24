<p align="center">
  <img src="https://raw.githubusercontent.com/NikitaMGrimm/slither-sim/main/assets/logo.svg" alt="SlitherSim" width="420">
</p>

<p align="center">
  A headless Slither.io simulator in Rust for bot development, training and custom servers.
</p>

<p align="center">
  <a href="#quick-start">Quick start</a> |
  <a href="docs/client-api.md">Portable bots</a> |
  <a href="docs/raw-server.md">WebSocket server</a> |
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

<p align="center">
  <a href="https://github.com/NikitaMGrimm/slither-sim/actions/workflows/checks.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/NikitaMGrimm/slither-sim/checks.yml?branch=main&amp;style=flat-square&amp;label=CI"></a>
  <a href="https://github.com/NikitaMGrimm/slither-sim/releases"><img alt="Release" src="https://img.shields.io/github/v/release/NikitaMGrimm/slither-sim?style=flat-square"></a>
  <a href="https://crates.io/crates/slither-sim"><img alt="crates.io" src="https://img.shields.io/crates/v/slither-sim?style=flat-square"></a>
  <a href="https://github.com/NikitaMGrimm/slither-sim/blob/main/Cargo.toml"><img alt="Rust 1.86+" src="https://img.shields.io/badge/rust-1.86%2B-orange?style=flat-square"></a>
  <a href="https://github.com/NikitaMGrimm/slither-sim/blob/main/LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue?style=flat-square"></a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/NikitaMGrimm/slither-sim/main/assets/banner.gif" alt="Stylized snake animation with the cargo run --locked -- serve command" width="860">
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

### Download a release

Without Rust, download an archive from a [release](https://github.com/NikitaMGrimm/slither-sim/releases).
Each holds the `slither-sim` CLI and the [C ABI](docs/c-api.md) library with its
header, for Linux x86-64 and ARM64 (glibc 2.17+), macOS (Apple silicon and
Intel) and Windows x86-64. `SHA256SUMS` lists their checksums.

```sh
curl -LO https://github.com/NikitaMGrimm/slither-sim/releases/latest/download/slither-sim-x86_64-unknown-linux-gnu.tar.gz
tar -xzf slither-sim-x86_64-unknown-linux-gnu.tar.gz
echo '{"op":"reset","agents":1}' | slither-sim-x86_64-unknown-linux-gnu/slither-sim serve
```

The macOS and Windows builds are unsigned. On macOS, a browser download is
quarantined; clear it with `xattr -dr com.apple.quarantine` on the extracted
folder. On Windows, unblock the `.zip` before extracting (Properties → Unblock,
or `Unblock-File` in PowerShell), or choose More info → Run anyway if
SmartScreen stops the CLI.

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
| Run `serve` sessions in-process from Python | [Python package](docs/python.md). |
| Run `serve` sessions in-process from any language | [C ABI](docs/c-api.md), or Rust `transport::Session`. |

The portable backends share a Rust packet decoder; bots in other languages use
JSON, or the C ABI for `serve` sessions. The geometric `serve` API provides direct observations without network delay.
See [interface versions](docs/client-api.md#interface-versions) for compatibility.

## Defaults and fidelity

The defaults combine measured movement and protocol constants with approximate
visibility, spawning and ecology; see [rules and evidence limits](docs/server-rules.md).

The default world is `Config::official(1)`: the best current approximation of an
official server, initialized for one snake, with dynamic radius, scaled food and
one prey. It includes provisional models and is not an exact official replica.
`Config::sandbox()` keeps the same mechanics in a compact fixed arena. See
[official and sandbox](docs/configuration.md#official-and-sandbox) for both and
[rules and evidence limits](docs/server-rules.md) for what has been measured.

## Development

- [Contributing](CONTRIBUTING.md): changes, tests and packaging.
- [Documentation map](docs/README.md): API guides, research tasks and maintenance.
- [Research workflow](docs/research-workflow.md): capture, audit and replay tools.
  These separately require uv, Python 3.12 and Node.js 22+ (24 recommended).
- [Performance](docs/performance.md): reproducible release workloads.
- [Research records](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md): dated results and evidence limits.

Report bugs in [Issues](https://github.com/NikitaMGrimm/slither-sim/issues), with
the command, configuration and expected behavior. Add the Rust library with
`cargo add slither-sim`, or install the CLI with `cargo install slither-sim --locked`.

[MIT](LICENSE). Independent of Slither.io.
