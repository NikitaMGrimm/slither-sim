# Menu resources

`apps/wgpu-slice/src/menu.rs` composes the page and world below the egui menus: a
`MenuFrame` carries only the page backdrop and the world crossfade cover. The menus'
controls, wordmark and spinner are [egui screens](login-egui.md). Both browser
clients use one semantic DOM overlay; their GPU/Canvas world renderers do not draw
login controls. This layer owns no clock, hit test, focus or Play admission policy.

Native login layout, pointer input and IME candidate rectangles use logical window
pixels. `SurfaceHost` paints egui directly over the world blit, alongside the
DOM-style HUD and minimap. Nicknames remain in the capped world backing.
A terminal connecting attempt returns to login without reconnecting; a fresh
explicit Play is required. Gameplay/death returns remain separate work.

Wordmark and spinner raster density is `min(window DPR, 5)` through one
`MAX_MENU_RASTER_DENSITY` constant. Windows scaling tops out at 500%. Higher DPR
keeps CSS layout, the real viewport scale and hit/IME coordinates, while sampling
the 5x textures. World nicknames retain their canvas placement.
Menu art is CPU-filtered on premultiplied pixels with Catmull-Rom once per source
revision and physical destination size, then cached at that exact size.

Authored Play capsules use eased vertical gradients and dim the normal surface
and label together when disabled. Nickname uses one bordered purple surface with
a broad shadow; tips and nickname use Lucida Sans Unicode on Windows, while Play
keeps Arial Bold. The opaque `#161C22` page backdrop sits below the fading world;
the world clear is unchanged. `MenuFrame.backdrop` carries it separately from the
menu controls, retaining its texture in the menu resources. `append_to` places
it below every world draw, mapped to the capped world viewport when controls
use the physical overlay. A hidden login emits no backdrop, even while the
spinner remains. Hosts can take it for a separate world layer before composing
a physical-resolution menu overlay. The stroke wordmark keeps the 450x115 flow
holder as its vertical anchor. Skin art uses idle/hover/press opacity 0.7/1/0.5. Plain skin icons, tips,
and a stroke wordmark approximate the controls at gameplay size. The spinner uses
two vivid green/purple deforming ribbon arrows around a 1.25:1 oval. Its mean
phase is shared time/44 (about 2.21 seconds per turn), with the existing shared
scale/opacity unchanged. Each visible frame fills both nonzero-winding paths
into one premultiplied antialiased raster at 128 CSS pixels times its animated
scale and capped menu density, rounded to a physical pixel (at most 2048 pixels per side).
One quad applies alpha to the complete canvas,
including folded/overlapping portions. A stable menu-material handle advances
revision when time or raster size changes; frozen frames reuse their image.
Hidden spinners allocate and rasterize nothing.
The native logo uses the [authored swelling stroke alphabet](wordmark.md), which
settles to a cached raster and has no ongoing wiggle. `TextCache` uses installed real Arial Bold (Windows) or DejaVu
Sans Bold (Linux). Labels fit core rectangles; no synthetic bold or copied art.

Both browser art loaders share reserved cosmetic-path rejection with the bounded
manifest parser. A leading UTF-8 BOM is retained and rejected, matching Rust.
The no-art placeholder uses `#7070B0`.

## Bounds and publication

- `frame::push_draw`/`reserve_draws` enforce the final picture's 64 MiB Draw-record
  guard (262,144 draws). Active pictures retain 64 MiB/2,048-texture limits.
- HUD text uses exact DPR, within the 2048px texture bound and its byte budget;
  world nicknames retain the 128px font guard and backing-space diagnostics retain
  their 3x raster bound. Decoded menu art allows up to 2048px per axis. Menu
  decoding alone admits that density; world cosmetic decoding keeps its 1024px
  bound. The picture/cosmetics pixel budgets still apply.
- External art shares the existing **16 MiB cosmetics allowance**. Authored
  menu art admission reserves the five procedural skin roles' 79,424 RGBA bytes;
  those mandatory roles also precede optional images at construction. A full
  cosmetics bank cannot block essential controls; unadmitted art uses authored
  fallbacks.
  Prepared pictures and staged replacements own additional bounded
  copies; active-cache ceilings do not describe total process memory.
- `MenuFrame::append_to` checks extent and picture draw/pixel/texture admission
  before changing the picture. Prepared resources stay immutable after resize,
  retirement and art replacement.

## One local art resolver

Native `--cosmetics-dir` and browser `/cosmetics/` use the same `Cosmetics`
semantic manifest and decoder. Without a manifest, optional paths are
`menu/skin.png`, `menu/builder.png`, `menu/cosmetic.png`,
`menu/quality-high.png` and `menu/quality-low.png`.
Missing/malformed art keeps layout and hit targets with authored fallbacks.
Builder/cosmetic entries are available for future chooser composition.

An optional **4,096-byte** UTF-8 `menu/manifest.txt` has this authored format:

```text
menu-art-v1
skin 1 menu/skin.png
builder 1 menu/builder.png
cosmetic 1 menu/cosmetic.png
quality-high 1 menu/quality-high.png
quality-low 1 menu/quality-low.png
```

Rows are role, positive u32 source revision and relative PNG/JPEG path. At most
five native roles: `skin`, `builder`, `cosmetic`, `quality-high`, `quality-low`.
The shared manifest still validates up to 37 rows for the standalone browser
reference, including its legacy logo roles. Native ignores those rows rather
than loading their image bytes; the remaining icons in existing packs still work.
No server role is admitted yet.
Paths are at most 128 ASCII characters (letters, digits, `_`, `-`, `.`, `/`).
Empty, `.` and `..` components, absolute paths, URLs and duplicate roles/paths
are rejected. Native/served reads check canonical containment, including links.
Rust and browser loaders also reject menu paths already bound to an existing
cosmetic, preserving accessory and skin-image bindings when a manifest is invalid.
Images retain 4 MiB/file and 1,024px decode limits. Browser streaming reads cap
manifest/image bytes before Rust decode; redirects/credentials are disabled and
the source must have the page's origin. No official art or pack is included.

`replace_menu_art` admits the remaining icon bytes atomically; GPU revisions
increase across replacement, and prepared pictures retain earlier pixels.
Native has no bitmap logo role, frame selector or reveal helper. The standalone
DOM pages retain their existing plain-text/art logo as a known reference-host
difference; the future extension will reuse the native painter through WebGL2.

## Offline specimens

Authored Rust/Node tests cover quotas, rejection/rollback, malformed/escaping
manifests, contained serving, icon replacement and prepared retention. With
`CARGO_BUILD_JOBS=2` and a task-local target directory, from the repository root:

```sh
cargo test -p slither-wgpu-slice --locked --lib --test menu --test cosmetics --test panel_text
cargo run -p slither-wgpu-slice --locked --example menu_specimens -- <new-output-dir> --login
```

The example needs a mode: `--login`, `--settings`, `--server-picker`, `--studio`,
`--builder`, `--focus`, `--champion` or `--stroke`. Inspect recognizable states and
no clipping. No reference-pixel equality. These specimens do not establish owner
perceptual acceptance, physical IME/accessibility or real display pacing.

The browser DOM loader uses the same role/path grammar, bounded same-origin
streaming reads and PNG/JPEG dimensions checked before browser decode. Only
skin and the two quality roles mount natively. The standalone DOM reference also
mounts its legacy logo (at most four images and
16 spinner dots). Its art shares the 16 MiB decoded cosmetics allowance: the
WebGPU host loads world cosmetics first, then gives DOM art the remaining bytes,
without decoding unused menu art into the GPU bank. Native retains the existing
shared bank. Object URLs retire on generation teardown; stale async reads cannot
publish into a replacement document. Missing images use authored controls.

## Native focus outlines

Every native screen draws one keyboard-focus ring, `egui_theme::focus_ring`: two
logical pixels in the theme text colour. Core-panel controls (picker, studio, login
links) inset it inside the control bounds at the idle control radius, with
half-height corners for studio capsules. Theme pills (Play, Settings, in-game menu)
draw it 3 px outside, concentric with their round ends. Theme pills take the core
buttons' hover and press overlays, with the dark backdrop colour as the label on
the light hover; focus adds no tint. Their idle stops are shaded (green ×0.75,
purple ×0.9) so every enabled state keeps 4.5:1 label contrast against the whole
fill (`tests/menu_contrast.rs`). Pointer focus stays silent;
selection uses a separate marker. A studio round trip restores the opener's
focus modality and clears pointer feedback.

Server picker buttons use the same pill palette, retaining their existing font,
geometry and shadow. Disabled button and checkbox labels retain their text ink
while their surfaces still dim; only the disabled multiplier is removed from
text, so panel and section fades remain. Settled disabled text also meets 4.5:1
in `menu_contrast.rs`; transition fades are not settled control states. Native
menus currently have no combo boxes (quality and anti-aliasing use radio pills).

Run `cargo run --manifest-path apps/wgpu-slice/Cargo.toml --example menu_specimens
-- NEW_EXTERNAL_DIRECTORY --focus` from `client/`, with `CARGO_BUILD_JOBS=2`.
`--polish` renders the login, Settings with its Graphics group (opened, hovered,
Done focused), the server picker and the studio at 1080p DPR 1 and 4K DPR 2 in
Original and Native for visual review.
The fixed-clock sweep covers DPR 1–8 plus 1.25/1.5/1.75, wide/narrow/short layouts,
Settings, each studio section, section fades, return and early Builder entry.
Screenshots compose page, egui and the studio preview as the desktop host presents
them.
Generated solid images exercise art-loaded budgets and the egui studio's cosmetic
thumbnails (`*-ready.png`); all exported screenshots use only procedural geometry
and placeholders. It never reads an art directory or
connects to a server. The ledger records copied focus states, not physical
keyboard, accessibility or visible-window acceptance.
