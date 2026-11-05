# Graphics quality and world anti-aliasing

Authored synthetic, offline world and native-menu controls on one Windows PC. No external artwork, window, socket, game server or capture was used. The baseline is `e066bc33`; selectable policies and their rationale are in [Settings](../../../client/docs/settings.md).

Low reduced the integrated GPU's Native/AA-Auto normal-view queue-span median from **5.196 to 1.375 ms**. CPU picture preparation increased from 0.343 to 0.494 ms: Low uses the canonical food planner while Auto can retain compact food batches. These are warmed offscreen world costs, not gameplay FPS or a refresh-budget acceptance.

## Default controls

- Auto quality maps to the unchanged requested High state, including legacy degradation/recovery. Explicit Low shadow cuts do not apply to degraded Auto. The CPU regression `graphics_settings` fences that distinction.
- Auto AA retains the inclusive 64 MiB Native policy: 1080p Native uses four samples; 4K Native uses one. Explicit 2/4 both rendered at their requested count on both DX12 adapters.
- The complete 16-cell Vulkan matrix matched decoded RGBA bytes exactly: two adapters × Original/Native × 1080p/4K × normal/0.065 zoom. All eight AMD DX12 cells also matched.
- All **32** DX12/Vulkan comparisons matched the complete canonical Frame and material Resource debug bytes, compressed losslessly outside Git.
- Four RTX DX12 MSAA4 cells are not pixel-repeatable even between independent runs of the unchanged main executable. The same cells vary in candidate repeats. Their pixel parity remains unqualified; no tolerance or changed expected pixels was accepted. RTX single-sample and other repeatable controls matched. This is a retained driver/backend limit, not evidence of a causal default change.

The final control devices request no timestamp features. The same authored driver/fixture was built against a clean main source snapshot and the candidate, sequentially cleaning only the two client packages in the task target directory to prevent cross-checkout Cargo artifact reuse. Timings use a separate diagnostic device with encoder timestamps.

## Measurement

- Fixture: 24 authored rival snakes plus the own snake, 2,400 food, procedural shaded materials, and an authored large arena. Spawn animation is settled. Camera/ingress are frozen; far zoom reaches factor 0.065 through the real wheel policy. No external accessories/art pack is loaded.
- DX12: NVIDIA GeForce RTX 5070 Ti at 3840×2160; integrated AMD Radeon Graphics at 1920×1080; DPR 1. Both world modes use the existing 1500×845 projection at these aspect ratios. Format is RGBA8 Unorm.
- Full preset × AA × world-mode × zoom matrix: 128 cells, five alternating forward/reverse rounds, 10 warmup + 40 measured pictures per cell/round. Exactly 25,600 successful measured pictures; no rejected/failed picture is counted as fast.
- Requested quality is initialized explicitly and frozen so automatic draw-count fallback cannot retarget a comparison. Auto and High begin with the same full picture; High disables later adaptation.
- CPU preparation is measured separately. GPU queue spans bracket all presenter submissions and include CPU supply gaps; waits, uploads completed before the span, readbacks, PNG encoding and logging are outside the corresponding timed region. HUD, menus, transport, simulation, display pacing and scanout are excluded.
- Each GPU invocation acquires the lane timing lock with purpose/start time. Final control reruns use atomic acquisition and token-checked cleanup. No owned lock was held near the 60-minute limit.

The timing run recorded 31 load samples: total CPU 3–51% (median 18%), with 0–3 Cargo/rustc processes present. No compiler was active at acquisition; other CPU builds started during the run. Configurations were interleaved, and per-round ranges below retain that variation. Full process/GPU load records remain local; no cross-hardware throughput claim is made.

Numbers below are milliseconds. Each span entry is **median / p95 [minimum–maximum round median]** over 200 samples. CPU entries are normal / far preparation medians. P95 uses sorted index floor(0.95N). Samples are the effective world count; overlays remain single-sample.

## RTX 5070 Ti, 4K, Original

| Preset | AA request | Samples | Normal span | Far span (0.065) | CPU preparation normal / far |
| --- | --- | ---: | ---: | ---: | ---: |
| Auto | Auto | 4 | 1.829 / 2.084 [1.808–1.902] | 2.984 / 3.302 [2.969–3.024] | 0.334 / 0.477 |
| Auto | Off | 1 | 1.811 / 2.396 [1.760–2.019] | 2.971 / 3.360 [2.920–3.041] | 0.336 / 0.478 |
| Auto | 2× | 2 | 1.805 / 1.987 [1.795–1.844] | 2.990 / 3.281 [2.958–3.054] | 0.331 / 0.477 |
| Auto | 4× | 4 | 1.805 / 2.011 [1.779–1.842] | 2.982 / 3.198 [2.955–2.997] | 0.332 / 0.478 |
| Low | Auto | 4 | 0.561 / 0.684 [0.544–0.594] | 0.745 / 0.855 [0.702–0.778] | 0.477 / 0.605 |
| Low | Off | 1 | 0.548 / 0.635 [0.531–0.586] | 0.699 / 0.825 [0.686–0.748] | 0.478 / 0.601 |
| Low | 2× | 2 | 0.558 / 0.672 [0.540–0.595] | 0.740 / 0.859 [0.702–0.757] | 0.477 / 0.605 |
| Low | 4× | 4 | 0.568 / 0.659 [0.545–0.608] | 0.723 / 0.837 [0.700–0.759] | 0.480 / 0.607 |
| Medium | Auto | 4 | 1.515 / 1.683 [1.458–1.536] | 2.529 / 2.696 [2.489–2.551] | 0.515 / 0.682 |
| Medium | Off | 1 | 1.492 / 1.702 [1.464–1.518] | 2.529 / 2.900 [2.467–2.562] | 0.517 / 0.687 |
| Medium | 2× | 2 | 1.516 / 1.710 [1.494–1.527] | 2.546 / 2.752 [2.492–2.567] | 0.517 / 0.682 |
| Medium | 4× | 4 | 1.547 / 1.853 [1.535–1.566] | 2.580 / 2.886 [2.532–2.639] | 0.518 / 0.687 |
| High | Auto | 4 | 1.894 / 2.229 [1.856–2.006] | 3.050 / 3.403 [3.021–3.153] | 0.339 / 0.486 |
| High | Off | 1 | 1.878 / 2.261 [1.865–1.900] | 3.048 / 3.420 [3.002–3.154] | 0.338 / 0.487 |
| High | 2× | 2 | 1.922 / 2.279 [1.858–2.005] | 3.088 / 3.463 [3.048–3.136] | 0.339 / 0.484 |
| High | 4× | 4 | 1.927 / 2.144 [1.895–1.960] | 3.103 / 3.423 [3.045–3.190] | 0.338 / 0.484 |

## RTX 5070 Ti, 4K, Native

| Preset | AA request | Samples | Normal span | Far span (0.065) | CPU preparation normal / far |
| --- | --- | ---: | ---: | ---: | ---: |
| Auto | Auto | 1 | 1.788 / 1.992 [1.767–1.870] | 2.956 / 3.439 [2.867–3.207] | 0.334 / 0.478 |
| Auto | Off | 1 | 1.795 / 2.072 [1.763–1.864] | 2.890 / 3.194 [2.862–2.948] | 0.331 / 0.476 |
| Auto | 2× | 2 | 1.880 / 2.086 [1.840–1.930] | 2.949 / 3.197 [2.942–2.961] | 0.333 / 0.476 |
| Auto | 4× | 4 | 1.877 / 2.075 [1.840–1.895] | 2.974 / 3.279 [2.935–3.106] | 0.331 / 0.475 |
| Low | Auto | 1 | 0.502 / 0.585 [0.496–0.529] | 0.648 / 0.744 [0.637–0.690] | 0.474 / 0.598 |
| Low | Off | 1 | 0.505 / 0.621 [0.497–0.536] | 0.650 / 0.736 [0.636–0.682] | 0.475 / 0.600 |
| Low | 2× | 2 | 0.577 / 0.673 [0.542–0.607] | 0.689 / 0.779 [0.673–0.732] | 0.483 / 0.600 |
| Low | 4× | 4 | 0.603 / 0.685 [0.567–0.615] | 0.718 / 0.800 [0.683–0.729] | 0.481 / 0.610 |
| Medium | Auto | 1 | 1.474 / 1.607 [1.433–1.489] | 2.494 / 2.790 [2.465–2.534] | 0.516 / 0.684 |
| Medium | Off | 1 | 1.486 / 1.673 [1.463–1.514] | 2.486 / 2.620 [2.463–2.512] | 0.519 / 0.686 |
| Medium | 2× | 2 | 1.541 / 1.688 [1.518–1.562] | 2.540 / 2.823 [2.511–2.581] | 0.518 / 0.686 |
| Medium | 4× | 4 | 1.562 / 1.820 [1.525–1.589] | 2.556 / 2.989 [2.518–2.609] | 0.521 / 0.687 |
| High | Auto | 1 | 1.845 / 2.162 [1.791–1.923] | 2.987 / 3.310 [2.929–3.126] | 0.339 / 0.484 |
| High | Off | 1 | 1.851 / 2.121 [1.813–1.924] | 3.000 / 3.267 [2.956–3.033] | 0.338 / 0.483 |
| High | 2× | 2 | 1.925 / 2.215 [1.888–1.970] | 3.161 / 3.695 [3.046–3.275] | 0.340 / 0.486 |
| High | 4× | 4 | 1.927 / 2.293 [1.906–1.951] | 3.140 / 3.429 [3.054–3.259] | 0.337 / 0.483 |

## AMD integrated GPU, 1080p, Original

| Preset | AA request | Samples | Normal span | Far span (0.065) | CPU preparation normal / far |
| --- | --- | ---: | ---: | ---: | ---: |
| Auto | Auto | 4 | 4.289 / 4.521 [4.271–4.345] | 5.104 / 6.418 [5.033–6.218] | 0.342 / 0.487 |
| Auto | Off | 1 | 3.365 / 3.693 [3.356–3.422] | 4.332 / 5.842 [4.305–5.445] | 0.341 / 0.484 |
| Auto | 2× | 2 | 3.903 / 4.391 [3.859–4.016] | 5.013 / 6.298 [4.857–6.195] | 0.342 / 0.488 |
| Auto | 4× | 4 | 4.285 / 4.549 [4.273–4.310] | 5.195 / 6.378 [5.006–6.282] | 0.342 / 0.486 |
| Low | Auto | 4 | 1.364 / 1.484 [1.352–1.375] | 1.271 / 1.642 [1.258–1.282] | 0.495 / 0.626 |
| Low | Off | 1 | 0.902 / 1.047 [0.891–0.908] | 0.952 / 1.164 [0.942–0.962] | 0.493 / 0.622 |
| Low | 2× | 2 | 1.300 / 1.464 [1.277–1.310] | 1.254 / 1.459 [1.244–1.266] | 0.495 / 0.625 |
| Low | 4× | 4 | 1.356 / 1.503 [1.338–1.366] | 1.263 / 1.510 [1.247–1.276] | 0.494 / 0.626 |
| Medium | Auto | 4 | 3.193 / 4.056 [3.161–3.216] | 4.098 / 4.507 [4.081–4.123] | 0.528 / 0.701 |
| Medium | Off | 1 | 3.110 / 3.393 [2.421–3.267] | 3.884 / 4.677 [3.820–4.049] | 0.525 / 0.699 |
| Medium | 2× | 2 | 3.017 / 4.000 [2.863–3.161] | 4.103 / 5.678 [4.088–4.145] | 0.537 / 0.707 |
| Medium | 4× | 4 | 3.197 / 4.201 [3.173–4.048] | 4.072 / 4.265 [4.026–4.102] | 0.530 / 0.706 |
| High | Auto | 4 | 4.312 / 4.545 [4.247–4.343] | 6.186 / 6.807 [5.095–6.306] | 0.343 / 0.497 |
| High | Off | 1 | 3.399 / 3.830 [3.367–3.459] | 4.364 / 5.900 [4.296–5.390] | 0.344 / 0.490 |
| High | 2× | 2 | 3.925 / 4.194 [3.903–3.980] | 5.074 / 6.321 [4.911–5.807] | 0.341 / 0.489 |
| High | 4× | 4 | 4.369 / 4.986 [4.309–4.467] | 6.229 / 6.383 [5.067–6.312] | 0.347 / 0.491 |

## AMD integrated GPU, 1080p, Native

| Preset | AA request | Samples | Normal span | Far span (0.065) | CPU preparation normal / far |
| --- | --- | ---: | ---: | ---: | ---: |
| Auto | Auto | 4 | 5.196 / 5.384 [5.171–5.252] | 5.077 / 6.401 [4.977–6.197] | 0.343 / 0.486 |
| Auto | Off | 1 | 3.963 / 4.441 [3.920–4.085] | 4.315 / 5.712 [4.297–4.348] | 0.345 / 0.489 |
| Auto | 2× | 2 | 4.642 / 4.951 [4.627–4.692] | 5.044 / 6.272 [4.869–6.123] | 0.342 / 0.486 |
| Auto | 4× | 4 | 5.197 / 5.435 [5.152–5.234] | 5.160 / 6.413 [5.046–6.242] | 0.341 / 0.489 |
| Low | Auto | 4 | 1.375 / 1.687 [1.358–1.396] | 1.228 / 1.403 [1.215–1.267] | 0.494 / 0.624 |
| Low | Off | 1 | 0.739 / 0.869 [0.725–0.749] | 0.805 / 0.896 [0.793–0.809] | 0.490 / 0.624 |
| Low | 2× | 2 | 1.275 / 1.479 [1.269–1.281] | 1.206 / 1.383 [1.197–1.212] | 0.494 / 0.624 |
| Low | 4× | 4 | 1.368 / 1.465 [1.355–1.383] | 1.224 / 1.324 [1.211–1.234] | 0.490 / 0.622 |
| Medium | Auto | 4 | 4.286 / 4.576 [3.696–4.331] | 4.101 / 5.507 [4.073–4.114] | 0.530 / 0.703 |
| Medium | Off | 1 | 3.279 / 3.539 [2.720–3.336] | 3.761 / 4.123 [3.402–3.811] | 0.528 / 0.704 |
| Medium | 2× | 2 | 3.226 / 4.253 [3.206–3.276] | 4.073 / 4.251 [4.044–4.092] | 0.532 / 0.702 |
| Medium | 4× | 4 | 4.265 / 5.011 [3.961–4.285] | 4.102 / 5.762 [4.070–4.112] | 0.538 / 0.718 |
| High | Auto | 4 | 5.276 / 5.853 [5.223–5.410] | 5.196 / 6.423 [5.030–6.267] | 0.350 / 0.494 |
| High | Off | 1 | 3.972 / 4.256 [3.935–3.986] | 4.303 / 5.542 [4.276–4.354] | 0.342 / 0.486 |
| High | 2× | 2 | 4.683 / 5.363 [4.632–4.878] | 5.776 / 6.828 [4.997–6.185] | 0.350 / 0.495 |
| High | 4× | 4 | 5.255 / 5.577 [5.228–5.285] | 6.268 / 6.410 [5.123–6.304] | 0.344 / 0.492 |

## Provenance and limits

Artifacts below are retained outside Git. The timing run and original candidate default controls used the same executable and the measured source recorded below. Default controls request no timestamp features; the separate timing invocation enables encoder timestamps. No timing-enabled controls are promoted into pixel proof.

| Artifact | SHA-256 |
| --- | --- |
| `baseline-bin/graphics_settings_control-v3.exe` | `98a0861f3123b3a6922d01de942744c21a11a9626251b18b91c4157b331c7001` |
| `candidate-bin/graphics_settings-final.exe` | `01c8999a30b3b475b12ab8a520174fcaf69d1c3ad00fcdd7dea9f7a540196297` |
| `candidate-bin/menu_specimens-v3.exe` | `b260df58c238d0083143e0ae15674f46e8e2e36acfbbda750f4a466b3929b906` |
| `timings-final/frames.csv` | `bb780a7195d9cadf46ca90bcd13a5502d0f1a0b5d979af58839a3fb4ccde3965` |
| `timings-final/details.json` | `727a7b695f47f65465d3744534488f6b9ee8b903c14e332d53a7081208add57e` |
| `load-final.jsonl` | `d0dcd37eb7756372449a3931c2dbcc930d2173e66c06c1c1b5d85a410fe4e7ee` |
| `final-default-parity.json` | `81c9cbe3c00ee382b8865a27cf7c3f94f9571b1834047dc201b573490e49ecbb` |
| `settings-comparison-export/comparison.png` | `769f04d0e094258862f3762b7e2e86de4074fd46aaecce14a4567efe904a70f7` |
| `owner/slither-wgpu-desktop.exe` (pre-rebase receipt) | `9dd58e2a7cdb74420d0a244f295f198060cde4eeeb433b847f5e4049672c4666` |

Measured source bindings (Git blob IDs, before rebase):

| File | Blob |
| --- | --- |
| `client/src/settings.rs` | `abce412415b6f58595ecc7d8cecf3063d55d2ede` |
| `client/src/presentation/quality.rs` | `64a34bfafe905d61526af1d1a7b55e6d5ae95733` |
| `client/src/start_page/menu.rs` | `e4f3061ead85269fde97e7f5910fab2a67d1e93d` |
| `client/src/world_resolution.rs` | `1190107e607ffd13d34e32f723048e9c0b66202a` |
| `client/apps/wgpu-slice/src/render.rs` | `ed28d1933d133c01d0f4d12405c7f8eb0cc0391f` |
| `client/apps/wgpu-slice/src/gpu.rs` | `84c4c25cefbe473f3880ec58cadd1d2520c27a05` |
| `client/apps/wgpu-slice/src/pellet_gpu.rs` | `3392f4f1ebba04351f5188eccfb3f876ca48a32e` |
| `client/apps/wgpu-slice/src/world_presenter.rs` | `f69f563364c897bff40e07443cab833530997afd` |
| `client/apps/wgpu-slice/src/main.rs` | `00eb9d1dad7bdb7f3dd63e33435991b73ec2149f` |
| `client/apps/wgpu-slice/src/settings_egui.rs` | `a710478a9a3ee8c07ab71e3252d9716d09614e25` |

Retained excluded attempts: the first small-arena control clamped far zoom to 0.373; it was replaced by the large-arena fixture, with the actual factor asserted. Initial cross-checkout Cargo reuse was rejected and rebuilt after package cleaning. One later control command did not stop after a timing-lock acquisition failure; its results were excluded and replaced by owned-lock reruns. Original files/errors remain in local task notes.

Native Settings specimens use the production egui host, authored data and no art directory, at 960×540, 360×640, 1920×1080 and 3840×2160. The PR comparison uses matched 1920×1080/DPR-1 crops. Synthetic keys/layout tests establish focus, capture and checked-state contracts; physical Windows input, owner appearance/feel, browser rendering and live gameplay remain unrun. New selectors are desktop-only; both Wasm parsers expose and retain the v3 fields.

## Reproduction

Use a fresh external directory, the pinned toolchain, the lane lock, and per-command `CARGO_TARGET_DIR`/`CARGO_BUILD_JOBS=2` as required by the task. Build `graphics_settings_perf` in release. Run `graphics_settings_perf NEW_DIRECTORY --control` for DX12 controls; set `GRAPHICS_SETTINGS_VULKAN=1` only for the separate Vulkan control run. Without `--control`, the driver runs the five-round DX12 timing matrix. Run `menu_specimens NEW_DIRECTORY --settings` for the authored Settings screen. All are finite and create no window or socket.

## Rebase verification

The graphics code was rebased unchanged onto `60bac3ea` (#462). `git range-diff` shows only this report's integration receipts beyond the original `af11cc60` patch. The upstream change adds menu/HUD visibility boundaries. The timing tables above retain their original measured source and artifacts; they were not retimed or relabelled as measurements of the rebased binary.

Rebased authored default controls matched all 16 Vulkan RGBA images, all eight AMD DX12 images, all remaining repeatable RTX DX12 controls, and all 32 canonical frame/resource records against the same pinned baseline. The same four RTX DX12 MSAA4 cells remain qualified as above. Full `npm run check`, both real Wasm login ABI checks, and the four explicit world-resolution GPU regressions passed after integration. The Settings UI code is unchanged, so the authored panel comparison remains applicable.

The owner executable was rebuilt with default features from the rebased source. The previous executable is preserved separately; the fixed owner path now contains the new build.

| Rebase artifact | SHA-256 |
| --- | --- |
| `owner/slither-wgpu-desktop.exe` | `1167af323b4e9dc071c98f628c505802e375082ef90c6f247511c3e5ab3ee149` |
| `owner/slither-wgpu-desktop-af11cc60.exe` | `9dd58e2a7cdb74420d0a244f295f198060cde4eeeb433b847f5e4049672c4666` |
| `candidate-bin/graphics_settings-rebased.exe` | `95fa450f6c8b888738760696bdb0a7ae88c43c9235b97553e192e08e087095a0` |
| `rebased-default-parity.json` | `6a4b99c4e150f7bcac268ba8f037e014252c07eb1575aae36459135524cbf3a6` |

Rebased production bindings:

| File | Git blob |
| --- | --- |
| `client/apps/wgpu-slice/src/render.rs` | `47c176b8ca4ba7354a7e3fbc2925ec744e9e677e` |
| `client/apps/wgpu-slice/src/main.rs` | `dff731d8364172f210c15beb0d1c0154dd5502a0` |
| `client/apps/wgpu-slice/src/game.rs` | `4093f4bd1b6c6cc1fddab9bd7d579a39daa43559` |
| `client/apps/wgpu-slice/src/menu.rs` | `9a0e88b9e38d0a68ed660e1b2151553745bae8ee` |
| `client/src/session_policy.rs` | `448bd2fd785545e09c65735b7cd7fa294b064fe1` |
