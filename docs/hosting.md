# Hosting on Linux

Run a public SlitherSim server from a release build as a systemd service. Release
archives cover x86-64 and ARM64 Linux with glibc 2.17 or newer; no Rust toolchain
is needed.

## Install

Pick a version from [releases](https://github.com/NikitaMGrimm/slither-sim/releases)
and your platform (`x86_64-unknown-linux-gnu` or `aarch64-unknown-linux-gnu`):

```sh
VERSION=v0.6.0
TARGET=x86_64-unknown-linux-gnu
BASE=https://github.com/NikitaMGrimm/slither-sim/releases/download/$VERSION
curl -fLO "$BASE/slither-sim-$TARGET.tar.gz"
curl -fLO "$BASE/SHA256SUMS"
sha256sum --ignore-missing -c SHA256SUMS
tar -xzf "slither-sim-$TARGET.tar.gz"
sudo install -m 0755 "slither-sim-$TARGET/slither-sim" /usr/local/bin/slither-sim
slither-sim --version
```

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
[Battledome](configuration.md#battledome). Every setting is in the
[configuration reference](configuration.md#hostconfig).

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

## Connect

Clients connect to `ws://HOST:444/slither`. The stock browser client can be
pointed at it with the [console snippet](raw-server.md#stock-browser-client);
browser mods need their own server setting. Pages served over HTTPS can only
open `wss` connections, which need the TLS proxy below.

Some browser mods use a legacy login that negotiates another wire version.
The observed selector-12 exchange advertised protocol 13 and is currently
unsupported; see [packet compatibility](raw-server.md#packet-compatibility).

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

One 30-second run per case on one desktop core, with test clients on another
core, no bots and no TLS
([method](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-hosting-sizes/REPORT.md)):

| Mode | Clients | Playing (mean) | CPU of one core | Memory |
| --- | ---: | ---: | ---: | ---: |
| Battledome | 80 | 35.5 | 56% | 9.5 MiB |
| Official | 250 | 250 | 51% | 21.6 MiB |
| Official | 500 | 500 | 91% | 44.2 MiB |

500 clients leave little CPU headroom on one core, and bots, TLS and long
sessions with grown snakes add more. Battledome currently admits about 35
players at once while the rest wait for a safe spawn; that limit is being reworked.

## Not included

The server has no game website or assets, server listing, teams, moderation
commands, accounts or persistent records, and it can't record traffic itself;
record on a client instead. Mechanics follow [measured official behavior](server-rules.md),
so servers with custom tunings will feel different.
