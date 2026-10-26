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

## Local follow-up: can this browser client play protocol 19?

The owner subsequently authorized a cheaper **local-only** experiment before
deciding about another encoder. The official-collection conclusions above are
retained as the original decision; no additional official or third-party
connections were made. This follow-up used base `4022581` and the same unpacked
extension, read as data outside sandboxed Chromium.

### Challenge parser and the official-format conflict

Static inspection found one incoming game-WebSocket challenge route. Every
`6` command goes to the same decoder, regardless of request variant or server
classification. After decoding the shifted letter pairs, that decoder takes
the **first nonempty single-quoted substring** and copies its first 24 character
codes into a 24-byte array. It does not require a particular JavaScript template
or evaluate that source. For uppercase ASCII letters, subtract 65 from each
literal byte; initialize the rotation to 2 plus the first value, write
`65 + ((value + rotation) % 26)`, then increase the rotation by `3 + value`.
Lowercase uses base 97; punctuation uses JavaScript's signed remainder and
byte-array coercion. Prefix/suffix syntax is not validated by this client.

An original, synthetic source accepted in both local runs was:

```text
var a = 'ABCDEFGHIJKLMNOPQRSTUVWX';
```

It was sent using the same +23/+17 nibble encoding as the other challenge.
The actual returned 24 bytes were `CGLRYGPZKWJXMCTLEYTPMKJJ`
(`43474c525947505a4b574a584d43544c455954504d4b4a4a`). This is a
**client-derived local test input**, not an observed official profile.

The captured official format uses double quotes and 27 characters. Apostrophes
inside its random literal contents do not make it compatible: in attempt 2,
this decoder's first single-quoted match would be the unrelated 17-character
substring `";a+="<1VMX#Wyqr}`. It still constructs a 24-byte response, rather
than the independently recorded accepted 27-byte reply. A source without a
single-quoted match throws, as observed in the earlier loopback diagnostic.
No alternate 27-byte decoder or request/server-type bypass was found. This
package is therefore incompatible with the official challenge exchange retained
here; the assumption that it currently works on that official path is not
supported. No new official-server run was made to test how that server would
reject a different reply.

### Scratch patch and browser result

An **uncommitted** 48-line `server.rs` patch on a scratch branch answered
`[99]`, `[99, 3]` and `[99, 4]` with that synthetic challenge, admitted its
reply, and parsed selector-11/12 legacy login offsets. Only `[99, 4]` was
exercised by the real client. Stock challenge handling and all output encoding
were left on their normal paths, including **setup version 19**. The scratch
patch was saved outside Git and removed after the failed trial. No encoder,
broadcast, spawn or CI source was edited.

Two fresh persistent Chromium profiles loaded the actual extension with
sandboxing enabled, loopback mappings for the two game-domain names, all other
names failing resolution, the local deny proxy and request/WebSocket
allowlists. The local static shell was unchanged. Host and browser inherited a
two-logical-CPU affinity mask; Cargo used one build job and one Rayon thread.
No packet rewriting or alternate gameplay encoding was used in these runs.

| Check | Observed result |
| --- | --- |
| Challenge/login | Both runs sent `[99, 4]`, the expected 24-byte reply, then `730c07084c6f63616c4d6f6400ff`: selector 12, skin 7, eight-byte controlled nickname, empty custom skin. No optional player-ID message. |
| Setup | Read as 19; the client set its playing flag. |
| Nickname/skin | Host's own appearance contained the correct nickname and skin 7; nickname appeared in the leaderboard. The client never constructed its own snake, so no skin was rendered. |
| Rendering | Empty arena, no snake or food, displayed length 0. No JavaScript page exceptions occurred; blocked external requests generated expected network errors. |
| Steering/boost | Mouse steering and Space down/up produced no heading or boost packets; only pings followed login. Own snake remained null throughout. |
| Death/Play Again | In the longer run, the host emitted game-over and own removal in a bundle. The client ignored it, retained its playing flag and never presented Play Again. |

The first self/food message starts `0000003373000098`: elapsed header `0000`,
then the two-byte length `0033`, then `s`. The browser dispatcher reads the
single code at frame byte 2, sees zero, and ignores the entire bundle. This
alone prevents play. The longer run's received frame 80 (zero-based among
`rawReceived` entries) starts `00b600027600000473000001`: it bundles `v0`
and own removal, which are likewise ignored. The longer run retained 115
received frames, including 11 bundles; all 64 sent messages were the three
initial messages and 61 pings.

### Static estimate for legacy/protocol-13 output

Reading the setup version does not select a complete protocol-19 parser in
this package. Its relevant version checks concern old appearance fields,
pre-version-3 anchors and minimap format. The following differences remain
even if frame bundling were disabled; these are source comparisons, not a
validated implementation specification:

| Family | Existing host output versus client parser | Owning encoder entry points |
| --- | --- | --- |
| Framing | Host groups logical packets behind lengths; client expects one command after the elapsed header. | `frame`, plus `simulation/connection.rs` grouping/emission |
| Setup | Host advertises 19; a protocol-13 path would advertise its actual layout. Changing that byte alone does not fix the other families. | `setup` |
| Appearance body | Host ends body anchors with a 16-bit direction; client consumes all remaining bytes as coordinate deltas. | `appearance` |
| Motion | Host omits self IDs and can send own `d`/`7` forms; client always consumes a two-byte snake ID and dispatches only the older `e`/`E`/`3`/`4`/`5` forms. | `motion` (and ID selection) |
| Body/growth | Host sends angle-based `g`/`G`/`n`/`N`, or `=`/`+` absolute fallbacks; client expects IDs plus absolute coordinates or relative coordinate bytes, and has no `=`/`+` dispatch. | `anchor`, `anchor_at` |
| Food | Host listings use a sector header plus four-byte entries; client expects repeated six-byte color/absolute-position/size entries. Birth/removal keys also differ, and host pickup `<` has no handler. | `food_key`, `food_sector`, `food_add`, `food_remove` |

This is a **medium, cross-cutting change**: roughly ten current encoder entry
points spanning about 200 existing production lines (around a third of the
pre-test encoder module), plus protocol selection and framing in the server/
connection path, and integration in snake/food delivery and cache reuse. A
planning estimate is several hundred new/changed Rust lines across roughly
five source modules, with separate mixed-protocol, stock-byte-parity and browser
regressions. This is an impact estimate, not a completed design or measured
implementation cost. Changing cache/profile selection must be coordinated with
the capacity owner; none of that hot path was edited here.

Basic mass/tail/removal, radius, pong/game-over, leaderboard and session field
layouts appear reusable. The high-resolution `M`/`V` minimap structure also
appears compatible with the client's version-at-least-13 branch. Prey and
optional report paths still require a complete audit and browser coverage; this
estimate does not certify them. There is no basis to build the whole protocol
from only the two short official sessions or this failed local trial.

### Local provenance and disposition

All local raw events, screenshots, harnesses and the scratch patch remain
outside Git. Source and selected-artifact SHA-256 values:

| Local artifact | SHA-256 |
| --- | --- |
| Unpacked main client (read-only) | `f7ecd6f9b577dd2343c8d390c1570deb54708ffe138ca7bdd599f58d260f0eb8` |
| Final local browser harness | `568697290b05d561689b0330247f7ea4e8c0b807a3e9d49ceba6f8996db9823a` |
| Scratch patch | `b913941e368d216a5e1aa149854994d531dc16540849454dee0d3dba3dded82f` |
| First run events | `f54d09c2874952cf49be18546accc38ad0525db5027132774ed39c9fd2446dc1` |
| Longer/death run events | `50c3160206a6d6cf99b8e697269903b866bee7081065565a549dff64c69dc485` |

The final harness adds a ten-second death observation to the first run's
interaction sequence; its digest identifies the longer run's harness. The
deny proxy saw locally aborted requests and one harmless connection-abort while
returning a local 403; it made no upstream connections. The prototype was
compiled for the experiment, then its source was restored exactly. The PR stays
evidence-only. No claim of playable protocol-19 compatibility or implemented
protocol-13 support is made.
