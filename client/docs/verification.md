# Testing

Use Rust 1.99.0, Node.js 24, platform build tools and wasm-bindgen CLI 0.2.129.
The toolchain file installs wasm32-unknown-unknown. Set WASM_BINDGEN to an absolute
CLI path if it is not on PATH.

```sh
npm ci --ignore-scripts --no-audit --no-fund
npm run check
```

The check command mirrors .github/workflows/checks.yml on the current host:

- Git index and reachable-history source guards.
- JavaScript behavior, build/server tooling and wgpu socket-adapter tests.
- Core rustfmt, all-target tests, doctests and strict Clippy.
- Canvas release Wasm build and real ABI tests using synthetic packets and sources.
- wgpu rustfmt, library/integration tests, strict native/Wasm Clippy, desktop and
  Wasm builds, and generated WebGPU bindings.

It opens no window or GPU and connects to no game server. Dependency installation
can access package registries. CI runs native checks on Linux and Windows, wgpu
native checks on Windows, and Wasm checks on Linux. A local run covers its host;
it does not replace the other OS job.

## Focused checks

```sh
npm test
cargo test --locked --test protocol
cargo fmt -p slither-client -p slither-wgpu-slice --check
cargo test --all-targets --locked
cargo test --doc --locked
cargo clippy --all-targets --locked -- -D warnings
npm run build:web
node tools/wasm-smoke.mjs
cargo test --manifest-path apps/wgpu-slice/Cargo.toml --locked --lib --tests
```

The app `custom_servers` integration test uses synthetic documentation addresses,
injected official acquisition callbacks and isolated temporary preference files.
It covers direct selection without fetching, input/focus and malformed storage.
The `menu_specimens --picker-panel` mode captures only the shared modal, without
art, a game session, a window or network work.

The ABI smoke helpers are shared regression coverage, not standalone network
probes. Tests use explicit authored doubles or real generated Wasm as labelled.
Run the ABI suite again after changing Rust/browser boundaries. For wgpu, the
full check command uses one build job, no debug info and target/wgpu-slice-check
to limit memory and keep its artifacts separate from Canvas. Generated bindings stay in dist/.

The pure Node [feel replay contract](../tools/feel-replay/README.md) defines
offline schedules, lightweight manifests and perceptual trace comparison.
See [perceptual parity](perceptual-parity.md) for channel budgets and acceptance. Run
`node --test tools/feel-replay.test.mjs tools/feel-browser-replay.test.mjs` for
authored synthetic fault checks. The [browser peer command](../tools/feel-replay/browser-peers.md)
uses prebuilt, feature-gated Wasm in background headless Chrome and a CPU
GPU-boundary sink. It is a separate local run, not part of npm test or proof of
pixel, physical-input or perceptual parity.
The [native CPU peer](../tools/feel-replay/native-peer.md) adds feature-gated
LocalGame driver tests and the same native/Wasm perceptual comparator; it opens no window and does not measure GPU or desktop pacing.

The wgpu [draw-chunk contract](draw-chunks.md) documents the draw-memory bound,
recoverable picture skips and native/browser correctness evidence limits.

## Offline host/client end-to-end run

Build from the repository root with `CARGO_BUILD_JOBS=2` (and
`CARGO_PROFILE_DEV_DEBUG=0` for the client probe); run these builds sequentially:

```sh
cargo build --release --locked
cargo build --manifest-path client/apps/wgpu-slice/Cargo.toml --locked --bin slither-client-e2e --target-dir client/target/wgpu-slice-check
cd client
node tools/e2e-local-host.mjs --host-bin ../target/release/slither-sim
```

On Windows append `.exe` to the host path. `--client-bin PATH` overrides the
prebuilt debug probe path; `--seconds N` selects 5–60 seconds of gameplay
(default 8). The runner does not build binaries. Its pure argument, port,
readiness and assertion tests run in `npm test` without either binary.
The probe takes `--server 127.0.0.1:<port> --seconds 5..60`; it builds the
WebSocket URL internally and retains its loopback-only admission. The ordinary
desktop headless path also needs `--server`, and rejects `--official`.

The runner writes a temporary host config: one player, no bots/food/prey,
fixed radius, a 400-point starting floor so boost is available, and leaderboard
and minimap periods of 0.5 seconds. It starts the host with
`--bind 127.0.0.1:0`, reads the **complete** `Listening on ws://…/slither`
announcement from stderr (or stdout), then connects only to that literal
loopback address. The host keeps its ephemeral listener; no port reservation
is released and rebound. Ports 8766, 18884 and 18885 are rejected. Current
`slither-sim host` already reports its actual port-0 bind: no host change is
requested. Readiness and admission each allow 20 seconds; close allows 5 seconds;
the outer client deadline is gameplay duration plus 30 seconds. Both direct
child processes are reaped and temporary config removed on success, failure,
SIGINT and SIGTERM; termination escalates after two seconds if necessary.

`slither-client-e2e` is a GPU-free native host of production `native::Transport`
and `game::LocalGame`. It runs the actual challenge/login, ingress, input,
egress and presentation preparation, then accepts each prepared picture at a
CPU sink through `presented(true)`. This is the existing transaction seam also
used by the [native feel peer proposal](https://github.com/NikitaMGrimm/slither-sim/pull/305);
it does not depend on that unmerged branch. No window, GPU adapter/device,
software Vulkan or submission is created. The ordinary desktop `--headless`
mode still uses offscreen GPU rendering and PNG capture. Linux builds of this
probe omit the winit dependency and use installed DejaVu Sans for
real text preparation (`fonts-dejavu-core` on Ubuntu). This is a protocol and
CPU presentation guard; it establishes no pixel, GPU or perceptual parity.

The single JSON result checks WebSocket open and challenge, login acceptance
through decoded Setup advertising protocol 19, own appearance, sent steering
and boost-on/off, changed **received** head coordinates and heading, and received
speed increasing after boost-on then decreasing after boost-off. It does not
substitute predicted presentation motion for server updates. Leaderboard and
minimap packets must pass production ingress in this reporting-enabled config;
at least one picture must be prepared. Finally the native transport sends Close
and polls for the peer's acknowledgement before exit code 0. Failures identify
the stage and include the partial client summary where available.

Only protocol 19 is covered. The host also supports 13, but this client's
`SessionPolicy::admit_protocol` and state decoder currently reject it. This
test does not add legacy support or relabel protocol 19 evidence.

### CI

CI runs this as the `e2e` job in `.github/workflows/checks.yml`, inside
`Required checks`; the simulator lane owns that workflow. It builds the release
host and debug `slither-client-e2e` with target directory
`client/target/wgpu-slice-e2e`, and runs on Ubuntu with DejaVu fonts.

## Source guard

```sh
git add <intended-files>
node tools/check-repository.mjs
node tools/check-repository.mjs --history
```

Run the staged guard before committing; `--history` audits history reachable from
branches, remote-tracking branches and tags.
Use `--pre-push` with Git's push input to check outgoing commits plus the index,
trusting only history the destination reported in this push. Only locally
available advertised commits are excluded; if none are available, scan the tips'
full history.
No network is used.
Do not set `core.hooksPath` in a shared multi-worktree checkout; `tools/hooks`
remains available for single-clone opt-in.

The guard reads Git blobs, so inspect unstaged changes separately. It blocks
known official bundles, raw captures, credentials, prohibited binary/generated
formats and unsafe SVGs. Content rules cover the whole repository; format, size,
encoding and SVG rules cover only client/. It complements review of authorship; it cannot identify
every copied program. Never bypass hooks to push an artifact the guard rejects.

## Manual checks

Use npm run dev:desktop or npm run dev:web with a separately installed slither-sim
binary. Both choose a free loopback port and clean up their own simulator. The
browser command prints the Canvas URL. Existing npm run scene accepts --port and
--simulator-port for a separately managed server; see the README.

Use offscreen rendering or headless browsers for automated visual work. Keep
captures outside Git. State what ran and any limitations: synthetic correctness,
local gameplay, browser rendering and perceptual parity are different results.
No tests or CI may connect to official game servers.
