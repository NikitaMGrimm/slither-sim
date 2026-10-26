# Legacy browser login: protocol-version stop

## Question and decision

Does accepting an older browser login layout let a protocol-19 host serve some
browser mods without changing its outgoing protocol?

**Stop before implementation.** One official-listed server accepted `[99, 4]`,
the existing stock-format 27-byte challenge reply and a synthetic selector-12
login, then advertised **protocol 13**. SlitherSim emits protocol 19. The owner
required a stop on a different setup version or gameplay encoding. No host
behavior was changed, and no compatibility claim follows from this experiment.

## Method and attempt budget

Collection was explicitly authorized on 2026-10-26: one official endpoint, at
most two sequential sessions, each at most 60 seconds. An original Python
standard-library client reserved each attempt before opening TCP, with no
automatic retries, discovery pings or additional connections. It executed no
downloaded JavaScript and contacted no mod service or community server.

The endpoint was the first active IPv4 entry in the current official HTTPS
discovery list. The listing was decoded as data; the chosen endpoint stays
private. Game WebSockets used the official HTTP Origin. Plain WebSocket traffic
and listing membership do not cryptographically authenticate the game server.

The first frozen plan expected a single-quoted 24-character challenge literal,
based on prior static inspection of the browser mod. Attempt 1 instead received
the existing two-literal 27-character stock source. The client failed closed
before reply/login after 0.373 seconds. That failure remains in the evidence.

Before attempt 2, an explicit plan amendment used the observed source with the
repository's documented 27-byte reply transform, followed by the same synthetic
legacy login. It retained approximately eight seconds of gameplay receipt and
sent pings, closing after 8.394 seconds. Both attempts used the same endpoint,
one connection at a time. The two-session budget is exhausted; `[99]`,
`[99, 3]` and an additional stock session were not collected.

## Observations

| Exchange | Observed bytes or format |
| --- | --- |
| Pre-login request, both attempts | `63 04` (binary `[99, 4]`) |
| Challenge, both attempts | Two-byte elapsed header, `36` (`6`), 184 alphabetic encoded bytes; decoded two-double-quoted-literal template of 5 + 22 characters and a 27-iteration copy loop |
| Accepted reply, attempt 2 | `3830314d31303e3d33343434706a656b4f466b7847565a684e7246` (27 bytes) |
| Synthetic login, attempt 2 | `730c070b436f6d70617450726f626500ff` |
| Setup | 63-byte elapsed-framed message: 61 command bytes, command byte 23 = `0d` (13) |
| Selected setup prefix | `61007f2601ae012c006630021b002805780021001d01ae0d2a` |

The login decodes as `[115, 12, skin=7, nameLength=11, "CompatProbe", 0, 255]`,
with no custom-skin data. The raw archive also contains the own appearance
echoing that controlled nickname and skin. This is a scripted-client join,
not the real mod joining.

Within the selected first 25 command bytes, setup differs from the retained
[stock protocol-19 prefix](../../../tools/fidelity/fixtures/protocol19-setup.json)
only at the advertised version byte: 13 versus 19. The comparison is with that
historical observation, not a simultaneous stock session. Remaining setup
extensions and their possible differences have not been compared.

The second raw archive has 632 records: 612 received binary messages, 19 sent
binary messages and one sent close. Received command codes include setup,
appearances, radius, food, pongs, motion/body updates, mass, minimap and
leaderboard reports. Selected `p`, `3` and `G` frames are retained without
protocol-13 semantic decoding. Recognizing a command byte does not establish
that its layout agrees with protocol 19. The advertised version alone meets
the owner's stop condition.

The real mod's prior loopback capture sent `[99, 4]` and was rejected by the
host before login. In a diagnostic request-only rewrite, it threw while parsing
the host challenge: its parser expects a single-quoted literal and a 24-byte
reply. The official evidence here does not justify inventing such a host
profile. No after-fix browser run, steering, boost, death or Play Again
acceptance is claimed. Optional player-ID traffic was not sent by this scripted
client, and no official acceptance evidence for it was collected.

## Selected fixture and provenance

[`client-legacy-login.json`](../../../tests/fixtures/client-legacy-login.json)
owns archive SHA-256 values, zero-based record indices, plan/collector hashes,
the official discovery/client source hashes and the selection rule. It retains
both challenge exchanges, the accepted reply and synthetic login, the setup
prefix through body spacing, and the first `p`/`3`/`G` frames. Setup bytes from
the server-number offset onward and all other-player appearances are excluded.
The observed wire version is pinned to 13; existing version-19 fixtures remain
unchanged. Raw captures and original/amended plans and collectors are
**local-only, outside Git**, under the capture bundle `official-evidence`.
Hashes establish byte identity, not server provenance or public retrievability.

Collection and reference source revision: `fbefe17`. The new offline regressions
cross-check the accepted reply against the existing restricted challenge solver,
check the literal version tag against setup bytes and the historical stock
prefix, and require the current decoder to reject protocol 13 without mutating
client state. They establish neither a complete protocol-13 decoder nor browser
compatibility. No held-out mechanics validation applies: no mechanics were
fitted or changed.

## Reproduction and limits

With local access to the retained bundle, verify the two raw archive hashes from
the fixture, select the named record indices, and take command bytes `0..25`
after the setup frame's two-byte header. Recompute the reply using
`src/client/challenge.rs`; compare setup byte 23 directly, without relabeling it
or decoding gameplay as version 19. The selected fixture is self-contained for
the offline tests; repeating the live collection needs fresh authorization
because this attempt budget is exhausted.

One endpoint and one completed gameplay session are exploratory compatibility
evidence, not a survey of official servers. The response contradicts the
assumed 24-character challenge profile and leaves the real mod's official-server
compatibility unexplained. Supporting another protocol, or establishing a
different compatible official exchange, requires a separate decision before
host implementation. No optional identity service or protocol-13 output was
implemented.
