# Menu resources

`apps/wgpu-slice/src/menu.rs` prepares an immutable `MenuFrame` from the core's
`StartMenuView` and `MenuLayout`. The native login enables it with the PR2 editor for caret, selection and preedit.
Both browser clients use one semantic DOM overlay; their GPU/Canvas world
renderers do not draw login controls. Chooser/picker screens remain later work.
This layer owns no clock, hit test, focus or Play admission policy.

Native login layout, pointer input and IME candidate rectangles use logical window
pixels. `NativeLogin::prepare_host` prepares physical-resolution menu resources;
`SurfaceHost` paints them directly over the world blit, alongside the DOM-style HUD
and minimap. Nicknames remain in the capped world backing. The diagnostic
`MenuFrame::into_backing` mapping remains available for retained comparisons.
A terminal connecting attempt returns to login without reconnecting; a fresh
explicit Play is required. Gameplay/death returns remain separate work.

Overlay image origins round to whole physical pixels after text baseline and
alignment offsets. Controls and text rasterize at the window DPR and draw one
source pixel per physical pixel. Clipping keeps whole texels inside the CSS clip;
world nicknames retain their canvas placement. Pointer hit targets keep the core
CSS layout; caret and IME candidate rectangles share physical-pixel rounding.
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
a physical-resolution menu overlay. Logo art keeps its image aspect inside the
unchanged flow holder. Skin art uses idle/hover/press opacity 0.7/1/0.5. Plain skin icons, tips,
and a text logo fallback approximate the controls at gameplay size. The spinner uses
two vivid green/purple deforming ribbon arrows around a 1.25:1 oval. Its mean
phase is shared time/44 (about 2.21 seconds per turn), with the existing shared
scale/opacity unchanged. Each visible frame fills both nonzero-winding paths
into one premultiplied antialiased raster at 128 CSS pixels times its animated
scale and DPR, rounded to a physical pixel (at most 2048 pixels per side).
One 1:1 quad applies alpha to the complete canvas,
including folded/overlapping portions. A stable menu-material handle advances
revision when time or raster size changes; frozen frames reuse their image.
Hidden spinners allocate and rasterize nothing.
A static logo uses shared reveal/opacity timing; it does not reproduce animated
strokes. `TextCache` uses installed real Arial Bold (Windows) or DejaVu
Sans Bold (Linux). Labels fit core rectangles; no synthetic bold or copied art.

Both browser art loaders share reserved cosmetic-path rejection with the bounded
manifest parser. A leading UTF-8 BOM is retained and rejected, matching Rust.
The no-art placeholder uses `#7070B0`; native caret, selection and preedit underline
use solid materials without capsule shadows.

## Bounds and publication

- Menu has **64 replacing slots / 13 MiB**, shared by text and authored materials,
  isolated from nickname/HUD/minimap/panel allowances. Aggregate cache ceilings
  are **386 slots / 31 MiB**; these are ceilings, not eager allocations. The Menu
  allowance includes exact 5x control/shadow rasters and the two-run wordmark
  through 1.1x outgoing growth; the active-picture 64 MiB limit is unchanged.
- Menu has a **512-draw producer limit**. `frame::push_draw`/`reserve_draws` also
  enforce the final picture's 64 MiB Draw-record guard (262,144 draws). Menu
  reserves no world capacity. Active pictures retain 64 MiB/2,048-texture limits.
- Menu/HUD text uses exact DPR within the 2048px texture bound and its byte
  budgets; world nicknames retain the 128px font guard and backing-space
  diagnostics retain their 3x raster bound. Controls and
  decoded menu art allow up to 2048px per axis. Menu decoding alone admits that
  density; world cosmetic decoding keeps its 1024px bound. The picture/cosmetics
  pixel budgets and the two-resident-logo-frame limit still apply.
- External art shares the existing **16 MiB cosmetics allowance**. Authored
  menu art admission reserves the five procedural skin roles' 79,424 RGBA bytes;
  those mandatory roles also precede optional images at construction. Authored
  controls use the Menu group's own allowance, so a full cosmetics bank cannot
  block essential controls; unadmitted art uses authored fallbacks.
  Prepared pictures and staged replacements own additional bounded
  copies; active-cache ceilings do not describe total process memory.
- Changed/hidden Menu slots retire together in a staged cache. Materials, text
  and picture validation must succeed before cache publication. A failure leaves
  the prior cache intact. `MenuFrame::append_to` checks producer, extent and
  picture draw/pixel/texture admission before appending. Prepared resources stay
  immutable after resize, retirement and art replacement.

## One local art resolver

Native `--cosmetics-dir` and browser `/cosmetics/` use the same `Cosmetics`
semantic manifest and decoder. Without a manifest, optional paths are
`menu/logo.png`, `menu/skin.png`, `menu/builder.png`, `menu/cosmetic.png`,
`menu/quality-high.png` and `menu/quality-low.png`.
Missing/malformed art keeps layout and hit targets with authored fallbacks.
Builder/cosmetic entries are available for future chooser composition.

An optional **4,096-byte** UTF-8 `menu/manifest.txt` has this authored format:

```text
menu-art-v1
logo:0 1 menu/logo.png
logo:1 2 menu/logo-next.png
skin 1 menu/skin.png
builder 1 menu/builder.png
cosmetic 1 menu/cosmetic.png
quality-high 1 menu/quality-high.png
quality-low 1 menu/quality-low.png
```

Rows are role, positive u32 source revision and relative PNG/JPEG path. At most
37 distinct rows: `logo:0` through `logo:31`, `skin`, `builder`, `cosmetic`,
`quality-high`, `quality-low`. No server role is admitted yet.
Paths are at most 128 ASCII characters (letters, digits, `_`, `-`, `.`, `/`).
Empty, `.` and `..` components, absolute paths, URLs and duplicate roles/paths
are rejected. Native/served reads check canonical containment, including links.
Rust and browser loaders also reject menu paths already bound to an existing
cosmetic, preserving accessory and skin-image bindings when a manifest is invalid.
Images retain 4 MiB/file and 1,024px decode limits. Browser streaming reads cap
manifest/image bytes before Rust decode; redirects/credentials are disabled and
the source must have the page's origin. No official art or pack is included.

Only the first two declared logos load at startup. `replace_menu_art` admits
resolved bytes atomically and evicts the oldest other logo if needed: **at most
two frames** reside. GPU revisions increase across replacement/eviction; older
prepared pictures retain earlier pixels. Callers select semantic frame numbers.

## Offline specimens

Authored Rust/Node tests cover quotas, rejection/rollback, malformed/escaping
manifests, contained serving, two-frame residency and prepared retention. With
`CARGO_BUILD_JOBS=2` and a task-local target directory, from the repository root:

```sh
cargo test -p slither-wgpu-slice --locked --lib --test menu --test cosmetics --test panel_text
cargo run -p slither-wgpu-slice --locked --example menu_specimens -- <new-output-dir>
```

After generating WebGPU bindings, from `client/`, run
`node tools/menu-specimens.mjs <headless-browser-executable> <new-output-dir>`.
It uses a temporary loopback asset server, blocks outside requests and closes its
isolated browser; it never opens a game socket. Both hosts cover full controls
at DPR 1/1.25/2/3; native also covers synthetic replacement, missing art, resize
and retained pictures. Inspect recognizable states, no clipping and core edges
within 2 CSS px. No reference-pixel equality. These specimens do not establish
owner perceptual acceptance, physical IME/accessibility or real display pacing.

The browser DOM loader uses the same role/path grammar, bounded same-origin
streaming reads and PNG/JPEG dimensions checked before browser decode. Only
logo:0, skin and the two quality roles mount in login (at most four images and
16 spinner dots). Its art shares the 16 MiB decoded cosmetics allowance: the
WebGPU host loads world cosmetics first, then gives DOM art the remaining bytes,
without decoding unused menu art into the GPU bank. Native retains the existing
shared bank. Object URLs retire on generation teardown; stale async reads cannot
publish into a replacement document. Missing images use authored controls.

## Shared control adopter

The native [champion editor](menu-controls.md) paints through the same transaction
and physical overlay. Its 444px holder uses a bounded sliced atlas instead of a
wide texture. Uniform tall surfaces tile in both axes. Each tile charges the
existing producer cap; all material/text slots share the same Menu allowance.
Theme raster content enters cache keys, while position, sampled alpha and
horizontal text scroll do not. Login control migration remains a follow-up.
