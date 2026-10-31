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
bracketed IPv6, with no URL scheme, credentials, path, query or whitespace. The
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
in-memory list intact. Writes atomically replace the record only after an
explicit Add/Remove. Official endpoints and all historical pings never persist;
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
size. Only these `ListRow` controls have text rasters and draws. Wheel, arrows,
Page Up/Down and Home/End navigate the bounded window; Tab wraps within the
modal.
Backdrop click and Escape close; blank panel space shields the backdrop.
Gameplay presses stay suspended, releases still propagate, and nickname focus
returns after the exit fade. Geometry, hit-testing and IME placement remain in
logical pixels while rendering uses the physical DPR.

Servers is one shared login icon link with an unboxed 14px Arial caption,
matching Change Skin's caption placement and .7/1/.5 normal/hover/press opacity.
Its shared admission/focus gate depends on picker availability, including offline
launch; Play and Quality retain their existing gates.

Modal materials use the shared theme with centre strips measured in logical pixels. Login
text is explicitly ordered below modal dimming and surfaces. Existing ceilings
remain 64 menu resources, 13 MiB and 512 menu draws. Synthetic tests cover 2,000
servers at DPR 1 through 5 over narrow, large and resized viewports, including
scrolling to both ends, exact numeric
selection, stale captures, storage failure and no Play on open.

From `client/`, with `CARGO_BUILD_JOBS=2`, run the core `server_directory` and
`server_picker` integration tests and app `server_picker` / `custom_servers`
tests. The finite `menu_specimens <new-external-directory> --picker-panel`
example emits art-free panel specimens using synthetic addresses and no
preference-file or network effects. The older finite
`menu_specimens <new-external-directory> --server-picker` example emits login,
picker and scrolled PNGs at 1x/1.5x/2x, plus the retained 960x540 to 960x720
logical resize at DPR 3 and resource counts. It uses authored
synthetic addresses and injected ping results; it opens no window or socket.
Full verification remains `npm run check`.

These checks establish synthetic behavior and offscreen rendering only. Live
discovery/play, physical input/IME, OS accessibility and owner appearance/feel
acceptance require separate verification. No official assets or source are
included.
