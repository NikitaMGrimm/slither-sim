# Legacy minimap projection

## Client-derived map correction

Base: `20e5064` (same host code as `fa92c47`). NTL 9.78 source was read as
untrusted data; only the sandboxed extension ran. Source SHA-256:
`f7ecd6f9b577dd2343c8d390c1570deb54708ffe138ca7bdd599f58d260f0eb8`.
No third-party source or assets are distributed. No outside servers contacted.

`sJ` scales the dot by live `LX` only while `NJ && e6`; otherwise by setup
`uX` (`grd`). The `U/u` handler clears `e6`. `c9` at Qm>=13 reads `M/V` from
last cell backwards, high bit first, with `255` runs and XOR for `V`. The
shared encoder uses that same order. The earlier acceptance checked map ink
and missed marker alignment; the old claim that live-radius `U` was safe
with dynamic maps off was incorrect.

Legacy selectors 11/12 now pass shared `M/V` through by default, matching the
[official 13 catalog](../2026-10-26-protocol13-catalog/REPORT.md). Only `[99,3]`
selects per-recipient `U`: decode the canonical map, conservatively reproject
occupied cells from live radius to `grd`, then encode plain zero runs. Setup
and `z` update that projection; reconnect clears the choice and delta base.
No shared broadcast preparation or stock wire encoding changed. #195 was not
used. Precise now sees `M` snapshots; its reader still ignores `V`.

## Measured map cases

Playwright bundled Chromium with the sandbox on, one logical CPU, fail-all
external DNS, loopback game-page mapping, deny proxy and HTTP allowlist. Native
WebSocket traffic was observed passively with CDP; only loopback bypasses the
deny proxy. Sources, raw frames, screenshots and failed attempts stay in durable
private task scratch. Viewport 1100x800, unchanged client map size. The browser
context uses its default scale; DPR was not logged in these retained cases.

The asymmetric body is off-center in both axes. The independent synthetic
regression uses world (1200,-1600), radius 2400 and a 256-side raster: cell
(192,42), not its 180-degree counterpart (63,213). It tests both M and V,
including extended runs, without using the library decoder as its oracle.
Real-client alignment uses the actual drawn dot (including its image-center
and map-inset offsets) and occupied pixels from NTL's own `uf` canvas.

| Real NTL case | Setup / live radius | Dot distance to occupied map cells |
| --- | --- | --- |
| Battledome, pit 900, off-center own trail | grd 32550 / radius 1800 | 0.20–1.71 cells after entry; first sample 4.67 |
| Default preset, dynamic radius, four bots | 32550 / 1498 | 0.30–2.28 cells (eight samples) |
| Fixed radius, largest admitted value | 32550 / 32000 | 0.10–0.69 cells (nine samples) |
| Seat joins, then leaves; actual `z` messages | 32550 / 1498→1641→1596→1544 | 0.05–1.15 cells (nine samples) |
| Dynamic minimap disabled, `[99,3]` | 32550 / 1800 | 0.20–1.22 cells (nine samples) |

These are distances from the dot center to the nearest nontransparent cell of
NTL's map canvas, in raster cells, not screen pixels. The marker refreshes on
its own clock; map snapshots and movement are asynchronous. Distances therefore
include refresh/quantization error and are not head-coordinate error estimates.
The off-center own trail and screenshots distinguish reflection from alignment;
a centered pit outline alone cannot. No page exceptions occurred in these runs.
The baseline Battledome sample had a 39.01-cell mismatch after `U` disabled
dynamic scaling; the candidate retains `e6` and uses live-radius `M/V`.

**Equality boundary:** a real host cannot have live radius equal to setup grd:
the admitted radius is at most 32000 and grd is 32550. The fixed-radius row
checks the closest valid config. A separately labeled, freshly authored local
replay checks exact equality without changing the host cap or stock setup:
grd=radius=8000, own head (10000,6000), an asymmetric 256-side map. Real NTL
places the dot at (160,96), exactly on its own occupied line, alpha 255.
This is a synthetic wire-boundary check, not a real-host configuration.

## Retention and limits

Durable private directory: `protocol13-map-motion-20261027`. Selected raw
JSONL SHA-256 values are recorded below; full frames and screenshots stay out
of Git. The synthetic encoder tests are independently authored format facts,
not copies of third-party code. No motion/pacing behavior is changed or claimed
by this correction; that investigation remains separate.

Fresh native-input NTL acceptance: correct nickname and skin 7, steering,
boost/release while alive, 33 own food pickups, one credited kill and kill
leaderboard, death and Play Again. Map: one M and six V; no U. No page
exceptions. The 1100x800 browser viewport reported DPR 1. The first acceptance
attempt died during the late boost check; the retained retry checks boost and
release earlier in the life, before steering into danger.
Raw `map-native-e2e-02-events.json` SHA-256:
`bbd57d655acf4c1659585393a201b99f29033e624f188dd6e068fd5e66e5f18f`.

Stock-only official-500 trace, debug baseline and candidate: all 82,283,626
bytes identical, SHA-256
`3acd05bd2bdeb30a96e188a85870ba4a88423292ac6c0f127defd9cbbab4afc0`.
The regression suite also checks captured M/V byte identity, synthetic reverse
scan, setup/z transitions, the full-grd projection and request selection over
real loopback sockets. No contract identifier or simulation rule changed.

- `map-before-03-events.jsonl`: `5c52c7852e7cb41aa95a0d42a483546898694ded529be2634ab9c0649fc7df63`

- `map-case-map-events.jsonl`: `300a312e9a6ebad525adf97275eb0883f83027f763903935cb1dff1c2ec73871`

- `map-case-dynamic-events.jsonl`: `082b8d0fa28db7cf5fad0637b3885dc899206327dc145ba659c9e5713a648359`

- `map-case-fixed-events.jsonl`: `a36346e2cfa03c3fbf5e35ebc60751072b3b993b826790aaafda74cb8caded9a`

- `map-case-change-events.jsonl`: `e1eae9c7dd2b8f12e5e3834befb48950328789cd6995574be0d3d0534e30a4b6`

- `map-case-off-events.jsonl`: `17b440d7c9a13a1f8501a63207ce4889df9b047ca1ad8bcb3a2a90026b2057d9`

- `map-equality-events.jsonl`: `8b61febe889e259c45a37af3865fa164b08b5267e3176321e40a58f321490f2e`

- `grd-equality-synthetic.jsonl`: `212659d1622483c369096e21f205afa452cb35ba93504385d87e8b14519fa7d6`
