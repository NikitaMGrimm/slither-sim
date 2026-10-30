# Record announcements

The shared `AnnouncementState` retains one admitted `m` record and its
receipt-time score. Setup hides it without clearing content. The first accepted
playing leaderboard starts the existing HUD fade; replacement does not restart
it. Death holds that fade, return follows the existing return fraction, and the
completion sentinel remains -1. There is no separate timer or queue.

`project_announcement_text` produces an `AnnouncementTextView` independently of
fade. Its blocks contain literal text with shared `TextStyle` faces and separate
alpha, or explicit breaks. The quote block has a 5px following gap. The existing
plain `AnnouncementView.lines` is derived from those same blocks.

| Admitted fields | Content |
| --- | --- |
| Name and message | Quoted message; `- Name, today's longest`; `with a length of N` |
| Message only | Quoted message; `- today's longest`; `with a length of N` |
| Name only | `Today's longest was Name`; `with a length of N` |
| Neither | `Today's longest: N` |

The font stack is Arial, Helvetica Neue, Helvetica, sans-serif, white, with
13px italic base text. Quotes use 17px bold italic: message alpha 1, marks .5.
Attribution words/dash and length labels use .5; names use bold .75 and numeric
lengths bold .65. The neither branch uses bold .75 for its number. Canvas places
the retained fade on the holder and fragment alpha on each span, multiplying
them at draw time. It clamps the holder alpha to [0,1], including the sentinel.

These are source-derived specifications from the supplied private readable
reference: `19-websocket-and-protocol.js` record-message branch (1024–1080),
`07-background-and-pools.js` text filter (84–129), and
`06-login-and-hud.js` record panel (471–495). No reference implementation or
markup is included here. The tests and captures use authored synthetic input.

Name and message are independently suppressed after seven digits without an
intervening ASCII letter. Separators, controls and non-ASCII letters do not reset
that count. Otherwise Latin-1 bytes are retained, including whitespace and
controls; fields are neither trimmed nor capped at the outgoing editor's 140
units. Only positive scores replace content. A bare command can occupy the
protocol's full 1 MiB frame allowance, including its eight-byte header/name
prefix. Full admitted content remains in the owner. The visible text projection
drops NUL and normalizes CR/CRLF to LF, matching escaped HTML text parsing;
other whitespace is left to CSS-normal flow or shared panel normalization.

The Canvas reference uses native text nodes/spans and explicit `br` elements,
never packet-derived markup. A fixed node pool admits at most two blocks of six
semantic fragments, independent of message length. Its holder is fixed at
left 8, top 4, width 300, height 228 CSS pixels, clips overflow, collapses normal
whitespace and wraps overlong words. Full text is submitted to browser layout;
the browser clips the visible region. `AnnouncementTextBlock::panel_text` also
admits each full block into the [bounded panel primitives](panel-text.md), whose
repeated layout visits only a bounded visible prefix. Production `LocalGame`
enables tracking before ingress and passes the existing
step lifecycle to the shared owner. Native and browser WebGPU preserve source
fragment indices through bounded layout and multiply each alpha by the retained
fade at submission. CSS-normal line height includes the backend font box and
line gap; the quote
keeps its 5px gap. Native reads installed Windows font metrics. Browser WebGPU
measures one invisible DOM span per face/size/density, removes it immediately and
retains a bounded 16-entry metric cache; Canvas TextMetrics omits the line gap.
At DPR1 the quote occupies 20px, then 5px gap, with 15px attribution/length rows.
The world precedes announcements, then minimap and HUD.
Announcement slots are independent of HUD/nickname quotas. The 96-draw reserve
applies only while the captured view has present content, projected blocks and
positive opacity, including through the fade. The same snapshot supplies both
the reserve and submission. Before a record, while Setup hides it, at zero fade
and after the return sentinel, the reserve stays at HUD/minimap 140 (8052 world
draws); visible announcements reserve 236 (7956 world draws).
Setup callbacks, ordinary work and resize prepare immutable announcement draws
and resources at the same boundary as the world; retained/retried pictures never
consult later live text. Same-content fades reuse texture identities.

Rust tests cover wording/styles, admission, receipt order and lifecycle. Node
tests cover passive DOM submission, fixed node counts, stale-document/topology
fences and failures. The real Wasm smoke adds frozen rich records, maximum-size
commands, Setup/record/leaderboard ordering and atomic rejection. These checks
do not establish official-client visual acceptance.
