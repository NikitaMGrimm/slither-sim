# Native skin studio

Change Skin opens an offline, modal built-in chooser from ordinary idle login,
including when no endpoint is configured. Previous/Next cycle 0–64 and skip 42
in both directions. No unlock grants exist in this delivery: local art and
`--accessory` cannot unlock studio choices. Builder and Cosmetics are deferred.
Saved skin 65 and existing custom patterns remain usable unchanged; either arrow
explicitly selects a built-in and retains the old custom bytes separately.

Save validates the complete draft, commits memory once, requests one best-effort
write and returns to login. Cancel, Escape and a matching outside press/release
discard it. Blur and resize cancel capture without discarding the draft. Tab
wraps Previous → Next → Cancel → Save; Enter/Space activates the focused button.
Left/Right navigate when an arrow button has focus. Entry and return use 230 ms
sampled from the caller clock; focus returns to Change Skin after return finishes.
Opening and saving have no profile, socket or send authority. Only explicit Play
takes an owned `AppearanceSelection`, also retained across profile acquisition.
The fixed endpoint and profile paths preserve the existing skin, marker order
and every trailing custom byte. Old built-in library entry points are retained.

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
for the run; omitted options do not erase saved choices.

The ASCII record is
`2:<nickname hex>:<b|c>:<builtin hex>:<custom base hex>:<custom hex>:<--|00..1f>`.
Inactive custom data and a future cosmetic preference remain separate from
unlock grants. Retained custom bytes keep opaque prefixes and raw tails unchanged,
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

Controls retain the shared 64-slot / 13 MiB / 512-draw Menu allowance, with
at least 24 slots / 5 MiB working headroom in the regression. The preview has a
separate 1,024-draw cap and uses direct logical quads, including rotation, rather
than a large render target. Combined controls/preview are bounded by 1,536 draws;
final picture admission retains 64 MiB / 2,048 textures and draw-record limits.
Fixed sampling makes the command bound independent of DPR. These are resource
limits, not GPU-memory or timing measurements.

Focused checks from `client/`: `cargo test --locked --test skin_studio` and
`cargo test --locked --manifest-path apps/wgpu-slice/Cargo.toml --test skin_studio
--test skin_studio_input --test login_native`. The Windows offscreen exporter is
`cargo run --locked --manifest-path apps/wgpu-slice/Cargo.toml --example
menu_specimens -- <new-external-directory> --studio`, with `CARGO_BUILD_JOBS=2`.
It produces 960×540 specimens at 1×/1.5×/2×, narrow layout, synthetic special-art
poses, a fixed-clock entry/return strip and a DPR1–5 numeric resource ledger.

All fixtures are authored and offline. Owner appearance/feel, real native
keyboard/IME/accessibility, GPU/display pacing, outside-server operation and
extension readiness are unverified. Builder editing and gated cosmetics remain
later deliveries; this PR does not establish their acceptance.
