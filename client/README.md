<p align="center"><img src="assets/logo.svg" alt="SlitherClient" width="480"></p>

Independent Rust client for Slither-compatible servers, aiming for the look and
feel of the original game. Alpha, unreleased. Windows desktop and browser come
first; mobile comes later.

The shared wgpu client is in [apps/wgpu-slice](apps/wgpu-slice/README.md).
The Canvas2D browser client remains the visual reference. Both use the Rust core.
The server is the slither-sim crate at the repository root.
See [perceptual parity](docs/perceptual-parity.md) for acceptance policy and visual drift budgets.

## Setup

Install Rust **1.99.0**, Node.js **24**, platform C/C++ build tools, and
**wasm-bindgen CLI 0.2.129**. The pinned Rust toolchain includes the Wasm target.
Put wasm-bindgen on PATH or set WASM_BINDGEN to its absolute executable path.
Run client commands from client/.

```sh
npm ci --ignore-scripts --no-audit --no-fund
node tools/check-repository.mjs
```

Stage intended changes and run the guard before committing. Use `--pre-push` with
Git's push input for outgoing commits, or `--history` for a full audit. Do not set
`core.hooksPath` in a shared multi-worktree checkout; `tools/hooks` remains
available for single-clone opt-in.

Install a current challenge-capable slither-sim binary separately. Put it on PATH
or set SLITHER_SIM to its executable path. To supply a path directly, use
`npm run dev:desktop -- --simulator <path>` or
`npm run dev:web -- --simulator <path>`.
Both commands build first and start their own loopback simulator on a free port.
Press Ctrl+C to stop either command and its simulator. Closing the desktop
window also stops its simulator; closing the browser tab leaves dev:web running.

## Run

Windows desktop (wgpu):

```sh
npm run dev:desktop
```

Press Enter to Play; use pointer/arrows and Space or mouse boost.
During play, Escape opens a small menu with **Leave game**. Escape again or click
outside to dismiss it. The game keeps running: steering holds its last heading
and boost is released until the menu closes. Leave game returns to login after
the peer closes or the five-second close deadline. Enter during that wait queues
another Play. Escape still cancels a pending connection.
During play, the mouse wheel [zooms](docs/zoom.md) from 1.25 times the stock
scale out to the whole map. Zoom lasts until the app closes.

Official servers (Windows desktop):

```sh
cargo run --release --manifest-path apps/wgpu-slice/Cargo.toml --locked --bin slither-wgpu-desktop -- --nickname Player --skin 0
```

slither-client is an independent project, not affiliated with or endorsed by
slither.io or Lowtech Studios. Native startup begins official-site discovery and
server probing in the background. Play uses the results or waits for the in-flight
acquisition. Use `--server host:port` to skip that contact. Check slither.io's terms
before connecting.

Use `--server host:port` to connect directly to a custom server. The client builds
`ws://host:port/slither` internally; IPv6 uses `[address]:port`. Direct launches
never fetch the official website or discover servers. With no server flag, the
native client discovers official servers and measures latency in the background
at startup. Play with Automatic (best ping) joins that work and connects using
fresh login values. The former official opt-in flag has been removed.
Official play is interactive only: headless runs require `--server`.

Direct `--server` sends a fixed official-format login (the same values as the
simulator's portable_bot), for compatibility with custom server implementations
that copied the official login.
The server's Setup selects protocol 13 or 19 for the connection; both use the
same client core and stock login.

The window opens on login, restoring saved appearance preferences. Explicit
`--nickname` and `--skin` override restored values for that run. Change Skin opens
the offline [skin studio](docs/skin-studio.md); Save commits the draft and Cancel
discards it. Click Play or press Enter to acquire the profile/server and connect
once. The native login always uses the authored green/purple SlitherClient stroke
wordmark. `--cosmetics-dir` still supplies backgrounds, skins, accessories and menu
icons. [Settings](docs/settings.md) on the login selects and remembers Quality
for the next Play.

Open [Servers](docs/server-picker.md) to see the official directory or add a
`host:port` and choose its Custom row to Play directly. Choosing Custom cancels
unfinished official discovery. Custom entries and choices save locally; a new
no-flag launch starts in official context, preserving a saved official ID and
using Automatic for a saved custom choice. Choose Automatic or an official row
to return to official play. The picker needs no directory-loading button.


Browser (Canvas2D):

```sh
npm run dev:web
```

Open the printed loopback URL, choose a nickname and click Play.
This builds the Wasm client and serves it with the local endpoint filled in.
See the [wgpu app README](apps/wgpu-slice/README.md) for the WebGPU browser build.

To run the simulator separately, select unused ports:

```sh
slither-sim host --bind 127.0.0.1:19090
npm run build:web
npm run scene -- --port 19091 --simulator-port 19090
```

Open http://127.0.0.1:19091/. The scene command remains available for existing
Canvas workflows; diagnostic and extension routes have been removed.

## Test

```sh
npm run check
```

Runs repository guards, JavaScript tests, core Rust format/tests/Clippy, both
Wasm builds and the real Canvas Wasm ABI tests. Native wgpu tests, Clippy and the
desktop build run on Windows; Linux skips those native wgpu steps. The command
opens no window and connects to no game server. [Testing](docs/verification.md)
lists focused checks and prerequisites.

Read [architecture](docs/architecture.md) and [contributing](CONTRIBUTING.md) for
code ownership and development rules. Keep official source, assets, credentials
and captures out of Git; the private reference JS is a local specification to
port idiomatically. See [source and license](docs/provenance.md).
