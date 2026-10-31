//! Authored headless shared-painter regressions; no GPU or window.
use slither_client::{
    champion::ChampionState,
    menu_controls::*,
    menu_input::*,
    menu_theme::*,
    session_policy::{Command, SessionPolicy},
};
use slither_wgpu_slice::{menu::Menu, text::TextCache};
use std::collections::BTreeMap;
fn panel() -> PanelView {
    let mut s = SessionPolicy::new();
    s.prepare_controls(
        &[
            Command {
                opcode: b'a',
                argument: 0,
            },
            Command {
                opcode: b'v',
                argument: 1,
            },
        ],
        0.,
    )
    .unwrap();
    s.commit_controls().unwrap();
    let mut champion = ChampionState::default();
    champion.observe(1, &s.view());
    champion.step(0., 1.).unwrap();
    champion.panel(&s.view(), [960., 720.])
}

fn transparent_border(image: &slither_wgpu_slice::materials::RgbaImage) {
    let (w, h) = (image.width as usize, image.height as usize);
    for y in 0..h {
        for x in 0..w {
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                assert_eq!(
                    &image.pixels[(y * w + x) * 4..][..4],
                    &[0; 4],
                    "edge ({x},{y}) in {w}x{h}"
                );
            }
        }
    }
    assert!(image.pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
}

#[test]
fn every_shadow_recipe_reaches_transparent_outer_texels() {
    use slither_wgpu_slice::materials::{MenuSurface, menu_control};
    for scale in [1., 1.5, 2., 4., 5.] {
        for surface in [
            MenuSurface::Nickname,
            MenuSurface::Play,
            MenuSurface::Compact,
        ] {
            let image = menu_control(
                [(120. * scale) as u32, (48. * scale) as u32],
                24. * scale,
                [[76, 68, 124]; 2],
                scale,
                surface,
            )
            .unwrap();
            transparent_border(&image);
        }
    }
}

#[test]
fn sliced_champion_shadow_keeps_full_padding_and_respects_explicit_clips() {
    use slither_wgpu_slice::materials::{MenuSurface, RgbaImage, menu_control};
    for scale in [1., 1.5, 2.] {
        let mut panel = panel();
        panel.initial_focus = None;
        panel.controls = vec![panel.controls[2].clone()];
        let resolved = panel.resolve([960., 720.]).unwrap();
        let field = resolved[0].rect;
        let mut input = PanelInputState::default();
        let mut edit = MenuInputState::default();
        input.sync(&panel, &mut edit);
        let mut menu = Menu::default();
        let mut text = TextCache::menu();
        let mut scroll = BTreeMap::new();
        let frame = menu
            .prepare_panel(
                &mut text,
                &panel,
                &input,
                &edit,
                &mut scroll,
                MenuTheme::default(),
                [(960. * scale) as u32, (720. * scale) as u32],
                scale,
            )
            .unwrap();
        let pad = (40. * scale) as u32;
        let width = (field.width * scale).round() as u32 + 2 * pad;
        let height = (field.height * scale).round() as u32 + 2 * pad;
        let origin = [
            (field.x * scale).round() as i32 - pad as i32,
            (field.y * scale).round() as i32 - pad as i32,
        ];
        let mut stitched = RgbaImage {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
        };
        let mut covered = vec![false; width as usize * height as usize];
        for draw in &frame.frame.draws {
            let resource = frame
                .resources
                .iter()
                .find(|r| r.asset == draw.asset)
                .unwrap();
            let source = draw.source.unwrap();
            let left =
                ((draw.destination.x + draw.translation[0]) * scale).round() as i32 - origin[0];
            let top =
                ((draw.destination.y + draw.translation[1]) * scale).round() as i32 - origin[1];
            assert!(left >= 0 && top >= 0);
            assert!((draw.destination.width * scale - source.width).abs() < 1e-7);
            assert!((draw.destination.height * scale - source.height).abs() < 1e-7);
            for y in 0..source.height as usize {
                for x in 0..source.width as usize {
                    let dx = left as usize + x;
                    let dy = top as usize + y;
                    assert!(dx < width as usize && dy < height as usize);
                    let target = dy * width as usize + dx;
                    assert!(!covered[target], "overlapping material slices");
                    covered[target] = true;
                    let src = ((source.y as usize + y) * resource.image.width as usize
                        + source.x as usize
                        + if draw.corners.is_some() {
                            source.width as usize - 1 - x
                        } else {
                            x
                        })
                        * 4;
                    stitched.pixels[target * 4..][..4]
                        .copy_from_slice(&resource.image.pixels[src..][..4]);
                }
            }
        }
        // Covers the complete padded holder, including pixels outside the
        // panel's x=240..720 layout box, and reconstructs the unsliced raster.
        assert!(covered.into_iter().all(|v| v));
        transparent_border(&stitched);
        let expected = menu_control(
            [
                (field.width * scale).round() as u32,
                (field.height * scale).round() as u32,
            ],
            24. * scale,
            [[76, 68, 124]; 2],
            scale,
            MenuSurface::Nickname,
        )
        .unwrap();
        assert_eq!(stitched.pixels, expected.pixels);
        panel.controls[0].clip = Some(slither_client::menu_layout::MenuRect {
            x: 18.,
            y: 88.,
            width: 444.,
            height: 48.,
        });
        let clipped = menu
            .prepare_panel(
                &mut text,
                &panel,
                &input,
                &edit,
                &mut scroll,
                MenuTheme::default(),
                [(960. * scale) as u32, (720. * scale) as u32],
                scale,
            )
            .unwrap();
        assert!(
            clipped
                .frame
                .draws
                .iter()
                .all(|d| d.destination.x >= field.x
                    && d.destination.y >= field.y
                    && d.destination.x + d.destination.width <= field.x + field.width + 1e-7
                    && d.destination.y + d.destination.height <= field.y + field.height + 1e-7)
        );
    }
}

#[test]
fn sliced_materials_theme_keys_scroll_clip_and_failed_publication() {
    let mut p = panel();
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&p, &mut edit);
    edit.apply(EditIntent::KeyText("W".repeat(140)));
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut scroll = BTreeMap::new();
    let theme = MenuTheme::default();
    let old = menu
        .prepare_panel(
            &mut text,
            &p,
            &input,
            &edit,
            &mut scroll,
            theme,
            [1440, 1080],
            1.5,
        )
        .unwrap();
    assert!(scroll[&EditorId::Champion] > 0.);
    assert!(old.resources.len() <= 64);
    assert!(
        old.resources
            .iter()
            .map(|r| r.image.pixels.len())
            .sum::<usize>()
            <= 4 * 1024 * 1024
    );
    assert!(
        old.resources
            .iter()
            .all(|r| r.asset.width <= 512 && r.asset.height <= 512)
    );
    assert!(old.frame.draws.len() <= 512);
    let field = p.resolve([960., 720.]).unwrap()[2].rect;
    for d in old
        .frame
        .draws
        .iter()
        .filter(|d| d.asset.handle >= slither_wgpu_slice::bank::MENU_TEXT_HANDLE_BASE)
    {
        let y = d.destination.y + d.translation[1];
        if y >= field.y && y < field.y + field.height {
            assert!(d.destination.x + d.translation[0] >= field.x + 12.);
            assert!(
                d.destination.x + d.translation[0] + d.destination.width
                    <= field.x + field.width - 12. + 1e-7
            );
        }
    }
    let keys = |p: &slither_wgpu_slice::menu::MenuFrame| {
        p.resources
            .iter()
            .map(|r| (r.asset, r.image.pixels.clone()))
            .collect::<Vec<_>>()
    };
    edit.apply(EditIntent::SelectAll);
    let selected = menu
        .prepare_panel(
            &mut text,
            &p,
            &input,
            &edit,
            &mut scroll,
            theme,
            [1440, 1080],
            1.5,
        )
        .unwrap();
    assert!(
        selected
            .resources
            .iter()
            .all(|r| r.asset.width <= 512 && r.asset.height <= 512)
    );
    assert!(selected.frame.draws.len() <= 512);
    edit.apply(EditIntent::Move {
        movement: Movement::End,
        extend: false,
    });
    p.alpha = 0.5;
    p.placement.offset[0] += 1.;
    let moved = menu
        .prepare_panel(
            &mut text,
            &p,
            &input,
            &edit,
            &mut scroll,
            theme,
            [1440, 1080],
            1.5,
        )
        .unwrap();
    assert_eq!(keys(&old), keys(&moved));
    let retained = keys(&old);
    let previous_scroll = scroll.clone();
    let original = p.clone();
    let template = p.controls[3].clone();
    p.controls.extend((10..70).map(|id| {
        let mut c = template.clone();
        c.id = ControlId(id);
        c
    }));
    assert!(
        menu.prepare_panel(
            &mut text,
            &p,
            &input,
            &edit,
            &mut scroll,
            theme,
            [1440, 1080],
            1.5
        )
        .is_err()
    );
    assert_eq!(scroll, previous_scroll);
    p = original;
    let recovered = menu
        .prepare_panel(
            &mut text,
            &p,
            &input,
            &edit,
            &mut scroll,
            theme,
            [1440, 1080],
            1.5,
        )
        .unwrap();
    assert_eq!(keys(&recovered), keys(&moved));
    let changed = MenuTheme {
        field: [90, 70, 130],
        ..theme
    };
    assert_ne!(
        theme.surface(ControlStyle::Field, ControlState::Idle),
        changed.surface(ControlStyle::Field, ControlState::Idle)
    );
    let colored = menu
        .prepare_panel(
            &mut text,
            &p,
            &input,
            &edit,
            &mut scroll,
            changed,
            [1440, 1080],
            1.5,
        )
        .unwrap();
    assert_ne!(keys(&colored), keys(&moved));
    // Retirement cannot mutate an already prepared picture.
    assert_eq!(keys(&old), retained);
}
#[test]
fn tall_surface_uses_nine_slice_and_charges_each_draw() {
    let mut p = panel();
    p.placement = Placement::rect(0., 0., 960., 720.);
    p.initial_focus = None;
    let mut field = p.controls[2].clone();
    field.placement = Placement::rect(0., 0., 600., 400.);
    p.controls = vec![field];
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&p, &mut edit);
    let frame = Menu::default()
        .prepare_panel(
            &mut TextCache::menu(),
            &p,
            &input,
            &edit,
            &mut BTreeMap::new(),
            MenuTheme::default(),
            [960, 720],
            1.,
        )
        .unwrap();
    assert!(
        frame
            .resources
            .iter()
            .all(|r| r.asset.width <= 512 && r.asset.height <= 512)
    );
    assert!(frame.frame.draws.len() > 9 && frame.frame.draws.len() <= 512);
    p.controls[0].placement.size = [960., 720.];
    assert!(
        Menu::default()
            .prepare_panel(
                &mut TextCache::menu(),
                &p,
                &input,
                &edit,
                &mut BTreeMap::new(),
                MenuTheme::default(),
                [1920, 1440],
                2.
            )
            .is_err(),
        "all tiles charge producer cap"
    );
}

#[test]
fn high_dpr_fields_bound_text_rasters_and_slice_selection_ink() {
    let p = panel();
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&p, &mut edit);
    for dpr in [4., 5.] {
        let mut menu = Menu::default();
        let mut text = TextCache::menu();
        let mut scroll = BTreeMap::new();
        for n in [0, 140, 50] {
            edit.reset_editor(EditorId::Champion, &"W".repeat(n));
            if n == 50 {
                edit.apply(EditIntent::SelectAll);
            }
            let frame = menu
                .prepare_panel(
                    &mut text,
                    &p,
                    &input,
                    &edit,
                    &mut scroll,
                    MenuTheme::default(),
                    [(960. * dpr) as u32, (720. * dpr) as u32],
                    dpr,
                )
                .unwrap();
            assert!(frame.frame.draws.len() <= 512);
            assert!(
                frame
                    .resources
                    .iter()
                    .map(|r| r.image.pixels.len())
                    .sum::<usize>()
                    <= 4 * 1024 * 1024
            );
            assert!(
                frame
                    .resources
                    .iter()
                    .all(|r| r.asset.width <= 512 && r.asset.height <= 2048)
            );
            let field = p.resolve([960., 720.]).unwrap()[2].rect;
            let field_text: Vec<_> = frame
                .frame
                .draws
                .iter()
                .filter(|d| {
                    d.asset.handle >= slither_wgpu_slice::bank::MENU_TEXT_HANDLE_BASE
                        && d.destination.y >= field.y
                        && d.destination.y < field.y + field.height
                })
                .collect();
            let raster_width: u32 = field_text.iter().map(|d| d.asset.width).sum();
            assert!(
                raster_width <= (420. * dpr) as u32 + 2 * field_text.len() as u32 + 2,
                "field raster must cover only the visible window, including tile overlap"
            );
            println!(
                "panel DPR {dpr}, {n} units: {} bytes",
                frame
                    .resources
                    .iter()
                    .map(|r| r.image.pixels.len())
                    .sum::<usize>()
            );
        }
    }
}

#[test]
fn champion_keeps_returning_login_last_length_geometry_and_fade() {
    use slither_client::{menu_layout::MenuElement, start_page::LastLifeView};
    use slither_wgpu_slice::{menu::PanelPaint, menu_specimen};
    let p = panel();
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&p, &mut edit);
    for alpha in [0.35, 1.] {
        let mut view = menu_specimen::view(0).unwrap();
        view.tips_visible = false;
        view.login_alpha = alpha;
        view.last_life = LastLifeView {
            label: view.wording.final_length,
            number: "1234".into(),
            suffix: "!",
            label_alpha: 0.45,
        };
        let mut text = TextCache::menu();
        let layout = menu_specimen::layout(&mut text, &view, [960, 720], 1.).unwrap();
        let rect = layout.viewport_rect(MenuElement::LastLength);
        let mut menu = Menu::default();
        let login = menu
            .prepare(&mut text, &view, &layout, [960, 720], 1., None)
            .unwrap();
        view.ordinary_controls_visible = false;
        let champion = menu
            .prepare_with_panel(
                &mut text,
                &view,
                &layout,
                [960, 720],
                1.,
                None,
                None,
                Some(PanelPaint {
                    panel: &p,
                    input: &input,
                    edit: &edit,
                    scroll: &mut BTreeMap::new(),
                    theme: MenuTheme::default(),
                }),
            )
            .unwrap();
        let line = |frame: &slither_wgpu_slice::menu::MenuFrame| {
            frame
                .frame
                .draws
                .iter()
                .filter(|d| {
                    d.asset.handle >= slither_wgpu_slice::bank::MENU_TEXT_HANDLE_BASE
                        && d.destination.y + d.destination.height > rect.y
                        && d.destination.y < rect.y + rect.height
                })
                .map(|d| {
                    (
                        d.destination,
                        d.translation,
                        d.effective_alpha,
                        frame
                            .resources
                            .iter()
                            .find(|r| r.asset == d.asset)
                            .unwrap()
                            .image
                            .pixels
                            .clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let expected = line(&login);
        assert_eq!(expected.len(), 3);
        assert_eq!(expected[0].2, alpha * 0.45);
        assert_eq!(expected[1].2, alpha);
        assert_eq!(line(&champion), expected);
    }
}

#[test]
fn menu_budget_failures_skip_overlay_and_preserve_picture() {
    use slither_wgpu_slice::{menu::MenuFrame, render::PreparedFrame};
    let mut p = panel();
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&p, &mut edit);
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let base = MenuFrame::world_crossfade([960, 720], 1., 0.).unwrap();
    let mut picture = PreparedFrame {
        frame: base.frame.clone(),
        overlay: None,
        resources: vec![],
    };
    let template = p.controls[3].clone();
    p.controls = (0..64)
        .map(|id| {
            let mut c = template.clone();
            c.id = ControlId(id);
            c
        })
        .collect();
    let failed = menu.prepare_panel(
        &mut text,
        &p,
        &input,
        &edit,
        &mut BTreeMap::new(),
        MenuTheme::default(),
        [960, 720],
        1.,
    );
    assert!(failed.is_err());
    let quota = failed.as_ref().err().unwrap().clone();
    assert!(
        MenuFrame::compose_optional(Err(quota), &picture)
            .unwrap()
            .is_none()
    );
    assert!(!MenuFrame::append_optional(failed, &mut picture).unwrap());
    assert_eq!(picture.frame, base.frame);
    assert!(picture.resources.is_empty());
    // Whole-picture admission is equally recoverable and atomic.
    let resource = base.resources[0].clone();
    picture.resources = (0..2048)
        .map(|i| {
            let mut r = resource.clone();
            r.asset.handle = i + 1;
            r
        })
        .collect();
    assert!(!MenuFrame::append_optional(Ok(base), &mut picture).unwrap());
    assert_eq!(picture.resources.len(), 2048);
    assert!(picture.frame.draws.is_empty());
    let invalid = MenuFrame::world_crossfade([800, 600], 1., 0.).unwrap();
    assert!(MenuFrame::append_optional(Ok(invalid), &mut picture).is_err());
}

#[test]
fn optional_menu_propagates_non_quota_preparation_errors() {
    use slither_wgpu_slice::{menu::MenuFrame, render::PreparedFrame};
    let mut p = panel();
    let label = p.controls[0].clone();
    let field = p.controls[2].clone();
    p.initial_focus = None;
    for (scale, control, expected) in [
        (0.01, label, "invalid text run/style/budget"),
        (20., field, "Sliced menu scale budget"),
    ] {
        p.placement.scale = scale;
        p.controls = vec![control];
        let mut input = PanelInputState::default();
        let mut edit = MenuInputState::default();
        input.sync(&p, &mut edit);
        let base = MenuFrame::world_crossfade([960, 720], 1., 0.).unwrap();
        let mut picture = PreparedFrame {
            frame: base.frame.clone(),
            overlay: None,
            resources: vec![],
        };
        let prepared = Menu::default().prepare_panel(
            &mut TextCache::menu(),
            &p,
            &input,
            &edit,
            &mut BTreeMap::new(),
            MenuTheme::default(),
            [960, 720],
            1.,
        );
        assert_eq!(prepared.as_ref().err().map(String::as_str), Some(expected));
        assert_eq!(
            MenuFrame::compose_optional(Err(prepared.as_ref().err().unwrap().clone()), &picture)
                .err(),
            Some(expected.into())
        );
        assert_eq!(
            MenuFrame::append_optional(prepared, &mut picture),
            Err(expected.into())
        );
        assert_eq!(picture.frame, base.frame);
        assert!(picture.resources.is_empty());
    }
}

#[test]
fn champion_and_logo_share_the_complete_menu_budget_at_high_dpr() {
    use slither_wgpu_slice::{
        menu::{MenuFrame, PanelPaint},
        menu_specimen,
        render::PreparedFrame,
        text::MENU_CACHE_BYTES,
    };
    let extent = [8000, 5400];
    let dpr = 5.;
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut view = menu_specimen::view(0).unwrap();
    view.ordinary_controls_visible = false;
    view.tips_visible = false;
    view.champion_pending = true;
    let layout = menu_specimen::layout(&mut text, &view, extent, dpr).unwrap();
    // Move from fitting stroke -> fitting font -> neither fits -> stroke again.
    for count in [0, 4, 8, 0] {
        let mut panel = panel();
        panel.placement = Placement::rect(0., 0., 1600., 1080.);
        // Synthetic picker instructions share the champion painter/text bank.
        for index in 0..count {
            let mut control = panel.controls[0].clone();
            control.id = ControlId(40 + index);
            control.placement = Placement::rect(10., 100. + f64::from(index) * 60., 900., 40.);
            control.clip = None;
            control.style = ControlStyle::Heading;
            control.kind = ControlKind::Label {
                text: "W".repeat(50),
            };
            panel.controls.push(control);
        }
        let mut input = PanelInputState::default();
        let mut edit = MenuInputState::default();
        input.sync(&panel, &mut edit);
        let cached = text.cached_runs();
        let prepared = menu.prepare_with_panel(
            &mut text,
            &view,
            &layout,
            extent,
            dpr,
            None,
            None,
            Some(PanelPaint {
                panel: &panel,
                input: &input,
                edit: &edit,
                scroll: &mut BTreeMap::new(),
                theme: MenuTheme::default(),
            }),
        );
        if count == 8 {
            assert!(prepared.is_err());
            assert_eq!(text.cached_runs(), cached);
            let base = MenuFrame::world_crossfade(extent, dpr, 1.).unwrap();
            let picture = PreparedFrame {
                frame: base.frame,
                resources: base.resources,
                overlay: None,
            };
            assert!(
                MenuFrame::compose_optional(prepared, &picture)
                    .unwrap()
                    .is_none()
            );
            continue;
        }
        let frame = prepared.unwrap_or_else(|e| panic!("champion DPR {dpr}, {count} labels: {e}"));
        let bytes = frame
            .resources
            .iter()
            .map(|r| r.image.pixels.len())
            .sum::<usize>();
        eprintln!("champion DPR {dpr}, {count} picker labels: {bytes} bytes");
        assert!(bytes <= MENU_CACHE_BYTES);
        assert!(!frame.frame.draws.is_empty());
        assert!(frame.frame.draws.len() <= 512);
        let green = slither_wgpu_slice::menu::WORDMARK_GREEN;
        let font_logo = frame.resources.iter().any(|r| {
            r.asset.handle >> 60 == 8
                && r.image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[..3] == green && p[3] == 255)
        });
        assert_eq!(font_logo, count == 4);
    }
}
