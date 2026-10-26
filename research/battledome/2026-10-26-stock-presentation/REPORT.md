# Stock Battledome presentation: offline marker visibility

This is a source-backed presentation regression, not an official-server
measurement or browser pixel acceptance. No listener, relay or game connection
is used. The baseline is `f8bcf5a6da256e4f3c02727f1d23172159249a32`.

## Failure and correction

The ring reaches decorated connections. Its lifetime during shrink is the
problem: main replaces radial food-grid cells repeatedly and emits ordinary `f`
births. The stock client starts food at `fr=0`, advances `vfr=elapsed_ms/8`, and
adds `rsp*vfr/150` to `fr`. Initial `F` listings have `rsp=3`; ordinary `f` births
have `rsp=1`. With normal, unlagged client advancement they take 400 and 1200 ms
respectively to reach full size. Short-lived replacements restart that growth.
This explains a nearly invisible moving ring without packet loss.

The fix sends decoration additions as rapid `F` listings, refreshes a shrinking
ring at most every 500 simulation ms, and retains its preceding generation for
500 ms. A mature generation remains while replacements grow. Growth replaces
immediately so old markers do not remain behind the expanded pit. After settling,
the prior generation retires. The outward trail is about one second of pit
shrink plus the existing 3-unit offset and grid rounding. Physical food,
admission and shrink physics are unchanged.

## Source and reproduction

The separately retained stock `game1107249518.js` has SHA-256
`5ad8aa8f06d1e71bad2945001bab077f8d4d279c093a046d19b6f26ac6b62095`, matching
`tests/fixtures/client-presentation.json`. It is not redistributed. Run
`node research/battledome/2026-10-26-stock-presentation/stock-fade.mjs STOCK_CLIENT_JS`
to execute its actual `newFood` and food fade bodies with inert sprite references.
The probe has no DOM, rendering or network access. Its 20 ms frame samples give
normal/rapid completion at 1200/400 ms, allowing one sample for floating-point
rounding.

Run `cargo test --locked --lib stock_pit_markers -- --nocapture` for serialized
stock food delivery and the fade-lifetime regression. The same test body was
run on baseline and candidate; baseline fails because its minimum mature count
is zero. The regression uses 65 seats and one decorated connection, seed 7,
no physical food, and a synthetic viewport 100 units outside the live pit.
No physical-food opt-out is used. Existing gateway regressions check that ordinary
connections get decoration and the explicit physical-food header excludes it.

At 0.7 s a recorded radius update grows the wall to the stated size; the normal
23.4375-unit/s wall controller then shrinks it. At 3 s a recorded sample sets it
to 3584 to isolate settled delivery. This is a scripted settle, not a measurement
of the full shrink duration. Join times are 0, 1 and 3.5 s. Each connection
advances 715 ticks (5.005 simulation seconds). After a 0.6 s initial allowance,
count food IDs old enough to have `fr=1`. The table gives the minimum count in
each observed phase. `n/a` means the connection joined after shrink. Counts
describe the subscribed view, not the entire ring.

| Wall after growth | Join (s) | Shrink mature minimum, main | Shrink mature minimum, fix | Settled mature minimum, main | Settled mature minimum, fix | Final decoded markers, both |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 8960 | 0 | 0 | 141 | 0 | 141 | 149 |
| 8960 | 1 | 0 | 141 | 149 | 149 | 149 |
| 8960 | 3.5 | n/a | n/a | 149 | 149 | 149 |
| 17900 | 0 | 0 | 141 | 0 | 141 | 149 |
| 17900 | 1 | 0 | 141 | 149 | 149 | 149 |
| 17900 | 3.5 | n/a | n/a | 149 | 149 | 149 |

Every tick also compares the decoder's food count with the IDs tracked through
`F`, `f`, `b`, `c`, `<` and `w` packets, rejecting duplicates. Existing
growth/re-entry/reconnect tests check radius placement and sector delivery.
The ring still follows normal local sector interest; it is not a global ground
overlay visible from every spawn in a grown arena. No permanent post-settle loss
was reproduced: both versions finish with 149 decoded markers. Main can still
be briefly invisible after a radius change
because births into retained subscriptions use slow `f`.

All six scenarios and the failing baseline are reported here. No CPU timing or
performance claim is made. The tests establish delivery and nominal stock fade
lifetimes; they do not prove rendered visibility under browser lag, arbitrary
relay behavior or the owner's live view. Host bot policy, arena sizing and
simulation rules are outside this presentation regression.
