# Shared browser admission

The later [projection correction](../2026-10-27-legacy-map-motion/REPORT.md)
supersedes the minimap choice below. This run checked nonempty map pixels,
not own-marker alignment; it missed that `U` disables NTL's live projection.

## Scope and source pins

Client-derived compatibility, not official challenge behavior. Static survey
of locally retained sources plus offline loopback acceptance; no external game
servers or client services contacted. Third-party source/assets remain private
and are not distributed. The implementation and synthetic fixture were written
from format facts, without copying client code.

| Retained source | SHA-256 |
| --- | --- |
| Replacement client 9.78, `main-mt.js` | `f7ecd6f9b577dd2343c8d390c1570deb54708ffe138ca7bdd599f58d260f0eb8` |
| Frontier Slither 6.4.2, `d/main.js` | `2e8927ffbf98d23ad6a9d372c506c29f6b7f101820121f78030a74f9779f860b` |
| Precise slither 0.6, `reference.user.js` | `1e048ed128f486576a55a470c154f093ee24f971216ca0d976512e6d2aa00448` |

## Admission facts and choice

- Replacement 9.78: `$w` extracts the first single-quoted literal after cipher
  decoding, then rotates its first 24 letters. `QG` sends `[99]` for selector
  11, otherwise `[99,4]` or `[99,3]` with dynamic minimap off. Its `c9` map reader
  and `U` dispatch support the existing full `U` map, including that off mode.
- Frontier: `i5/o5/l5` read 24 characters at decoded-source offset 7 for custom
  endpoints. Rotation starts at 2 plus the first letter index, advancing by
  3 plus each letter index. `K5` sends selector 31 and version bytes 1/161,
  then skin/name length at offsets 4/5. This is a login layout selector.
  `Y5.s` reads modern terminal angle points at version >=15; `j5/N5` read
  modern body/motion; `O5/F5` modern food; `J5` modern maps; `Z5` splits bundles.
  Therefore selector 31 keeps existing protocol-19 output, without a third
  output version. Its separate official-endpoint solver is not used locally.
- Precise: `decodeSecret_29` (line 141) evaluates decoded source in a closure
  with a zero-filled 24-byte `idba`, then rotates that buffer with the same
  recurrence. A string-only declaration leaves the buffer unpopulated. Its
  challenge handler (line 592) then sends selector 12; setup must advertise 13
  (line 482). Its only minimap handler is `M` (line 845).

The fixed local challenge starts `var a='` (first letter at offset 7) and
populates all 24 `idba` bytes from that literal. The host generates fixed source
and never evaluates received source. The reply remains
`CGLRYGPZKWJXMCTLEYTPMKJJ`. NTL's quoted extraction ignores the trailing fill;
Frontier's fixed positions also ignore it; Precise executes the fill.
The [official catalog](../2026-10-26-protocol13-catalog/REPORT.md) instead
observed a 27-character challenge even for `[4]`, `[99,4]`; this choice serves
client solvers rather than claiming to reproduce that official source.

Selectors 11/12 retain protocol 13. All accepted layouts require two reserved
trailing bytes and enforce the same 24-byte nickname / 255-byte custom limits.
Stock selector-30 offsets, challenge and output are unchanged. The
[synthetic fixture](../../../tests/fixtures/browser-admission.json) pins the
short selector-31 layout and expected protocol 19, not an official observation.

Precise and NTL share `[99,4]` / selector 12. There is no evidence-backed request
field to choose between their map readers. Keep `U` for NTL; Precise joins with
no minimap. MySlither and Slyther remain out of scope, without new protocols.

## Local verification

Playwright bundled Chromium ran with the sandbox enabled and inherited one-core
affinity. DNS mapped game-page names to loopback and failed other names; a deny
proxy and HTTP/WebSocket allowlists excluded every external request. Saved
extensions/userscript and a minimal local page stayed outside Git. Client game
code and WebSocket payloads were unchanged. No outside sessions were needed.

| Client | Measured result | Limit |
| --- | --- | --- |
| Replacement 9.78 | Setup 13, chosen name/skin 7; steering, speed 5.83 -> 14 -> 5.83, 44 own food pickups, death, playable rejoin; pit kill board 1, 256-side map with 401 occupied cells, visible edge ring | Six/seven expected blocked-resource console errors; zero page exceptions |
| Frontier 6.4.2 | Saved extension did not bootstrap a game UI; its declared icon and image assets are missing. Handwritten client reproduced custom fixed-offset solver, `[4]`, `[99,4]`, selector-31 login in two sessions: name/skin 7, setup 19, movement, food, board, map, death and reconnect | Actual rendering inferred from its modern parser, not measured |
| Precise 0.6 | Real unmodified userscript's eval solver sent the expected reply; setup 13, default name/skin 1, visible snake/food/board, steering/boost, 38 own food pickups, death and New game; two setups, zero page exceptions | `U` ignored: no minimap |

NTL's pit/death/rejoin test was repeated after both rebases; the final debug
host had 27 own pickups, kill board 1, map occupancy 401, zero page exceptions. The earlier
release-host browser runs also passed; Precise/Frontier evidence is from that
pre-rebase binary. Inherited main changes isolate conversion faults and share
minimap preparation without changing successful gameplay bytes or admission.

The first pit harness run clicked Play with an empty endpoint and opened no
client game socket. Retained as a failure; saving the loopback endpoint before
Play fixed the harness. A scripted stock peer deliberately hit the NTL snake's
body for the kill-board check. No other players participated.
The additional `[99]` / `[99,3]` flows passed loopback handshake/control/reconnect
regressions; NTL's rendering for those optional settings is source-derived.

Stock `client-official-500 1 --trace`, matched debug build, seed 7: all 82,283,626
bytes equal the retained main `ac8552891db836ab43fdb15f18017179f1c445c4`
reference, SHA-256
`3acd05bd2bdeb30a96e188a85870ba4a88423292ac6c0f127defd9cbbab4afc0`.
This is finite deterministic output parity, not a timing or capacity claim.
After rebasing onto merged #190 and #192 (`ac67b56`), the same trace still matches;
217 client unit tests (one explicit benchmark ignored), 84 protocol/socket/
contract/runner/transport tests, all-target Clippy, formatting and 20 documentation/
fixture tests pass offline with one-core affinity and single-threaded tests.
An initial release-vs-debug comparison differed only in the trace metadata's
`release` boolean; that failed comparison is retained, not normalized away.

## Retained private artifacts

Raw events, screenshots, sources and binaries stay in durable task scratch
outside Git. Hashes identify files, not official conformance.

| Artifact | SHA-256 |
| --- | --- |
| NTL sandbox events | `08a8616f654c00e1b5674042202e418fd0a451c28b784bbcfe6f9eae4b5cc83f` |
| NTL first pit events (empty endpoint failure) | `a7841e725bdd459e7c59d9252cf48587b6cbec31d82338672d801a672300b5f7` |
| NTL pit/rejoin events before rebase | `c84bc90c0f1ae666b987ad2ef213b8e164c6696f69fc9283845d6dbd21ba17aa` |
| Frontier failed bootstrap events | `a23d216189901fd7397c27cefe137ffea0e7fdf625720d1ff0d048ae3595a946` |
| Frontier scripted events | `be3b6c65d8a2d1d46a47d7300da6ab5578cb48c6e2d1ae63b6bec121719b7704` |
| Precise browser events | `4de998af779520344f737821a3c2bc2a110db549cc000bc102222075521aba87` |
| Tested release host | `02db70509ce7b4d4da1ba5f4e5c09b3ca46ea7637a227ad3d8b493356c4ee663` |
| NTL pit/rejoin events after #190 rebase | `8baab971e81ac6ead53f493d02656c67e18f25828e7c892b1903b0c604bd6fc5` |
| Tested debug host after #190 rebase | `97bcb4a0f09ceb391c01ffe73d0bca3656b981171c5f36314f341e72ce579a9b` |
| NTL pit/rejoin events after #192 rebase | `f638b483567de9f978454712ca1044b353775fe6cfd3e2606d636be20ab7e3f5` |
| Tested final debug host | `6cfd596972093144e2c3a1fd0af88e44f0da2b0d53c7fc5b438428b3490458ff` |
