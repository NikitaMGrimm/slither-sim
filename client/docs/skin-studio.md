# Native skin studio

Change Skin opens an offline, modal studio from ordinary idle login,
including when no endpoint is configured. Previous/Next cycle every ID 0–64,
including 42. None and cosmetic IDs 0–31 are all selectable by default. Built-in,
Builder and Cosmetics share one draft; there is no lock, grant, import or redemption flow.
Saved skin 65 and existing custom patterns remain usable unchanged; either arrow
explicitly selects a built-in and retains the old custom bytes separately.

Save validates the complete draft, commits memory once, requests one best-effort
write and returns to login. Cancel, Escape and a matching outside press/release
discard it. Blur and resize cancel capture without discarding the draft. Tab
wraps the active section tab → section controls → Cancel → Save; Enter/Space
activates the focused control. Left/Right on tabs switches sections, and on
built-in arrows cycles skins. Section changes clear capture and fade their
content over 180 ms without moving preview/footer. Entry and return use 230 ms
sampled from the caller clock; focus returns to Change Skin after return finishes.
Opening and saving have no profile, socket or send authority. Only explicit Play
takes an owned `AppearanceSelection`, also retained across profile acquisition.
The fixed endpoint and profile paths preserve the existing skin, marker order
and every trailing custom byte. Old built-in library entry points are retained.

## Builder

The six-column grid has 39 palette IDs, in order: 0–35, 37, 39, 41. Choose a
swatch, then Add to append one colour; adjacent equals form one run. Add is
disabled at 47 expanded colours. Arrows/Home/End move the grid's single roving
focus stop. The run strip shows four runs, with pointer arrows and keyboard
Left/Right/Home/End navigation. Counts describe whole runs. Remove deletes the
selected run; Recolour changes that run to the selected palette. Short/narrow
windows scroll the active editor, with keyboard focus revealed automatically.
All hit targets remain at least 28 logical pixels.

Opening Builder selects its custom draft for preview/Save. Switching sections
retains edits; explicitly choosing a built-in with Previous/Next turns custom
use off, while Save still retains any edited custom pattern as inactive data.
Reset clears only the builder colours. Leaving an empty Builder keeps the
draft's prior selection active. Save while still on an empty Builder restores the
previously committed custom bytes if any (including inactive retained custom),
otherwise the committed built-in. It never emits an empty custom override.

Runs are bounded by the existing 123-pair wire domain and are never expanded
for legacy editing. Long imported runs remain saveable unchanged; Remove and
Recolour can edit them while Add stays disabled at 47 or more colours. Edited
merged counts split at 255 and must fit the 255-byte wire limit. A changed,
non-empty Save samples two injected random bytes once, after validation, for
the header `[255,255,255,0,0,0,a,b]`, followed by positive count/palette pairs.
New 47-colour patterns occupy at most 102 bytes. No length byte or preview
spacer is appended. The preview uses the same normal palette mask and a
synthetic pose, with no palette-40 padding or serialized preview geometry.

Unchanged runs preserve every original byte, including zero/builder/opaque
prefixes and noncanonical adjacent runs. Retained payloads that cannot be
edited as complete normal runs still Save unchanged; their editor starts empty.
Repaints, restoration, Cancel and unchanged/empty Save consume no entropy.
The old three-palette development editor and Wasm preference API are unchanged.

## Cosmetics

The four-column, three-row window includes None and all 32 IDs. Arrows/Home/End
move one roving focus stop; Enter/Space selects without saving. Wheel and page
arrows move the bounded window, with stable item IDs and generation-bound capture.
The selected item stays independent of keyboard focus. Narrow windows retain
28px targets; very short windows display fewer grid rows, and very small tab
bars use short labels with complete semantic names.

Art comes only from the contained local `--cosmetics-dir` pack. No official art is
bundled. Existing manifest roles, revisions, relative-path checks and allowlists
remain unchanged. Native loading keeps base face/background/menu sources and the
committed accessory, rather than eagerly retaining all 32 full accessory images.
The studio retains at most twelve 128px thumbnails and one selected full source,
with one serial decode at a time. Only the twelve visible items plus one row of
metadata are projected (the fixed catalogue has 33 entries, below 1,024).
Off-window thumbnails are evicted; selected full art stays pinned. Readiness is
rechecked using contained file identities before publication. Replacement revisions
remain monotonic and earlier prepared pictures own immutable pixels.

The existing 4 MiB encoded-file, 1,024px world-image and 16 MiB decoded-art limits
remain. Retained plus staged pixel allocations are preflighted before decode;
thumbnails have a separate 4 MiB allowance. Missing, corrupt or over-budget art
shows a placeholder/status without changing the selected preference. None always
removes the local attachment. The preview shares only the requested ready source,
uses gameplay's own-head attachment geometry and suppresses it for single-image
faces. Its fit includes the complete rotated attachment.

Save commits this field with the whole draft once; Cancel discards it with all
other edits. The next explicit Play applies the committed local selection through
a narrow idle-login setter, including after an earlier game. It does not rerun
startup configuration or reset the bank/session. Neither cosmetic selection nor
source readiness changes any login byte. Explicitly choosing built-in 42 sends 42.

## Storage

Native storage is `%LOCALAPPDATA%/SlitherClient/menu.v2`, with a bounded read
(1,200 bytes plus one overflow witness), same-directory temporary file and atomic
replacement. Startup does not create directories, rewrite corrupt files or
write/delete v1. Absent v2 imports `menu.v1` using the existing v1 validation.
Present malformed v2 defaults atomically and is replaced only on Save. Unknown
versions and unreadable files disable persistence for the run. Typed read outcomes
keep I/O failures separate from corrupt contents. Failed writes preserve the memory
commit and display “Saved for this run” on return. Nickname is sampled from its
current login owner. Explicit `--nickname` / `--skin` override restored values
for the run; omitted options do not erase saved choices. Explicit `--accessory`
overrides the saved cosmetic for the run without a startup write.

The ASCII record is
`2:<nickname hex>:<b|c>:<builtin hex>:<custom base hex>:<custom hex>:<--|00..1f>`.
Inactive custom data and the local cosmetic selection are retained independently.
Cosmetic IDs never enter the wire snapshot or replace its `[0,255]` markers. Retained custom bytes keep opaque prefixes and raw tails unchanged,
subject to the wire's 255-byte bound. Header/run validation applies only to newly
authored patterns; v1 migration, the old development editor and inbound resolver
retain their existing rules.
No official browser keys are read. `slither-client.menu.v2` is reserved for a
future browser adapter; the development Canvas page continues using v1.

## Preview and validation

The live preview has 27 synthetic anchors and 79 fixed paint samples, a gentle
one-second wave, gaze and moving body shading. It borrows readiness-resolved
face/bulb sources and uses the existing body, face and antenna planners. Missing
art uses the same full-art fallback as gameplay and is labelled. It does not
create a miniature world or fabricate packets. The static Canvas preview is
preserved. Hidden/minimized time is excluded without catch-up; rejected pictures
do not advance antenna/random-tape state. Prepared resources remain owned across
replacement and retry. Preview assets have an independent handle namespace.

The preview has a 1,024-draw cap and uses direct logical quads, including
rotation, rather than a large render target. The desktop host presents it in
the top layer above egui; final picture admission retains 64 MiB / 2,048
textures and draw-record limits. Fixed sampling makes the command bound
independent of DPR. These are resource limits, not GPU-memory or timing
measurements.

The controls are egui widgets (`skin_studio_egui.rs`) in the login's egui pass.
They paint the core `StudioState::panel` layout as it is: the same rectangles,
scroll windows, section fade and entry/return alpha, so the browser and native
hosts still share one state and layout. Swatches, cells, tabs and actions use
the theme capsule gradient and compact shadow. Hover and press are instant, as
on the other egui screens; the keyboard focus ring shows only in keyboard
modality. A press focuses its control, as the old panel input did. Wheel and
key handling stay in the core: arrows, Home/End and Left/Right cycling on
Previous/Next go through `StudioState::key_navigation`, once per physical key press
(egui-winit reports native auto-repeats as fresh presses). A wheel scroll or a
cancel of the touch that owns the pointer drops the held press without a click;
another touch's cancel leaves it alone.
Cosmetic thumbnails upload once per handle and revision as egui textures and
are dropped when evicted or the studio closes. The old toolkit no longer
paints studio controls and rejects them.

Focused checks from `client/`: `cargo test --locked --test skin_studio` and
`cargo test --locked --manifest-path apps/wgpu-slice/Cargo.toml --test skin_studio
--test skin_studio_input --test skin_builder --test studio_cosmetics
--test login_native`. The Windows offscreen exporter is
`cargo run --locked --manifest-path apps/wgpu-slice/Cargo.toml --example
menu_specimens -- <new-external-directory> --studio`, with `CARGO_BUILD_JOBS=2`.
It composes page, egui and preview as the desktop host does and produces
960×540 specimens at 1×/1.5×/2×, narrow layout, synthetic special-art poses, a
fixed-clock entry/return strip, entry over the settled login and a DPR1–5
ledger of egui shapes/textures and preview resources. Use `--builder` instead
for the edit/reset/47-colour/save/restore strips at 1×/1.5×/2×, responsive
editor specimens and builder DPR1–5 ledger, and `--focus` for keyboard focus in
every section, section fades and cosmetic thumbnails from procedural art.

All fixtures are authored and offline. Owner appearance/feel, real native
keyboard/IME/accessibility, GPU/display pacing, outside-server operation and
extension readiness are unverified.
Run `--test studio_cosmetics` for the cosmetic-only byte, preview, setter,
control and quota regressions.
