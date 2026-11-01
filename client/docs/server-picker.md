# Native server picker

The login's Servers icon opens a shared modal with a `host:port` field, Add,
saved Custom entries and an explicit Load official action. A no-flag launch
stays offline: opening the picker, restoring a saved official ID and adding or
choosing custom entries do not fetch the official website. Choose a custom row
to enable direct Play with the fixed login. Removing the selected custom entry
leaves Play unconfigured until another choice is made.

Load official enters official context and requests discovery. `--official`
opts into that context at login, starting directory discovery and latency
measurement in the background. Opening the picker then joins that work;
it never grants Play, creates a game session, or sends login. Choosing a Custom
row leaves official context and cancels unfinished official work. Direct Play
never invokes official profile acquisition, directory fetching or probing.
Subsequent opens reuse the process's completed directory and ping samples;
there is no open-count or timed refresh. An in-flight request is shared.
If acquisition fails or Play cancels it before completion, the next open retries
once. Failure never starts an automatic retry loop or discards completed samples,
including a successfully empty directory.
Direct `--server host:port` keeps its explicit server and performs no discovery
or website fetch. It uses the fixed login and builds `ws://host:port/slither`
internally; IPv6 authorities use brackets. `--official --server host:port` keeps
the explicit server but fetches fresh login values on Play.

Custom addresses use the same admission as `--server`: a host and nonzero port,
bracketed IPv6, with no URL scheme, credentials, path, query or internal whitespace.
The field trims leading/trailing whitespace before validation, including pasted newlines. The
field retains at most 512 UTF-16 units; canonical saved authorities are bounded
to 512 bytes. Add validates before mutation, deduplicates canonical authorities
and caps the list at 32 entries. Remove applies only to a visible custom row.
Custom entries form a group above official rows and have no automatic pings.
The shared six-server `ScrollList` bounds both groups, with headers outside the
navigable row indices. Keyboard navigation and mouse selection use the ordinary
editor path; only keyboard focus paints focus decoration. Mouse clicks retain
text editing/caret behavior without a focus outline.

`server_directory` owns bounded decoding and the existing automatic selection
policy. IPv4/IPv6 alternatives remain separate in the decoded directory.
`server_picker` deduplicates IDs by the fastest supplied cluster ping, then sorts
by latency and ID. Unknown pings sort last. Address ties prefer the higher
cluster, then socket address, making injected results deterministic. Active
servers and the saved selection appear in the list; numeric search can also
select an inactive directory entry.

The worker publishes decoded servers before probing and coalesces incremental
cluster minima into one bounded in-memory progress slot. The host polls it at
50 ms while discovery or Play preparation is running, including after Play
takes the worker. Partial results are for display only: automatic Play still
waits for the unchanged ping sweep and selects from its completed samples.
Partial results never become the completed cache or suppress a retry after
cancellation/failure. Fetch limits, cancellation and probe deadlines are unchanged.
Cold Play after a failed or cancelled startup starts that same streaming worker,
even without reopening the picker. Failure/cancellation clears provisional rows
and their captures; completed samples remain reusable. Retries still require an
explicit Play or a new picker open, with no automatic retry loop.

While the picker is visible (including its exit fade), progress and completion
update latency text in place and retain row order and visible membership. They
keep the same control generation, focus, pointer capture and scroll window.
The next open applies the latest deduplication and latency order, with unknown
pings last. The initial decoded list can appear while search is focused without
resetting its editor. Actual directory replacement still cancels stale captures.

Search strips non-digits and retains at most four digits. A changed value whose
numeric ID matches selects immediately and closes, including leading zeroes;
an empty changed value has numeric value zero. Search does not invent regions
or silently filter the directory. Clicking the search field deliberately clears
its text on every click, including when already focused, matching the reference.

Official Automatic (best ping) remains the default choice, but does not opt a
no-flag launch into official context. `SlitherClient/server-choice.txt` beneath
local application data still loads `1:<id>` and `1:auto`; a custom choice uses
`2:custom:<authority>`. Reads are capped at 544 bytes and custom authorities are
revalidated. Missing, malformed or unreadable choice storage restores Automatic.
A failed write leaves the in-memory choice intact.

The adjacent `custom-servers.txt` stores version `1` on its first line and one
canonical user-entered authority per following line. Reads are capped at 16,418
bytes; malformed, oversized, duplicate or unsupported records leave the current
in-memory list intact. Each explicit Add/Remove re-reads that bounded record,
merges the operation (deduplicated Add or addressed Remove), keeps the 32-entry
cap and atomically writes it back. An exclusive interprocess lock on the adjacent
`custom-servers.lock` covers that whole read–apply–replace. The empty sidecar stays
on disk; the operation releases its lock on return. Lock contention retries for
at most 200 ms. If locking fails or times out, the edit stays in memory only and
the picker shows a status; it never waits indefinitely for the lock.
The picker refreshes from the merged list,
preserving edits saved by other instances since startup. Failed or malformed
reads use the current in-memory list instead. Official endpoints and all historical pings never persist;
the user's own custom authorities and custom choice are user data and may
persist. An explicit CLI server wins until a picker choice/context action.
`--official` overrides a restored custom choice with official Automatic for that
run; this startup override does not rewrite the saved choice.

Official Play still acquires a fresh profile through its preparation worker. A saved ID is
resolved against the retained directory using measured pings, then passes through
`explicit_endpoint`; a missing ID reports an acquisition error rather than
playing elsewhere. Directory fetches and pings remain bounded, cancellable
native effects. Cold Play retains its directory/pings for later opens and Plays;
a Play racing picker acquisition waits for that same worker. Only the profile
page and script are reacquired per Play. The picker does not execute downloaded scripts.
Completed directory/ping samples are published even if that Play later fails
profile acquisition or selected-ID resolution; reopening reuses those samples.
An explicit or forced endpoint leaves pending discovery with the picker; Play
does not wait on it, and its success or failure is handled independently.

The shared `ScrollList` admits six server rows per window, independent of directory
size. Only these `ListRow` controls become widgets. Wheel, arrows,
Page Up/Down and Home/End navigate the bounded window; Tab wraps within the
modal. The address field keeps Home/End and horizontal caret keys; numeric
search keeps Home/End. Other arrows never move focus between buttons.
Backdrop click and Escape close; blank panel space shields the backdrop.
Gameplay presses stay suspended, releases still propagate, and nickname focus
returns after the exit fade. Geometry, hit-testing and IME placement remain in
logical pixels while rendering uses the physical DPR.

Servers is one shared login icon link with an unboxed 14px Arial caption,
matching Change Skin's caption placement and .7/1/.5 normal/hover/press opacity.
Its shared admission/focus gate depends on picker availability, including offline
launch; Play and Quality retain their existing gates.

## Native egui screen

`server_picker_egui.rs` draws the modal in the shared `NativeEguiHost`
([native login](login-egui.md)). `ServerPicker::panel` still owns placement,
scale, wording, the row window and tab order; each egui widget uses its resolved
control rectangle, so painting and hit-testing share the core geometry. Login
paints below the modal without input. A new panel generation (open, custom edit,
directory replacement) focuses the address field and cancels a held pointer press,
and a focused row that leaves the window loses focus before the next key, as
with the shared panel input. The opener's keyboard or
pointer modality decides whether focus is painted.

Field text follows the same rules: numeric search keeps four ASCII digits and the
address keeps 512 UTF-16 units with controls replaced by spaces. Inserted text is
normalized first, then the whole value; carets are clamped in chars. Enter in the
address adds it and keeps focus; held Enter repeats are ignored. Wheel events are
queued into row scrolling without an egui pass per event; pixel deltas use 46
physical pixels per row. IME follows the login rules: adjacent preedits coalesce
until redraw, composition owns Enter and Tab, and cancellation (including native
IME Disabled, Escape, window or field focus loss, closing and an empty preedit)
restores the field text, caret and copied undo history. Provisional text never
selects a server or reaches Add.
egui-winit owns IME admission; opening and closing the picker emit no legacy IME
effects. While open, the picker owns Enter and Escape even if an old socket is
still closing.

From `client/`, with `CARGO_BUILD_JOBS=2`, run the core `server_directory` and
`server_picker` integration tests and app `server_picker` / `custom_servers`
tests. The finite `menu_specimens <new-external-directory> --server-picker`
example emits before/after PNGs for official, scrolled, DPR 3 resize, custom,
pointer/keyboard focus, invalid and mixed states. The before side is the retained
toolkit painter fed the same panel, field values and focus; it is not a desktop
capture. It uses authored synthetic addresses and injected ping results and opens
no window, socket or preference file. Full verification remains `npm run check`.

These checks establish synthetic behavior and offscreen rendering only. Live
discovery/play, physical input/IME, OS accessibility and owner appearance/feel
acceptance require separate verification. No official assets or source are
included.
