# Verification record, October 22, 2026

This is a historical record, not a current test inventory. For checks to run
today, use the [offline check groups](research-workflow.md#offline-checks).
For current model limits, use [rules and evidence limits](server-rules.md).

Checks below were recorded on 2026-10-22. Development hashes may predate a
squashed public history. No remote CI, registry publication or visibility change
was part of these checks. Original import hashes remain in [provenance](provenance.md).

## Local checkpoints

| Checkpoint | Verified results |
| --- | --- |
| Raw host, `e2fa671` | 222 Rust tests on Windows with Rust 1.99.0/1.86.0 and WSL Ubuntu 24.04 with Rust 1.95.0. Formatting/Clippy pass; 102 Python and 147 Node tests pass. The 97-file crate builds and its 222 tests pass from unpacked contents. |
| HUD/session extension | 249 Rust tests on Windows with Rust 1.99.0/1.86.0, formatting/Clippy, 102 Python and 147 Node tests. Release executable builds; Python bot completes 60 native and loopback steps, ending playing. |
| Interface audit, after `03243bb` | 256 Rust tests across 35 binaries on Rust 1.99.0/1.86.0, formatting/Clippy, 102 Python and 147 Node tests. Release build and 60-step native/loopback bot runs pass; package includes the contract fixture. |
| Whole-repository fixes, `dd96044` | 261 Rust tests across 35 binaries on Windows with Rust 1.99.0/1.86.0, formatting/Clippy, 105 Python and 147 Node tests. A 100,000-food loopback snapshot arrives in bounded, ordered messages. |
| Onboarding, `57d21a3` | 261 Rust tests, formatting/Clippy, release build and 102-file crate verification pass. Rust and Python bots complete 200-step native and loopback runs, ending playing. README rendering and local links checked. |
| Core consumer APIs | 268 default and 127 core-only Rust tests on Windows; formatting and both Clippy modes pass. Seven new regressions also pass on Rust 1.86. Core-only WASM builds, with no WebSocket/TLS dependencies. A 122-request seeded baseline matches exactly. |

All checkpoints verify 69 imports against their manifests and pinned Git bytes.
Later extensions were not rerun on Linux or a live endpoint. Offline tests use
doubles/loopback; temporary hosts were stopped and verification artifacts remain
local. Package contents exclude evidence tools, game assets and generated caches.

## Bot transfer demonstration

The same Python `FoodBot` and compiled client ran on all three backends:

| Backend | Actions sent | Protocol 19, self, food and measured ping |
| --- | ---: | --- |
| Native simulator | 158 | Yes |
| Local WebSocket host | 157 | Yes |
| Live endpoint listed by the official site | 159 | Yes |

All exited successfully while playing. The live run used one client for about
7.5 seconds with no retries, using `ws`. It preceded the header-free bundle
correction, which has offline decoder/network regressions. Native `wss` compiled
but was not exercised live. This demonstrates interface transfer, not playing
strength or identical mechanics.

## Stock-client compatibility

The local browser check used the [hash-recorded stock bundle](provenance.md#protocol-references).
Three connections exercised header-free login, self/body geometry, food, minimap,
both turn directions, keyboard turns, boost commands, own death/removal and
reconnects without JavaScript errors. An offscreen boost check ticked the stock
animation and emitted 253/254; it did not validate complete rendering or speed.
Prey and every anchor variant were not exercised in that browser run.

Offline tests cover directional motion, ASCII prey directions, appearance heading
correction, self-death removal and anchor layouts. All 11 archived challenge
replies match. The 2,187-record replay fixture matches archived bytes and reference
decoder checkpoints, not authoritative server internals.

HUD checks fed two retained `l` packets into the stock handler: rank 291/413 and
220/409, ten leaders each. Synthetic `m`, `k` and `S` layouts matched. Team/session
and legacy minimap coverage is parser-based and synthetic, not a live team game.

## Reviews and regressions

Independent Grok 4.7 review covered the host, raw adapter, CLI and header-free
bundles. GPT 6.1 Sol reviews covered HUD/minimap handling, lifecycle failures and
two independent whole-repository passes. Supported findings were fixed and
confirmed in their focused scopes; these reviews did not repeat every suite or
establish exhaustive correctness.

Regressions cover:

- Socket isolation: admission/reuse, direct/challenge login, all framing modes,
  malformed/oversized input, partial HTTP/WebSocket messages, stalled readers,
  login deadlines and measured ping. Capacity errors drop the socket without
  another read; ping timing assertions wait within a deadline.
- Minimap format/resize transitions, keeping legacy `u` pixels separate from
  modern `V` targets, cross-checked with the retained handler.
- Rejected-action processing and atomic reconnect capacity/retry/replay. Lifecycle
  checks and mass updates avoid full observation/body copies without formula changes.
- JSON field/type/lifecycle baselines, allowing response additions while keeping
  requests strict and contracts experimental.
- Broadcast size/order, prey-speed validation before backend replacement,
  malformed UTF-8 recovery, header-free capture bundles, replay self removal and
  bundles missing source snapshots.
- Selected-slot observation parity, supplied-body clearance/respawn resets,
  fractional refill credit, and failed-operation nonmutation. An independent
  GPT 6.1 Sol review found no supported defects in these core-consumer changes.

A new replay test initially reused a growth marker; correction restored all 35
replay tests. Its failed run remains in the local review artifacts.

## Bounds and remaining coverage

Stalled-reader tests show other clients continue, not sustained host capacity.
No throughput benchmark, production VPS deployment or TLS reverse-proxy hosting
was tested. Visibility, minimap generation, relative keyboard turns and several
[mechanics](server-rules.md) remain approximate. The host's record policy is
world-scoped; official daily persistence/qualification is unknown. Supported
layouts do not guarantee every client/server variant.

## Retained local artifacts

These ignored directories retain plans, source/binary/policy hashes, controller
traffic, payloads, checks and failed attempts; they are not distributed in Git.

| Directory under `outputs/` | Scope |
| --- | --- |
| `raw-server-compat-20261022/` | Transfer demonstration and stock-client checks. |
| `protocol-extras-v2-2eiw2v5f/` | HUD/session checks. |
| `interface-audit-20261022-221e5147/` | Lifecycle/interface audit. |
| `repository-review-20261022-6d18ae56/` | Whole-repository fixes, including the failed Python run before marker correction. |
| `public-polish-20261022-dfdd479c/` | Setup, rendering and package checks. |
| `core-consumer-check-20261022-5738850b/` | Seeded baseline/replay, default/core-only test logs and dependency tree. |

Captured and synthetic evidence remain distinct. Hashes establish consistency,
not authenticated origin, independent sampling or scientific validity.
