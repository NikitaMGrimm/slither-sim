# Native server picker

The login's Servers icon opens an authored modal through the shared control
painter. The first open requests directory discovery and latency measurement in
official mode; it never grants Play, creates a game session, or sends login.
Subsequent opens reuse the process's completed directory and ping samples;
there is no open-count or timed refresh. An in-flight request is shared.
If acquisition fails or Play cancels it before completion, the next open retries
once. Failure never starts an automatic retry loop or discards completed samples,
including a successfully empty directory.
Custom endpoint mode keeps its explicit endpoint and performs no discovery.

`server_directory` owns bounded decoding and the existing automatic selection
policy. IPv4/IPv6 alternatives remain separate in the decoded directory.
`server_picker` deduplicates IDs by the fastest supplied cluster ping, then sorts
by latency and ID. Unknown pings sort last. Address ties prefer the higher
cluster, then socket address, making injected results deterministic. Active
servers and the saved selection appear in the list; numeric search can also
select an inactive directory entry.

Search strips non-digits and retains at most four digits. A changed value whose
numeric ID matches selects immediately and closes, including leading zeroes;
an empty changed value has numeric value zero. Search does not invent regions
or silently filter the directory. Clicking the search field clears its text.

Automatic (best ping) is the default. A separate versioned preference records
only the chosen ID (`1:<id>`), or `1:auto`, like the independent quality
preference. The Windows host reads/writes `SlitherClient/server-choice.txt`
beneath its local application data directory. Reads are capped at 32 bytes;
missing, malformed or unreadable storage restores automatic. A failed write
leaves the in-memory choice intact. No endpoints or historical pings persist.
An explicit CLI server wins until the user chooses a row or Automatic.

Play still acquires a fresh profile through `official::prepare`. A saved ID is
resolved against the retained directory using measured pings, then passes through
`explicit_endpoint`; a missing ID reports an acquisition error rather than
playing elsewhere. Directory fetches and pings remain bounded, cancellable
native effects. Cold Play retains its directory/pings for later opens and Plays;
a Play racing picker acquisition waits for that same worker. Only the profile
page and script are reacquired per Play. The picker does not execute downloaded scripts.

The shared `ScrollList` admits six rows per window, independent of directory
size. Only these `ListRow` controls have text rasters and draws. Wheel, arrows,
Page Up/Down and Home/End navigate the bounded window; Tab wraps within the
modal. Directory replacement cancels old pointer captures and activations.
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
`server_picker` integration tests and the app `server_picker` test. The finite
`menu_specimens <new-external-directory> --server-picker` example emits login,
picker and scrolled PNGs at 1x/1.5x/2x, plus the retained 960x540 to 960x720
logical resize at DPR 3 and resource counts. It uses authored
synthetic addresses and injected ping results; it opens no window or socket.
Full verification remains `npm run check`.

These checks establish synthetic behavior and offscreen rendering only. Live
discovery/play, physical input/IME, OS accessibility and owner appearance/feel
acceptance require separate verification. No official assets or source are
included.
