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
