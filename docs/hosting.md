# Host a server on Linux

This runbook installs a release build as a systemd service on x86-64 or ARM64
Linux (glibc 2.17+). It needs `sudo`, `curl`, `tar`, `sha256sum` and
Python 3.11+, but no Rust toolchain. Run the commands in one Bash session.

## Before replacing another server

SlitherSim serves one protocol-19 world over WebSocket: player seats, optional
background bots, leaderboard, minimap and kills
([supported packets](raw-server.md#packet-compatibility)). It has no game
website or assets, server listing, teams, admin or moderation commands, accounts
or persistent records. It can't record traffic on the host
([HostConfig](configuration.md#hostconfig)); record on a client instead.

Mechanics follow [measured official behavior](server-rules.md), so a server
with its own speed, growth, food or spawn tunings will feel different.
[Battledome](configuration.md#battledome) is our own preset.

Ask the current operator for:
- the hostname, port and TLS/certificate setup;
- the browser bundle or mod players use, and how it picks a server;
- the expected player count;
- custom rules and any moderation, account or record features players rely on.

Keep the old executable, config and proxy setup for rollback. Switching engines
disconnects everyone; worlds and records don't carry over.

Clients connect to `ws://HOST:444/slither`, or `wss://game.example.org/slither`
behind TLS. The path is `/slither`, not `/`. For the stock client, use the
[console recipe](raw-server.md#stock-browser-client) with your `ip` and `po`.
Mods need an equivalent server override. A page served over HTTPS can only use
`wss`. Test the community's actual client before switching.

## 1. Download and verify a release

Pick a tag from [releases](https://github.com/NikitaMGrimm/slither-sim/releases):

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

Expect `OK` from the checksum and the chosen version from `--version`. The
checksum detects corrupted downloads; it is not a signature.

## 2. Write the config

Write **one** of these. Both listen on IPv4 port 444 and run no background bots.
Open TCP 444 in the firewall (keep SSH open).

Official mode, 250 player seats:

```sh
sudo tee /etc/slither-sim/host.json >/dev/null <<'JSON'
{
  "bind": "0.0.0.0:444",
  "players": {"max": 250},
  "bots": {"count": 0},
  "simulation": {"world": {"preset": "official"}}
}
JSON
```

Battledome, 80 player seats (see the [admission limit](#resource-use) below):

```sh
sudo tee /etc/slither-sim/host.json >/dev/null <<'JSON'
{
  "bind": "0.0.0.0:444",
  "players": {"max": 80},
  "bots": {"count": 0},
  "simulation": {"world": {"preset": "battledome"}}
}
JSON
```

To keep the world populated when nobody is online, set `bots.count`. Bots add to
`players.max` rather than taking player seats, and they use CPU even when no
players are connected; on a one-core server also set `"threads": 1`. Unknown
keys stop startup; see [HostConfig](configuration.md#hostconfig) for every setting.
The official arena grows and shrinks with occupied seats
([population sizing](configuration.md#population-sizing)), so don't copy a
fixed radius from another server.

## 3. Install the service

The service runs as its own unprivileged user. Its only extra right is binding a
port below 1024.

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
`LimitNOFILE=4096` covers 500 clients; raise it for more seats.

## 4. Check that a client can play

The example Python bot needs only the standard library:

```sh
curl -fL "https://raw.githubusercontent.com/NikitaMGrimm/slither-sim/$VERSION/examples/portable_bot.py" \
  -o "$DOWNLOAD/portable_bot.py"
python3 "$DOWNLOAD/portable_bot.py" --binary /opt/slither-sim/current/slither-sim \
  --url ws://127.0.0.1:444/slither --steps 100
```

Success is a summary showing `status=playing` with advancing `time`. Exit code
zero alone is not enough: a client still waiting to spawn also exits cleanly.
If it dies first, run it again. If it keeps waiting in Battledome, the arena is
too crowded to admit it.

Then run the same command from another machine against the public address,
and play with the community's client: steer, boost, die and use Play Again.

## 5. Optional: TLS proxy

Use this when the client page needs `wss`. You need a DNS name and a trusted
certificate with renewal, set up with your existing certificate tooling.

Change `bind` to `127.0.0.1:444`, restart SlitherSim, close TCP 444 and open 443.
Add this server block to the Nginx `http` context, for example
`/etc/nginx/conf.d/slither-sim.conf`:

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

Each proxied client holds two connections. For 500 clients, allow at least 2048
`worker_connections` in the `events` block and 4096 open files for Nginx.

```sh
sudo nginx -t
sudo systemctl restart slither-sim
sudo systemctl reload nginx
python3 "$DOWNLOAD/portable_bot.py" --binary /opt/slither-sim/current/slither-sim \
  --url wss://game.example.org/slither --steps 100
```

Expect the same `status=playing` result.

## 6. Logs, upgrades and rollback

```sh
sudo journalctl -u slither-sim -f
sudo systemctl show slither-sim -p MainPID -p NRestarts -p LimitNOFILE
```

The log shows startup, warnings and errors only; there is no live dashboard. An
overloaded server [slows world time](raw-server.md#vps-use) instead of skipping
physics, so watch CPU, not just memory.

To upgrade, read the changelog, then save the current version and config:

```sh
PREVIOUS=$(readlink -f /opt/slither-sim/current)
BACKUP="/etc/slither-sim/host.json.$(date -u +%Y%m%dT%H%M%SZ)"
sudo cp -p /etc/slither-sim/host.json "$BACKUP"
printf 'Previous binary: %s\nConfig backup: %s\n' "$PREVIOUS" "$BACKUP"
```

Repeat step 1 with the new `VERSION`; it refuses to overwrite an installed
version. Then restart:

```sh
sudo systemctl restart slither-sim
sudo systemctl status slither-sim --no-pager
```

A restart starts a fresh world and disconnects everyone. Repeat the step 4
checks. To roll back:

```sh
sudo ln -sfnT "$PREVIOUS" /opt/slither-sim/current
sudo cp -p "$BACKUP" /etc/slither-sim/host.json
sudo systemctl restart slither-sim
sudo journalctl -u slither-sim -n 30 --no-pager
```

## Resource use

One 30-second run per case on one desktop core (Linux under WSL2, release
`249cdb2`), with test clients on a separate core, no bots and no TLS
([method](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-hosting-sizes/REPORT.md)).
CPU is the host process's share of one core; memory is peak RSS.

| Mode | Clients | Playing (mean) | CPU | Memory |
| --- | ---: | ---: | ---: | ---: |
| Battledome | 80 | 35.5 | 56% | 9.5 MiB |
| Official | 250 | 250 | 51% | 21.6 MiB |
| Official | 500 | 500 | 91% | 44.2 MiB |

Memory is small; CPU is the limit. 500 clients leave little headroom, so start
a one-core server at 250 seats and measure real traffic. Bots and TLS cost extra.

**Battledome doesn't reach 80 players yet.** Spawn admission keeps about 35
alive and the rest wait. That is being reworked; this row will be refreshed.

The test clients steer every 100 ms and stay small, so long sessions with
grown snakes will cost more.

This runbook was last checked on 2026-10-26 with v0.6.0: download and checksum,
both configs, the join check, `systemd-analyze verify`, and the Nginx block
relaying play over TLS with a test certificate. Not yet exercised: an installed
service, public certificates, ARM64 and browser clients.
