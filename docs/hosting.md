# Hosting on Linux

Run a public SlitherSim server from a release build as a systemd service. Release
archives cover x86-64 and ARM64 Linux with glibc 2.17 or newer; no Rust toolchain
is needed.

## Install

Pick a version from [releases](https://github.com/NikitaMGrimm/slither-sim/releases)
and your platform (`x86_64-unknown-linux-gnu` or `aarch64-unknown-linux-gnu`):

```sh
VERSION=v0.6.2
TARGET=x86_64-unknown-linux-gnu
BASE=https://github.com/NikitaMGrimm/slither-sim/releases/download/$VERSION
curl -fLO "$BASE/slither-sim-$TARGET.tar.gz"
curl -fLO "$BASE/SHA256SUMS"
sha256sum --ignore-missing -c SHA256SUMS
tar -xzf "slither-sim-$TARGET.tar.gz"
sudo install -m 0755 "slither-sim-$TARGET/slither-sim" /usr/local/bin/slither-sim
slither-sim --version
```

### Build for the hosting CPU

Release archives run on any x86-64 CPU, so they can't use newer instructions
such as AVX2. If the server has a Rust toolchain, build a binary for its own
CPU from a checkout of the version you want:

```sh
RUSTFLAGS="-C target-cpu=native" cargo build --release --locked --bin slither-sim
```

Install `target/release/slither-sim` as above. It runs only on CPUs with the
same features, so rebuild after moving to other hardware. In our tests its
game output matched the portable build exactly, and its median cycle count was
3.5–4.5% lower on one shared development machine
([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-cpu-targets/REPORT.md)).
Check both on your own server before relying on it.

### Size the server

In our test, one core of an AWS c7a.medium kept up with 500 fully grown bot
players on plain protocol-19 WebSockets (no TLS) for 30 minutes. A tick used
about 4.8 ms of its 7 ms on average, and about 7.1 ms on the food update every
72 ticks
([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-30-official-500-acceptance-v4/REPORT.md)).
Anything else running on that core can still make the server fall behind: in
an earlier run, non-server work took 1.4 s of CPU within 4 s and the server
deferred 126 ticks
([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-30-official-500-acceptance-v3/REPORT.md)).
Keep other work off that core while players are on, for example by scheduling
package updates for quiet hours.

## Large bot worlds

Run one large world per machine and size its tick from a **15-minute or longer
aged run** with the intended player load. Food, snake growth and presentation
work can make a fresh two-minute window misleading. Watch deferred ticks and
application ping as well as CPU: a measured 48k/40 ms world accumulated roughly
350 ms of start lateness alongside reported 350 ms ping. A second 45k world on
separate cores raised a 64k world's mean work from 66.8 to about 84–86 ms;
aging and player counts confound that comparison, but stopping it recovered
75.9 ms work and 80.06 ms median dt.

Optional host `maxTick` is **proposed in
[#384](https://github.com/NikitaMGrimm/slither-sim/pull/384)**, not available on the
main revision checked for this guide. It would stretch overloaded ticks from
`simulation.world.dt` up to that maximum to preserve real-time progress within
the cap. Work above the cap still loses time; size for the aged workload and
monitor `lost_seconds` when using a version that supports it. On current fixed
scheduling, increase the configured dt or reduce load if the world falls behind.

### Build and allocator options

On Linux/glibc, test `GLIBC_TUNABLES=glibc.malloc.hugetlb=1` in the host process
environment (for systemd, add `Environment=GLIBC_TUNABLES=glibc.malloc.hugetlb=1`
to the service). Value 1 requests transparent hugepage advice; it does not need
an explicit HugeTLB pool. One same-binary L64 replay measured 6.9% lower world-step
CPU p50, with system THP settings left at `madvise`. Verify on your workload.

For a source build, fat LTO, native CPU targeting and profile-guided optimization
(PGO) measured 10.9% lower world-step CPU p50 than portable release in a separate
L64 replay, with byte-identical final world/controller output. That comparison
already used hugetlb advice on every arm; the percentages are not additive.
It trained on a different frozen world, not the measured L64 world. The replay
used `bot,parallel`; a networking host needs its own same-feature training.

Use this generate/train/merge/use sequence on the destination CPU, from one
fixed source revision and toolchain. Prepare a finite, representative `serve`
JSONL input tape using the [direct API](bot-api.md); it trains simulator work,
not the socket path. For network-heavy hosts, use a finite host-API training
driver with loopback clients instead. Let the instrumented process exit normally
so LLVM writes its profiles. Keep generation/use features, target, optimization
and panic options aligned; use a fresh profile directory for every revision.

```sh
rustup component add llvm-tools-preview
TARGET=$(rustc -vV | sed -n 's/^host: //p')
PROFDATA="$(rustc --print sysroot)/lib/rustlib/$TARGET/bin/llvm-profdata"
PGO_DIR=$(mktemp -d "${TMPDIR:-/tmp}/slither-pgo.XXXXXX")
mkdir "$PGO_DIR/raw"
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
export CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
export CARGO_PROFILE_RELEASE_PANIC=abort
unset CARGO_ENCODED_RUSTFLAGS

# Generate: the same default features include networking and parallel support.
CARGO_TARGET_DIR="$PGO_DIR/generate" \
RUSTFLAGS="-C target-cpu=native -Cprofile-generate=$PGO_DIR/raw" \
cargo build --release --locked --target "$TARGET" --bin slither-sim

# Train with your prepared finite workload; EOF returns normally.
LLVM_PROFILE_FILE="$PGO_DIR/raw/train-%m-%p.profraw" \
"$PGO_DIR/generate/$TARGET/release/slither-sim" serve \
  < /path/to/training.jsonl > /dev/null
"$PROFDATA" merge -o "$PGO_DIR/merged.profdata" "$PGO_DIR/raw/"*.profraw

# Use: build the host from the same source and features.
CARGO_TARGET_DIR="$PGO_DIR/use" \
RUSTFLAGS="-C target-cpu=native -Cprofile-use=$PGO_DIR/merged.profdata -Cllvm-args=-pgo-warn-missing-function" \
cargo build --release --locked --target "$TARGET" --bin slither-sim
```

Install `$PGO_DIR/use/$TARGET/release/slither-sim` after checking output parity
and an aged workload not used for training. Inspect missing-profile diagnostics;
retrain after source, feature, toolchain or CPU changes. Native builds need the
same CPU features, and `panic=abort` terminates the process on panic.

These are operator options, not a 64k product capacity promise. The public session
also used approximate collisions that stay out of the product. The
[dated report](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-31-64k-session/REPORT.md)
records each measurement's windows, conditions, failures and limits.

## Configure

Save one of these as `/etc/slither-sim.json`. Both listen on port 444 and run
without background bots.

Official mode with 250 player seats:

```json
{
  "bind": "0.0.0.0:444",
  "players": { "max": 250 },
  "bots": { "count": 0 },
  "simulation": { "world": { "preset": "official" } }
}
```

Battledome with 80 player seats:

```json
{
  "bind": "0.0.0.0:444",
  "players": { "max": 80 },
  "bots": { "count": 0 },
  "simulation": { "world": { "preset": "battledome" } }
}
```

Set `bots.count` to keep the world populated when nobody is online. Bots take
their own seats, not player seats, and use CPU even with no players connected.
The official arena grows and shrinks with occupied seats; see
[population sizing](configuration.md#population-sizing) and
[Battledome](configuration.md#battledome). Its stock leaderboard ranks rolling
kills and its minimap defaults to 256 cells, showing inner-pit snakes and the
pit outline. An explicit `simulation.minimapSide` overrides the size. Every
setting is in the [configuration reference](configuration.md#hostconfig).

## Run as a service

Save as `/etc/systemd/system/slither-sim.service`:

```ini
[Unit]
Description=SlitherSim server
After=network.target

[Service]
ExecStart=/usr/local/bin/slither-sim host --config /etc/slither-sim.json
DynamicUser=yes
AmbientCapabilities=CAP_NET_BIND_SERVICE
LimitNOFILE=4096
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now slither-sim
journalctl -u slither-sim -f
```

The log shows `Listening on ws://0.0.0.0:444/slither`, then only warnings and
errors. The service runs as an unprivileged user that may bind ports below 1024.
Raise `LimitNOFILE` above 4096 for more than about 500 clients.

An overloaded server [slows world time](raw-server.md#vps-use) rather than
skipping physics. Watch CPU; memory stays small.

To see the tick rate and load, add `--stats 5` to the `host` command. Every five
seconds the log gets a line like:

```text
stats: ticks/s=142.86 deferred_ticks=0 simulation_load=41.2% players=40 bot_slots=0
```

- `ticks/s`: physics ticks run per second; 142.86 is full speed (one per 7 ms).
- `deferred_ticks`: ticks that fell behind schedule in the interval.
- `simulation_load`: share of one core used by the game loop, not counting
  sockets and bots.
- `players`: connected players, including those waiting to spawn.
- `bot_slots`: configured background bots.

## Connect

Clients connect to `ws://HOST:444/slither`. The stock browser client can be
pointed at it with the [console snippet](raw-server.md#stock-browser-client);
browser mods need their own server setting. Pages served over HTTPS can only
open `wss` connections, which need the TLS proxy below.

The NTL, Frontier and Precise slither browser mods connect when the host's
`IP:PORT` is entered in their custom server field. NTL and Precise get
protocol 13 and Frontier gets 19. NTL's map tracks the live radius; Precise
shows full minimap snapshots but ignores deltas. See
[compatibility limits](raw-server.md#packet-compatibility).

To check that a client can join, run the example bot from any machine with
Python 3.11+ and a `slither-sim` binary:

```sh
curl -fLO "https://raw.githubusercontent.com/NikitaMGrimm/slither-sim/$VERSION/examples/portable_bot.py"
python3 portable_bot.py --binary /usr/local/bin/slither-sim \
  --url ws://HOST:444/slither --steps 100
```

Its summary shows `status=playing` once the bot has spawned. In a crowded
Battledome it may wait for a safe spawn spot first.

## TLS

For `wss`, put a reverse proxy with a trusted certificate in front of the server.
Bind SlitherSim to `127.0.0.1:444`, close port 444 to the outside and add an
Nginx server block:

```nginx
server {
    listen 443 ssl;
    server_name game.example.org;
    ssl_certificate /etc/ssl/game.example.org/fullchain.pem;
    ssl_certificate_key /etc/ssl/game.example.org/privkey.pem;

    location = /slither {
        proxy_pass http://127.0.0.1:444;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 300s;
    }
}
```

Clients then use `wss://game.example.org/slither`. Each client holds two proxy
connections, so allow at least 2048 `worker_connections` for 500 clients.

## Update

Repeat [Install](#install) with the new version, then restart:

```sh
sudo systemctl restart slither-sim
```

A restart starts a fresh world and disconnects everyone. To roll back, install
the previous version the same way. Read the [changelog](../CHANGELOG.md) first:
breaking changes can rename settings, and unknown settings stop startup.

## Resource use

One real AMD EPYC server core, with test clients on a separate machine, no bots
and no TLS. The Zen 4 official rows are two-minute tests of this version
([method and results](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-c7a-main-recheck/REPORT.md));
the Battledome and Zen 5 rows come from an
[earlier revision](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-epyc-index-recheck/REPORT.md)
(five and two minutes). Memory is the host process's peak. CPU is the average
share of one core.

| Server core | Mode | Clients | Playing (mean) | CPU | Memory | Delivery target |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Zen 4 (c7a.medium) | Official | 500 | 500.0 | 61% | 51.1 MiB | Missed |
| Zen 4 (c7a.medium) | Official | 375 | 375.0 | 46% | 36.8 MiB | Met |
| Zen 4 (c7a.medium) | Battledome | 80 | 79.3 | 14% | 12.3 MiB | Met |
| Zen 5 (c8a.medium) | Official | 500 | 500.0 | 31% | 51.0 MiB | Met |

The delivery target is no newly deferred ticks and at most 10 ms of extra delay
for 99% of received frames. A missed target can occur even below 100% average
CPU. These short synthetic tests are sizing examples, not a capacity
guarantee; bots, TLS, larger snakes and longer sessions can add work.

Playing counts exclude clients waiting to spawn. The study reports those waits
separately, including unfinished requests. Measure your intended workload before
choosing a player limit.

## Not included

The server has no game website or assets, server listing, teams, moderation
commands, accounts or persistent records, and it can't record traffic itself;
record on a client instead. Mechanics follow [measured official behavior](server-rules.md),
so servers with custom tunings will feel different.
