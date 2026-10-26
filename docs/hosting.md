# Host a community server on Linux

This runbook targets a systemd Linux server, x86-64 or ARM64, with glibc 2.17+.
Use an account with `sudo`, `curl`, `tar`, `sha256sum` and Python 3.11+.
Run commands in one Bash session. No Rust build is needed on the server.

## Before replacing an engine

- SlitherSim provides a shared protocol-19 WebSocket world, socket player
  seats, optional background Foragers, leaderboard, minimap and kills. See
  [supported packets](raw-server.md#packet-compatibility).
- It provides no game website/assets, discovery listing, team gameplay,
  admin/moderation commands, accounts or persistent records. Optional
  [local records](raw-server.md#reports-and-game-over) last only for the current
  world run. [Host-side wire recording](configuration.md#hostconfig) is unavailable;
  record on a client instead.
- Official mode combines [measured mechanics and provisional models](server-rules.md),
  rather than importing another engine's tunings. Custom speed, growth, food,
  spawn and arena rules may feel different. [Battledome](configuration.md#battledome)
  is a separate custom preset; stock browsers do not display its rolling kill score.
- Ask the previous operator for the public hostname, IP family, port, TLS and
  certificate renewal setup, exact browser/mod and endpoint override, expected
  simultaneous players/bots, custom rules, moderation tools, record/account
  dependencies and a maintenance window. Retain the old executable, config,
  service and proxy config for rollback. Switching engines disconnects players;
  worlds and records are not migrated.

Stock and modded clients need an explicit endpoint: `ws://HOST:444/slither`, or
`wss://game.example.org/slither` on TLS port 443. The path is `/slither`, not `/`.
Use the [stock HTTP client's console recipe](raw-server.md#stock-browser-client),
replacing its `ip` and `po`; mods must offer an equivalent server override.
An HTTPS client page requires a connector that selects `wss`; changing only a
port does not add TLS. Test the community's actual browser bundle before cutover.

## 1. Download and verify a release

Choose a reviewed tag from [releases](https://github.com/NikitaMGrimm/slither-sim/releases).
Keep that tag pinned for both the archive and checksum list:

```sh
set -eu
VERSION=v0.6.0
case "$(uname -m)" in
  x86_64) TARGET=x86_64-unknown-linux-gnu ;;
  aarch64) TARGET=aarch64-unknown-linux-gnu ;;
  *) echo 'No release archive for this architecture' >&2; exit 1 ;;
esac
ASSET="slither-sim-$TARGET.tar.gz"
DOWNLOAD=$(mktemp -d)
cd "$DOWNLOAD"
BASE="https://github.com/NikitaMGrimm/slither-sim/releases/download/$VERSION"
curl -fLO "$BASE/$ASSET"
curl -fLO "$BASE/SHA256SUMS"
awk -v asset="$ASSET" '$2 == asset {print; found=1} END {exit !found}' \
  SHA256SUMS > selected.sha256
sha256sum -c selected.sha256
tar -xzf "$ASSET"
"./slither-sim-$TARGET/slither-sim" --version
test ! -e "/opt/slither-sim/$VERSION"
sudo install -d -m 0755 "/opt/slither-sim/$VERSION" /etc/slither-sim
sudo install -m 0755 "slither-sim-$TARGET/slither-sim" "/opt/slither-sim/$VERSION/slither-sim"
sudo ln -sfnT "/opt/slither-sim/$VERSION" /opt/slither-sim/current
```

Expect `OK` from the checksum check and the selected version from the CLI.
Keep the downloaded archive and checksum list for recovery. Checksums detect
changed bytes; they are not an independent signature.

## 2. Choose seats and mode

Write **one** of these configs. Both listen publicly on IPv4 port 444, reserve
socket seats and disable background bots. Open TCP 444 in the server's existing
firewall/network policy when using direct `ws`. Keep SSH access working.
`[::]:444` is an IPv6 bind; verify IPv4 reachability separately if using it.

Official mode, initially capped at 250 socket players:

```sh
sudo tee /etc/slither-sim/host.json >/dev/null <<'JSON'
{
  "bind": "0.0.0.0:444",
  "seed": 7,
  "players": {"max": 250},
  "bots": {"count": 0, "threads": 1},
  "simulation": {"world": {"preset": "official"}}
}
JSON
```

Or Battledome, with 80 socket seats (see the admission limitation below):

```sh
sudo tee /etc/slither-sim/host.json >/dev/null <<'JSON'
{
  "bind": "0.0.0.0:444",
  "seed": 7,
  "players": {"max": 80},
  "bots": {"count": 0, "threads": 1},
  "simulation": {"world": {"preset": "battledome"}}
}
JSON
```

To keep opponents online when empty, change `bots.count` to the desired count,
keeping `bots.threads: 1` on a small box. Bot seats are additional to
`players.max`, not replaceable player seats. They consume CPU even with no socket
players. The resource samples below have **zero** background bots.
[HostConfig](configuration.md#hostconfig) lists limits; unknown keys fail startup.

Official mode starts sized for background bots (radius 670 with none), then
follows occupied seats. Empty reservations do not enlarge it. Battledome uses
its fixed arena and can keep connections waiting for safe spawn space.
Do not copy an old radius or enable population sizing to increase admission;
first read [population sizing](configuration.md#population-sizing) and the
[preset rules](configuration.md#battledome).

## 3. Install the service

Use a dedicated system account. The unit grants only the bind capability needed
for port 444, which is below 1024; the process otherwise runs unprivileged.
The executable and configuration remain root-owned.

```sh
id slither-sim >/dev/null 2>&1 || sudo useradd --system --user-group \
  --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin slither-sim
sudo chmod 0644 /etc/slither-sim/host.json
sudo tee /etc/systemd/system/slither-sim.service >/dev/null <<'UNIT'
[Unit]
Description=SlitherSim protocol-19 server
After=network.target

[Service]
Type=simple
User=slither-sim
Group=slither-sim
ExecStart=/opt/slither-sim/current/slither-sim host --config /etc/slither-sim/host.json
Environment=RAYON_NUM_THREADS=1
Restart=on-failure
RestartSec=3
TimeoutStopSec=10
LimitNOFILE=4096
LimitCORE=0
TasksMax=32
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
UNIT
sudo systemd-analyze verify /etc/systemd/system/slither-sim.service
sudo systemctl daemon-reload
sudo systemctl enable --now slither-sim
sudo systemctl status slither-sim --no-pager
sudo journalctl -u slither-sim -n 30 --no-pager
```

Expect `active (running)` and `Listening on ws://0.0.0.0:444/slither`.
4096 open files leaves room for 500 clients, pending handshakes and process files;
raise it if increasing the seat budget. Proxy limits are separate. See
[systemd execution settings](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html).

## 4. Prove a client can join

Use the existing Python bot with the installed CLI. It needs only the standard
library; downloading it does not build or install a package.

```sh
curl -fL "https://raw.githubusercontent.com/NikitaMGrimm/slither-sim/$VERSION/examples/portable_bot.py" \
  -o "$DOWNLOAD/portable_bot.py"
python3 "$DOWNLOAD/portable_bot.py" --binary /opt/slither-sim/current/slither-sim \
  --url ws://127.0.0.1:444/slither --steps 100
```

Success is a summary with `status=playing` and advancing `time`, not just exit
code zero: a waiting or disconnected client can also finish the bot loop.
If it dies before the summary, repeat on the empty test world; persistent waiting
in Battledome needs admission investigation. Check logs on errors.

Run the same command from another machine with the public endpoint and a local
release CLI to check the actual firewall, route and TLS. Then join with the
intended browser/mod, steer, boost and use Play Again. A loopback bot alone does
not prove browser compatibility or public reachability.

## 5. Optional TLS proxy

Use this when the browser page requires `wss`. Install Nginx with the operating
system's package manager. Arrange a DNS name and a trusted certificate plus
renewal using the operator's existing certificate tooling before enabling it.
These example paths must contain the full certificate chain and private key.

Change the host config's `bind` to `127.0.0.1:444`, restart SlitherSim and close
public access to TCP 444. Allow TCP 443 instead. Place this server block in a file
included by Nginx's `http` context (for example `/etc/nginx/conf.d/slither-sim.conf`):

```nginx
server {
    listen 443 ssl;
    server_name game.example.org;
    ssl_certificate /etc/slither-sim/tls/fullchain.pem;
    ssl_certificate_key /etc/slither-sim/tls/privkey.pem;

    location = /slither {
        proxy_pass http://127.0.0.1:444;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 300s;
    }
}
```

The [WebSocket upgrade headers](https://nginx.org/en/docs/http/websocket.html)
must reach the backend. Follow Nginx's
[certificate chain setup](https://nginx.org/en/docs/http/configuring_https_servers.html).
Each proxied client uses a downstream and an upstream connection: for 500 clients
allow at least 2048 `worker_connections` in the existing `events` block and an
adequate Nginx service open-file limit (4096 or higher). Reuse the existing proxy
configuration rather than replacing other hosted sites.

```sh
sudo nginx -t
sudo systemctl restart slither-sim
sudo systemctl reload nginx
python3 "$DOWNLOAD/portable_bot.py" --binary /opt/slither-sim/current/slither-sim \
  --url wss://game.example.org/slither --steps 100
```

Require the same `status=playing` result through the public, trusted TLS endpoint.
Check the existing Nginx access/error logs if it fails.

## 6. Logs, upgrades and rollback

```sh
sudo journalctl -u slither-sim -f
sudo systemctl show slither-sim -p MainPID -p NRestarts -p LimitNOFILE
```

The CLI logs startup, warnings and terminal errors; it is not a live player/admin
dashboard. Check CPU/RSS with the operating system's process tools and disk use
with `journalctl --disk-usage`; retain/rotate logs using the system's journal
policy. Overload [slows world time](raw-server.md#vps-use) rather than skipping
physics, so low RSS does not imply adequate CPU capacity.

For an upgrade, reserve a maintenance window and read the new changelog.
Before changing anything, save the current target and config:

```sh
PREVIOUS=$(readlink -f /opt/slither-sim/current)
BACKUP="/etc/slither-sim/host.json.$(date -u +%Y%m%dT%H%M%SZ)"
sudo cp -p /etc/slither-sim/host.json "$BACKUP"
printf 'Previous binary: %s\nConfig backup: %s\n' "$PREVIOUS" "$BACKUP"
```

Repeat step 1 with its `VERSION=v0.6.0` line replaced by the new pinned tag.
Its existing-directory check prevents overwriting a retained version. Install
into that **new** version directory and switch `current` as in step 1. The
running process keeps the old executable until:

```sh
sudo systemctl restart slither-sim
sudo systemctl status slither-sim --no-pager
```

Repeat the local smoke, public endpoint smoke and browser checks. Every restart
creates a fresh world and disconnects all players. There is no automatic
configuration migration or world persistence. Keep the old version/config until
the replacement has passed these checks. To roll back in the same session:

```sh
sudo ln -sfnT "$PREVIOUS" /opt/slither-sim/current
sudo cp -p "$BACKUP" /etc/slither-sim/host.json
sudo systemctl restart slither-sim
sudo journalctl -u slither-sim -n 30 --no-pager
```

If reconnecting later, recover the printed previous target/backup paths first.
Rollback restores the engine/config, not a previous world or its records.

## Small-box resource samples

One run per case on **one desktop core**, Linux x86-64 under WSL2, release build
of `249cdb2` (2026-10-26). Host pinned to one logical CPU; the socket generator
used a separate core. Shared desktop activity was uncontrolled. Each sequential
run used 15 s warmup and a 30 s hold, seed 7, default preset/delivery, no added
latency, no background bots and no TLS. CPU is host process user + system time
over sampled wall time (100% = one logical CPU). Peak RSS includes startup and
warmup; it excludes generator, proxy, OS and kernel socket memory.

| Mode | Requested socket clients | Mean playing clients | Host CPU | Host peak RSS | Deferred ticks |
| --- | ---: | ---: | ---: | ---: | ---: |
| Battledome | 80 | 35.5 | 55.6% | 9.5 MiB | 0 |
| Official | 250 | 250 | 51.4% | 21.6 MiB | 0 |
| Official | 500 | 500 | 91.1% | 44.2 MiB | 0 |

Battledome's **80-client attempt did not produce 80 playing or connected clients**.
It ended with 40 playing; the generator's three-pending-login limit stalls
further connection attempts while admission waits. Host connected count ranged
32–43 during the hold. Admission at 64–80 clients is being reworked; refresh this
row after that work. Do not treat low memory or CPU as evidence of 80-player
capacity. No spawn code or preset was changed for these samples.

Both official runs held the requested playing count throughout the hold. No
decoder/network errors were reported; Battledome had ordinary death/rejoin churn.
The generator used 9.3%, 44.4% and 85.2% of its separate core respectively.
The existing socket harness decodes gameplay and steers every 100 ms; this is
not a heavy human-gameplay or long-lived grown-body test. On a roughly 1 GB,
1-vCPU box, start with at most 250 seats and measure the actual traffic. The 500-client
sample has little CPU margin; background bots and TLS require additional budget.

These are sizing observations, not a minimum-memory specification or a VPS
capacity guarantee. See the
[method, commands and selected results](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-hosting-sizes/REPORT.md).
The existing `host_load` example checks background bots plus one watcher;
the linked socket harness supplies the distinct workload needed here.

## Runbook validation

Checked on 2026-10-26 in Linux under WSL2: downloaded the x86-64 v0.6.0 archive,
verified `SHA256SUMS`, extracted and checked its version. The installation block
also ran with temporary destination paths and without `sudo`; all shell blocks
passed `bash -n`. Both configs parsed and
the existing Python bot reached `status=playing` with advancing time for 100 steps
on both v0.6.0 and the pinned source build. Tests used loopback bind overrides.
The unit passed `systemd-analyze verify` with executable/config paths redirected
to the temporary test installation.

Nginx 1.24.0 passed `nginx -t`; the server block relayed a playing bot on a
temporary plain listener and returned HTTP 101 through TLS with a temporary
certificate trusted by the test client. Only test ports, hostname and certificate
paths changed. No system service was installed/enabled and no privileged bind,
service restart policy, public certificate issuance/renewal, public route,
ARM64 build or browser bundle was exercised. Upgrade/rollback commands were
reviewed; the operator must execute the service and public/browser acceptance
checks on the destination server before replacement.
