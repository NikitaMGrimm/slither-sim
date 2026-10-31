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
    for scale in [1., 1.5, 2.] {
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
                        + x)
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
