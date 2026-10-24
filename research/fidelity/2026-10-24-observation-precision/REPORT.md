# Observation precision for the geometric controller

**Question.** How finely must `slither-sim serve` print observation numbers?
Replies printed full doubles such as `-352.05080564444495`, which made them large
and slow to parse. Which rounding steps stay well inside what real players
receive and what the simulation's geometry can distinguish?

**Decision.** `slither-sim-json-v2` frames round coordinates to **0.01 world
units**, headings to **0.0001 rad** and speeds to **0.01 units/s**. Every other
number stays exact, including radii, length, fam, score, food value and size,
time and vision. Rounding happens only when a frame is written, never in
simulator state, snapshots, the portable client or the WebSocket host.

## Method and pins

- Source facts come from slither-sim `0d367f6f98b94c046684bc8bf70d17e860fea5e9`:
  the protocol-19 decoder `src/client/protocol.rs`, `src/physics.rs`,
  `src/simulator.rs` and `src/prey.rs`. They state what packet bytes can express
  to a recipient, not the hidden official server's internal precision.
- The capture measurements cover the first 5,000 incoming frames of three
  retained official-server recordings from the Arena archive. They are
  **local-only**: the recordings and the probe's output stay outside Git, and
  no capture-derived values are copied here. [`probe.py`](probe.py) (SHA-256
  `22d4a08943d1a71d2c2dd56dd0bae72c8f4617700b8930b423b58961848776b5`) extracts
  raw fields with `tools/fidelity/slither_protocol.py` and checks each against
  its grid, with a tolerance of 1e-7 steps. Every setup reported protocol 19,
  sector size 300, anchor spacing 42 and grid center 32550.

| Capture (Arena `recordings/headless/`) | Frames | Packets | SHA-256 of archive |
| --- | ---: | ---: | --- |
| `fidelity-paired-coil-20260927/bot0.jsonl.gz` | 5,000 | 7,950 | `559225fcb6316c565ff5642fb038aa28e8b5bd1c0a5d019471a3c90e8862c719` |
| `prey-controlled-20260927-first/bot0.jsonl.gz` | 5,000 | 14,920 | `c65f2d0b248f851743963b067dc53767028eb63d2709c72f35766b4d3c45ba7e` |
| `anchor-encoder-holdout-20260928/bot0.jsonl.gz` | 5,000 | 10,640 | `07998f1c967fde0e6c78f88399b89d2ab27b33516c83051db0ac9ff659e6d7be` |

## Outcomes

**Wire resolution (decoder facts, checked against the captures).**

| Field | Protocol-19 step | Values checked | Off grid |
| --- | --- | ---: | ---: |
| Snake spawn head and first anchor | 0.2 units (`u24/5`) | 536 | 0 |
| Spawn body deltas | 0.5 units | 13,136 | 0 |
| Anchor directions (spawn, `g`/`n` updates) | 2π/65536 rad | 6,193 | 0 |
| Absolute anchor updates (`=`) | 1 unit | 14 | 0 |
| Food position / size | sector/256 = 1.171875 units / 0.2 | 26,826 | 0 |
| Snake spawn / motion headings | 2π/16777215 / 2π/256 rad | 23,069 | 0 |
| Snake spawn / motion speeds | 0.03125 / 1.736 units/s | 3,955 | 0 |
| Fam | 1/16777215 | 4,951 | 0 |
| Prey spawn / update positions | 0.2 units / 3 units | 990 | 0 |
| Prey headings / speeds / sizes | 2π/16777215 / 0.03125 / 0.2 | 1,138 | 0 |
| Arena radius (`z`) | 1 unit | 410 | 0 |
| **Total** | | **81,218** | **0** |

Food `value` is not transmitted; clients derive it from size, and simulator food
satisfies `value = size² × 0.0031535`. Positions a client reconstructs are not on
these grids: anchors rebuilt from directions, contracted anchors and extrapolated
heads take arbitrary values (2,664 of 2,668 sampled head components were off
the 0.2 grid). The grids bound what each packet field can say, not every derived
position.

**Simulation scales (default settings, from source).** The smallest snake has a
drawn radius of 14.5 and a width of 29. Anchors start 42 apart, and food pickup
reaches at least 36 units. A tick is 7 ms; the slowest snake moves 1.27 units
and turns at most 0.0289 rad per tick.

**Error budget of the chosen steps** (nearest rounding, so half a step at most):

| Field | Step | Finer than the wire by | Worst error relative to the scales |
| --- | --- | --- | --- |
| Head, anchors | 0.01 | 20× (spawn) to 100× | 0.0071 per point, 0.05% of the smallest radius; 0.0142 between two points, 0.03% of anchor spacing |
| Food position | 0.01 | 117× | 0.0142 between snake and food, 0.04% of the smallest pickup radius |
| Prey position | 0.01 | 20× to 300× | same bound as food |
| Snake and prey headings | 0.0001 rad | 245× finer than motion updates; 267× coarser than spawn angle24 | 0.17% of the smallest snake's turn per tick; at most 0.004 units at the head tip of a maximum-size snake |
| Speeds | 0.01 units/s | 3× (spawn) to 174× | 0.00004 units per tick |

Two fields can be finer on the wire than the chosen step: spawn and prey angles
(angle24) and direction-reconstructed anchors (about 0.004 units of chord per
direction step). The bounds above, not a wire grid, justify those coarsenings.

On v2 frames, rounding food `value` to 1e-6 would have saved only 0.7–2.3% of
reply bytes, so it stays exact. The measured savings of rounding are in the
v2 change itself: about 11 bytes per coordinate or heading.

**WebSocket players are unaffected.** The host owns its own simulator and encodes
protocol-19 packets directly from snake, prey and food state
(`src/client/simulation.rs`, `src/client/encode.rs`). It never reads `serve`
observations or frames, so rounding them cannot change host packets or physics.

## Limitations

- Three recordings, prefixes only; no adjacent pair of codes was seen in every
  category (prey spawn positions were whole units). Rare `+` updates did not occur.
- Wire grids describe received fields, not the official server's internal state.
- Rounding can still change a bot's action when a value sits at a decision
  threshold; the budget bounds the numeric error, not action equivalence.
- Simulation scales use default settings; a configured `dt` or arena can be smaller.

## Reproduction

With the three recordings in `ARCHIVE_DIR/<capture>/bot0.jsonl.gz`:

```sh
python -B research/fidelity/2026-10-24-observation-precision/probe.py ARCHIVE_DIR outputs/precision-probe.json
```

It prints and writes the per-field counts, maximum grid residuals and formatting
savings. Keep that output outside Git; it contains capture-derived values.
