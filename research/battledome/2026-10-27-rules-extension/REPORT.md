# Opt-in mode rules and the stock client

The SlitherSim `~` extension reports live world rules to bots that request
physical food. The stock browser must not receive it, and an accidentally
forwarded message must not invoke a stock gameplay command.

## Source and method

This is an offline protocol/source regression, not a gameplay capture or a
rendering test. The stock source is `s/game1107249518.js`, SHA-256
`5ad8aa8f06d1e71bad2945001bab077f8d4d279c093a046d19b6f26ac6b62095`.
It is local-only and not redistributed here. The pin identifies the exact source
used; it does not establish compatibility with other client versions.

Inspection found no `~` command handler. The stock connection uses
`new WebSocket(url)`, without the custom HTTP header. The executable check
extracts the actual `window.gotPacket` handler and evaluates it in a Node VM,
with game-object proxies that fail on access. A 24-byte `~` packet completes
with only the ordinary traffic counter increment and no game-state access.

Reproduce with the pinned source supplied as the argument:

```sh
node research/battledome/2026-10-27-rules-extension/stock-client-check.mjs /path/to/game1107249518.js
```

Result: command 126 ignored; `gameStateTouched: false`. No game server or browser
was contacted. This covers the pinned client's normal `testing: false` path;
its debug path additionally records diagnostic counters.

## Delivery and decision regressions

The versioned 24-byte packet, fields and validation are documented in
[raw-server.md](../../../docs/raw-server.md#slithersim-rules-extension).
Rust regressions cover exact encode/decode, malformed-frame atomicity,
setup/reconnect, opt-in reset on seat reuse, unchanged-rule suppression and
subunit changes that do not change the rounded wall report.

An ephemeral loopback host test connects both `NetworkBackend` and a stock-style
socket without the header. The bot sees base, grown and shrinking pit radii;
the stock socket receives gameplay and no `~` command. This establishes local
host routing, not third-party relay behavior.

The Forager kernel is unchanged. Adapter regressions compare heading bits and
boost decisions with #181's configured-pit calculation at frozen integer wall
radii 3584, 7168 and 8960. At subunit wall radii, the new path deliberately uses
the exact reported pit rather than the old rounded-wall approximation. Worlds
without a pit retain the existing conformance and determinism checks.

## Decision and limits

Reuse `X-SlitherSim-Food: physical` as the bot opt-in, with a separate internal
rule-delivery flag. One received-rule path serves native and socket bots.
Client JSON v2 permits additive response fields, so its identifier stays v2.
Removing the Rust configuration constructor is a breaking Rust API change.
Geometric Python and C ABI rule fields already exist and are unchanged; this
change does not introduce socket bindings in those interfaces.
