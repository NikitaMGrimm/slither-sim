# Protocol-13 host compatibility

## Design and reference

Legacy selectors 11/12 select a recipient-only wire adapter; stock remains 19.
Physics, report clocks, coverage, shared packet preparation, food delivery and
Battledome rules stay shared. Only legacy recipients allocate adapter state.
The bot JSON contract and native decoder are unchanged.

The [official catalog](../2026-10-26-protocol13-catalog/REPORT.md) supplies the
13 layouts. Existing scrubbed fixtures are reused without changing expectations.
Two compatibility choices are client-derived, not claims about official output:

- `[99,4]` gets a single-quoted 24-letter assignment and its rotated reply.
  The official capture returned the 27-copy stock source even after `[4]`.
- Full `U` minimaps carry the shared raster without modern-map flags or deltas.
  Official 13 used `M`. The first body update is absolute to establish an
  integer base after the half-unit appearance; ordinary later steps can be relative.

Legacy messages are individual elapsed-header frames. A private audit of all
65,712 received frames in retained official `02-legacy` found no bundles;
stock `06-stock` included 6,884 bundles. The browser ignored our experimental
bundles even after accepting setup 13; individual messages fixed gameplay.
Source archive/receipt hashes remain in the catalog. No new outside sessions ran.

The adapter consumes canonical host packets, never arbitrary client frames.
Its source front follows 19 angle reconstruction; a separate integer front
prevents repeated 13 rounding from accumulating drift. Both food births and
removals project the shared 300/256 grid onto integer coordinates. Minimap
conversion changes encoding, not the world raster or pit mask.
Shared physical frame groups still consume exactly the original latency RNG
draws; extra legacy frames use those releases. Stock recipients retain their
bytes and due times even in a mixed world (600-tick regression with varied delay).

## Local browser acceptance

Playwright bundled Chromium, sandbox enabled, one-core process affinity;
no simultaneous build or performance process. The unpacked replacement client
9.78 and retained stock assets stayed outside Git. DNS maps the game-page
names to loopback and fails other names. A deny proxy plus HTTP/WebSocket
allowlists blocks all traffic except local assets and the selected loopback host.
No official, community or client-service connections were made.
A minimal local page and discovery callback shim bootstrap the replacement
client; its game/challenge decoder and WebSocket payloads are unchanged.

| Check | Replacement client 9.78 | Stock browser |
| --- | --- | --- |
| Setup, own snake | 13; chosen name and skin 7 correct | 19; chosen name and skin 7 correct |
| Controls | Visible steering; boost speed increased, release worked | Steering, boost and release worked |
| Food | 44 own pickup reports in sandbox; 54 in final pit/rejoin run | Visible pellets and changing length |
| Death/rejoin | Death fade, menu and fresh playable connection | Death, Play Again and fresh playable connection |
| Reports | Size board; pit board showed an actual local collision kill of 1 | Size board received |
| Pit | Edge pellet ring visible; 256-side map with pit outline/inner cells | Covered by unchanged path and offline regressions |

The replacement client's visible menu uses a red **Play** button after death;
its detached stock control says **Play Again**. The actual visible button was
clicked to rejoin. A handwritten local stock peer deliberately hit the player's
body; its decoder reported game-over without errors. No other players participated.
The replacement client rendered map occupancy (400 nontransparent raster cells),
kill counter 1 and leaderboard 1 while the player was alive.
No game JavaScript exceptions occurred. Six replacement-client console errors
were expected blocked external resource requests; stock had 26 such failures.
The tested gameplay screenshot showed the snake and pellets without visible corruption.

Retained failures: first local port was occupied (the new host failed to bind);
a second attempt accepted setup but ignored bundled gameplay. The first pit
run earned a kill but the harness targeted a detached rejoin control and timed out;
the final run used the visible button. Every event log is retained privately.

## Stock trace comparison and checks

Matched Windows x86-64 debug binaries, Rust 1.99.0, seed 7, one CPU,
`CARGO_BUILD_JOBS=1`, `RAYON_NUM_THREADS=1`, single-threaded tests.
Baseline `b40a2ca`; candidate is this change. Command on both binaries:
`perf_baseline client-official-500 1 --trace` (8 warmup records, 1 measured decision).
All 82,283,626 bytes match, including actions, world state, frames and wire records.
Both SHA-256: `3acd05bd2bdeb30a96e188a85870ba4a88423292ac6c0f127defd9cbbab4afc0`.
This is finite deterministic output parity, not measured CPU or live capacity.
No legacy adapter allocation or conversion occurs in that stock workload.

Offline regressions cover catalog setup/appearance/motion/body/growth/food and
common reports, map raster equivalence, first-anchor rounding and 2,000 steps,
large food listings/maps, legacy login boundaries, handshake/control/reconnect,
mixed-recipient byte/due-time parity and Battledome kill board/ring/raster.
Checks and final revisions are listed in the PR validation section.

## Retained private artifacts

Raw captures, browser profiles, scripts, binaries and screenshots remain outside
Git in the task scratch directory. Names below are neutral artifact aliases.
SHA-256 values identify files, not official server conformance or game physics.

| Artifact | SHA-256 |
| --- | --- |
| Sandbox browser events | `c9385900fbd5df12436e3b2324a484e1ff4a658e1a2553338d7729a52f44d0ca` |
| First pit browser events (failed rejoin harness) | `c46e2d493ad5d1d0d54ebc000d0aa02e004ea6611110b695a47bafac2b79d860` |
| Final pit/rejoin browser events | `6a87f372ad61a7bfc8e91019737e3e10ed2485209e61af659f130dcf69d805fe` |
| Stock browser events | `ba30f48831f1ccef2c2e7ee2372780b5a38c511a58dca127a891f2155a183796` |
| Tested host binary | `e183602ae460a056c22938026989e778a5648fed1fc08f90ea23054ab3a17517` |
| Final pit screenshot | `424520a441c3cebcb2eb415985e493ccc28ece97974e149cd468ab94898672fb` |
| Final rejoin screenshot | `01ab0b4c3af33052a2304c606504010467d6bc74d30539f0960954881caacd5d` |
