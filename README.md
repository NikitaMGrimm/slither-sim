<p align="center">
  <img src="https://raw.githubusercontent.com/NikitaMGrimm/slither-sim/main/assets/logo.svg" alt="SlitherSim" width="420">
</p>

<p align="center">
  A headless Slither.io simulator in Rust for custom servers and bot development.
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
  <a href="https://github.com/NikitaMGrimm/slither-sim/blob/main/Cargo.toml"><img alt="Rust 1.99+" src="https://img.shields.io/badge/rust-1.99%2B-orange?style=flat-square"></a>
  <a href="https://github.com/NikitaMGrimm/slither-sim/blob/main/LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue?style=flat-square"></a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/NikitaMGrimm/slither-sim/main/assets/banner.gif" alt="Stylized snake animation with the cargo run --locked -- serve command" width="860">
</p>

## Quick start

Install [Rust](https://www.rust-lang.org/tools/install) 1.99 or newer and its
platform build tools, then run a food-seeking bot:

```sh
git clone https://github.com/NikitaMGrimm/slither-sim.git
cd slither-sim
cargo build --release --locked
cargo run --release --locked --example portable_bot -- --steps 200
```

The bot runs locally, prints a summary and exits. No game window opens.

For a simple portable controller in Python:

```sh
python examples/portable_bot.py --steps 200
```

It needs Python 3.11+ and the build above, and uses only the standard library.
On Windows, use `py -3` if `python` is missing.

More examples: [embedding the simulator](examples/basic.rs) and
[the Forager on core observations](examples/forager.rs). To use the
library elsewhere, run `cargo add slither-sim`; for the CLI alone,
`cargo install slither-sim --locked`.

Rust bots can reuse the tuned [Forager policy](docs/bot-api.md#frozen-forager)
through `client::ForagerBot::new()` or `default()`. A4 (A4-mean) is the only
optimized policy. Offline builds get `bot::Forager::default()` with
`--no-default-features --features bot`.

### Download a release

[Release archives](https://github.com/NikitaMGrimm/slither-sim/releases) hold the
`slither-sim` CLI and the [C ABI](docs/c-api.md) library and header, so Rust is
not needed. Builds cover Linux x86-64 and ARM64 (glibc 2.17+), macOS (Apple silicon
and Intel) and Windows x86-64. Check downloads against `SHA256SUMS`.

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

`client` bots decode the same packets in simulation as on a real server.
`serve` observations come straight from the world, with no network delay. See
[interface versions](docs/client-api.md#interface-versions) for compatibility.

## Defaults and fidelity

The default world, [`Config::official(1)`](docs/configuration.md#official-and-sandbox),
is sized for the population at startup and then holds its radius; official
radius waves are not modeled. It combines measured movement and protocol
constants with provisional visibility, spawning and ecology models; it is not an
exact official replica.
[`Config::sandbox()`](docs/configuration.md#official-and-sandbox) uses the same
mechanics in a compact fixed arena.

[`Config::battledome()`](docs/configuration.md#battledome) is our own mode, not an
official one: a one-way pit, a permanent 400-point size floor with free boosting
at it, and a rolling kill score. See
[rules and evidence limits](docs/server-rules.md) for measured behavior and gaps.

## Development

- [Contributing](CONTRIBUTING.md): changes, tests and packaging.
- [Documentation map](docs/README.md): API guides, research tasks and maintenance.
- [Research workflow](docs/research-workflow.md): capture, audit and replay tools
  (uv, Python 3.12, Node.js 22+).
- [Performance](docs/performance.md): reproducible release workloads.
- [Research records](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md): dated results and evidence limits.

Report bugs in [Issues](https://github.com/NikitaMGrimm/slither-sim/issues) with
the command, configuration and expected behavior.

[MIT](LICENSE). Independent of Slither.io.
