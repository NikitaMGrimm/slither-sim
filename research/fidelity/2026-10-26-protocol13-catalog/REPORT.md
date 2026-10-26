# Official protocol-13/19 message catalog

## Scope and acquisition

Official servers are the reference here; no community connection or mod code was used.
One endpoint remained in the fresh official HTTPS listing; its address stays private.
Listing membership does not cryptographically authenticate the plain-WebSocket peer.
Six connections ran sequentially on two allowed logical CPUs; host/encoder code is unchanged.
The public sequence `[4]`, `[99,4]`, parsed reply, selector-12 login reached gameplay.
It still returned the double-quoted 5+22-character/27-copy challenge, not a 24-byte profile.
The original data-only restricted solver returned 27 bytes; received source was never executed.
`S` reports followed appearances. Thus the extra `[4]` does not resolve that challenge mismatch.

The controller sought food, steered every >=120 ms, briefly boosted when clear,
avoided opponents and steered outward after 100 s. `v0` establishes game-over.
Each next connection was a new life. The stock route used the same schedule,
not simultaneous world states. No player was targeted.

| Archive | Wire | Seconds | Own food/eater-ID receipts | Own growth | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| 01-sequence | 13 | 12.586 | 18 | 1 | Sequence probe, clean close |
| 02-legacy | 13 | 151.424 | 137 | 11 | Game-over |
| 03-legacy | 13 | 23.012 | 17 | 2 | Early death, retained |
| 04-legacy | 13 | 124.560 | 188 | 18 | Game-over |
| 05-stock | — | 0.844 | 0 | 0 | Closed before setup; placeholder password |
| 06-stock | 19 | 127.689 | 78 | 13 | Corrected stock login, game-over |

Attempt 5 used zero password bytes; attempt 6 used the fixed bytes documented in
`src/client/network.rs` at base `d5a6c70`. The original collector/plan and amendment are retained.
This was the sole retry. Boost-on packets were recorded in both long legacy lives and stock.

## Layout catalog

Integers below are unsigned big-endian; widths are bits. `id` is 16, `fam` is 24.
`xy16` means two 16-bit coordinates; appearance coordinates are 24-bit values divided by 5.
Layouts/lengths are observed and checked by the independent offline decoder.
Field meanings/scales and replacements are **interpretations (I)**, not hidden physics
measurements or a complete specification.
Observed lengths/counts and per-code timing are in [catalog.json](catalog.json).

| Code | Observed wire-13 layout after code | Wire-19 counterpart / difference (I) |
| --- | --- | --- |
| `6` | 184 encoded letters for the fixed 27-copy challenge | Same shape in this sample; request variant differs |
| `a` | grid24, maxSegments16, sectorSize16, sectors16, speed/turn constants, version8, spacing8, server16, radius24, opaque extension | Same prefix; version 19 rather than 13 |
| `s` full | id, heading24, direction8, wanted24, speed16, fam, skin8, head24×2, name length/data, custom length/data, cosmetic8, tail24×2, signed half-unit delta pairs | Same header; final body pair is a 16-bit direction in 19 |
| `s` short | id, death-flag8 | Same observed four-byte form |
| `e/E/3/4/5` | id plus 1–3 heading8/wanted8/speed8 fields, selected by code and length | Peer forms retain id; short self forms omit it; own `d/7` carry all three |
| `g` | id, absolute xy16 | 19 `g` carries an optional angle16; 19 `=` carries angle16+absolute xy16 |
| `G/N` | id, signed relative x8/y8 (subtract 128), then fam for `N` | 19 selects `g/G` or `n/N` by recipient; self id omitted, optional angle16 |
| `n` | Not observed in 13 | Observed 19 peer growth: id, optional angle16, fam |
| `=/+` | Not observed in 13 | `=` observed in 19 for absolute correction; `+` unobserved |
| `h/r` | id, fam (all observed forms six bytes) | Same observed mass/tail fields |
| `F` | repeated color8, x16, y16, size8 tuples | 19 sector8×2 header, then color8/offset8×2/size8 tuples |
| `f/b` | color8, xy16, size8 | 19 sector/offset form; sector and color may be inherited/omitted by length |
| `c` | xy16, eater-id16 | 19 observed `C` without eater and `<` with eater; sector/offset coordinates |
| `C/<` | Not observed in 13 | Explicit/inherited sector, offset8×2, eater-id16 only for `<` |
| `w` | sector8×2 | Same sector-clear fields |
| `M` | side16, reverse-order seven-bit literal/zero-run stream; 255 escape | Same full-map structure; 19 also used `V` |
| `V` | Not observed in 13 | Delta byte stream; XOR meaning is convention-based, not independently raster-validated here |
| `l` | position8, rank16, population16; rows of segments16/fam/color8/name-length8/name | Same layout |
| `k` | id, count24; two reports of value 1 | No `k` in the 19 comparison; same-layout support remains an interpretation |
| `p/z/v` | empty pong / radius24 / status8 (`v0` observed) | Same observed forms |
| `S` | id, opaque32 | Same structure; session identity is not authentication |
| `y` | id; full color/position/size/direction/angle/speed fields, or removal with optional eater-id | Same observed 20/3/5-byte forms |
| `j` | id, xy16 on 3-unit grid, direction8+angles24×2, optional speed16 | Same observed 14/16-byte forms |

Every received gameplay code/length in the accepted corpus was recognized; no packet was
silently classified as a supported unknown. Setup extension bytes remain opaque and are scrubbed.
The 13 `n`, 19 `+`, alternate minimaps, records, teams, admin packets and score credits were unobserved.
Two `k=1` reports were received in 13; collision attribution and a complete counter transition were not established.

## Receipt timing and coverage

The JSON gives per-code/version counts, distinct frames, gap denominators, median/p95 and lengths.
Gaps pool **within** sessions; one code counts once per frame, never across reconnects.
These are route statistics, not tick periods, per-entity laws or conformance tolerances.

| Code | 13 receipt-gap median / p95 ms | 19 receipt-gap median / p95 ms |
| --- | ---: | ---: |
| `l` | 3007.798 / 3018.349 | 3009.212 / 3017.694 |
| `M` | 2504.192 / 2512.357 | 2506.738 / 10018.870 |
| `V` | Unobserved | 2512.416 / 15009.089 |
| `j` | 334.246 / 347.231 | 331.927 / 343.865 |
| `p` | 505.175 / 559.437 | 506.714 / 557.755 |

19 alternates map snapshots/deltas, so `M` alone is not its map-update period.
Event/visibility-dependent food, creation, death and motion gaps are reported in the JSON, not fitted.
In particular, 13 `G` includes explicit IDs for many snakes, while 19 `G` is self-only:
comparing those pooled rates as if they covered the same actors would be invalid.

## Fixtures, privacy and reproduction

[Wire 13](../../../tests/fixtures/protocol-catalog-13.json) and
[wire 19](../../../tests/fixtures/protocol-catalog-19.json) contain small representative witnesses,
including self/peer creation, growth, map, reports and game-over; not every observed length is retained.
`frameBase64` stores **derived**, not raw frames: names/text and identities are scrubbed at explicit ranges.
Setup extensions are zeroed; redacted values are not encoder expectations. Length fields are preserved.
`selection` extracts one payload from a larger frame while retaining its elapsed header;
that reframing is synthetic. Original archive/frame hashes, record indices and offsets are retained.
No display names, endpoints, mod assets/code or local personal paths are published.

The standard-library [decoder](../../../tools/fidelity/protocol_catalog.py) was independently written
from documented layouts and packet inspection, with no mod imports or JavaScript execution.
`catalog.json` pins decoder and private collector/exporter/plan hashes. Full compressed JSONL,
HTTP bytes, sent controls, source snapshots and failures are **local-only, outside Git**.
Raw SHA-256 values (full gzip streams audited to EOF):

| Archive `.jsonl.gz` | SHA-256 |
| --- | --- |
| 01-sequence | `46b34a85d9612bd0fc7a1adbced14e72c28d463b69092a124fa8876dec567797` |
| 02-legacy | `85068f4b2f3ac7dfea2500c5fc41d082a47ca2334fec4d16f35be235e6cf5aca` |
| 03-legacy | `cf8985761adf3826dd70eb7b2ae2c7e72b5d092914f0a80cbad70e76d7f177b3` |
| 04-legacy | `d39a726aa48cb29fd47dfd393afc1adea618a0b0b7775d3a3157a817667e4ebe` |
| 05-stock | `b30f8d9281d8d1bf2e67e05b6dc367461e2cc657e03f9eb05352793bc3f94fb1` |
| 06-stock | `32355b0af44922a667a84a9695544cd06366ffc6f40ae40374e03e9bef4c9048` |

Run `uv run --locked --only-group tools python -B -m unittest discover -s tools/fidelity -p test_protocol_catalog.py`.
Tests are offline: selected decoded values, handwritten arithmetic checks, scrubbing and network-free imports.
With private access, verify hashes, replay indexed frames, apply redactions/extractions
and compare fields. Hashes certify identity, not model truth.
This exploratory catalog prepares a later encoder; it adds no host behavior or runtime protocol support.
