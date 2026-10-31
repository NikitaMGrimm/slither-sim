//! Original synthetic storage, transport and preview regression fixtures.
#![cfg(not(target_arch = "wasm32"))]
#[path = "support/announcement_tape.rs"]
mod support;
use slither_client::skin_studio::*;
use slither_wgpu_slice::{
    game::{LocalGame, LoginProfile},
    menu_storage::MenuStorage,
    skin_studio_native::SkinStudioNative,
};

fn temp() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "skin-studio-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).unwrap();
    p
}
#[test]
fn startup_is_read_only_unknown_corrupt_v1_and_atomic_replacement() {
    let path = temp();
    let v1 = "1:4f6c64:41:00000000000000000102";
    std::fs::write(path.join("menu.v1"), v1).unwrap();
    let mut storage = MenuStorage::at(Some(path.clone()));
    let restored = storage.restore();
    assert_eq!(restored.record.selection().skin, 65);
    assert!(!path.join("menu.v2").exists());
    storage.save(&restored.record).unwrap();
    assert_eq!(std::fs::read_to_string(path.join("menu.v1")).unwrap(), v1);
    let mut new = restored.record;
    new.nickname = "New".into();
    storage.save(&new).unwrap();
    assert_eq!(storage.restore().record, new);
    std::fs::write(path.join("menu.v2"), "9:future").unwrap();
    assert!(!storage.restore().persistence_enabled);
    assert!(storage.save(&new).is_err());
    assert_eq!(
        std::fs::read_to_string(path.join("menu.v2")).unwrap(),
        "9:future"
    );
    std::fs::write(path.join("menu.v2"), "2:corrupt").unwrap();
    assert_eq!(storage.restore().record, AppearanceRecord::default());
    assert_eq!(
        std::fs::read_to_string(path.join("menu.v2")).unwrap(),
        "2:corrupt"
    );
    storage.save(&new).unwrap();
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 2);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn failed_storage_keeps_one_memory_commit_and_cancel_never_writes() {
    let path = temp();
    let mut studio = SkinStudioNative::new(
        AppearanceRecord::default(),
        MenuStorage::at(Some(path.clone())),
    )
    .unwrap();
    studio.open(true);
    studio.step(230.);
    studio.activate(NEXT, "");
    studio.activate(CANCEL, "");
    studio.step(460.);
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
    studio.open(true);
    studio.step(690.);
    studio.activate(NEXT, "");
    // A directory in place of the file rejects atomic replacement without losing it.
    std::fs::create_dir(path.join("menu.v2")).unwrap();
    studio.activate(SAVE, "Current");
    studio.activate(SAVE, "Ignored");
    assert_eq!(studio.status, "Saved for this run");
    assert_eq!(studio.state.committed().builtin, 1);
    assert_eq!(studio.state.committed().nickname, "Current");
    assert!(path.join("menu.v2").is_dir());
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 1);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn delayed_profile_and_endpoint_send_frozen_exact_appearance_only_on_play() {
    for skin in [0, 41, 64, 65] {
        for bytes in [vec![], vec![0, 0, 0, 0, 0, 0, 0, 0, 255, 41, 1, 2]] {
            let record = AppearanceRecord {
                builtin: skin,
                custom_base: skin,
                custom_bytes: bytes.clone(),
                active_custom: !bytes.is_empty(),
                ..Default::default()
            };
            let mut studio = SkinStudioNative::new(record, MenuStorage::at(None)).unwrap();
            let mut game = LocalGame::new(0.).unwrap();
            studio.open(true);
            studio.step(230.);
            studio.activate(SAVE, "");
            assert!(game.effect().is_none());
            let frozen = studio.state.committed().selection();
            studio.step(460.);
            studio.open(true);
            studio.activate(NEXT, "");
            let profile = LoginProfile {
                client_version: 291,
                page_echo: [19; 20],
            };
            game.play_with_appearance("ws://127.0.0.1:19491/slither", "Frozen", &frozen, &profile)
                .unwrap();
            let expected = slither_client::protocol::encode_login(
                291,
                &[19; 20],
                "Frozen",
                skin,
                Some(&bytes),
            )
            .unwrap();
            support::pump(&mut game, 1.);
            support::effects(&mut game);
            game.opened(game.generation(), 2.).unwrap();
            support::effects(&mut game);
            game.receive(
                game.generation(),
                &support::bundle(&[support::challenge()]),
                3.,
            )
            .unwrap();
            assert_eq!(support::effects(&mut game)[1], expected);
            let mut endpoint = LocalGame::new(0.).unwrap();
            slither_wgpu_slice::endpoint_login::play_appearance(
                &mut endpoint,
                "ws://127.0.0.1:19491/slither",
                "Frozen",
                &frozen,
            )
            .unwrap();
            support::pump(&mut endpoint, 1.);
            support::effects(&mut endpoint);
            endpoint.opened(endpoint.generation(), 2.).unwrap();
            support::effects(&mut endpoint);
            endpoint
                .receive(
                    endpoint.generation(),
                    &support::bundle(&[support::challenge()]),
                    3.,
                )
                .unwrap();
            let sent = support::effects(&mut endpoint);
            assert_eq!(&sent[1][24..], &expected[24..]);
        }
    }
}
#[test]
fn all_skin_ready_and_fallback_poses_fit_caps_at_every_dpr_and_survive_retry() {
    use slither_wgpu_slice::{cosmetics::Cosmetics, menu::Menu, text::TextCache};
    let mut png = std::io::Cursor::new(vec![]);
    image::RgbaImage::from_pixel(16, 16, image::Rgba([190, 120, 210, 255]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let full = Cosmetics::load(|_| Ok(png.get_ref().clone()), Some(0)).unwrap();
    let missing = Cosmetics::from_images(Default::default(), None).unwrap();
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut input = slither_client::menu_controls::PanelInputState::default();
    let mut edit = slither_client::menu_input::MenuInputState::default();
    let mut scroll = Default::default();
    let mut maximum = 0;
    for dpr in [5, 1, 2, 3, 4] {
        let dpr = f64::from(dpr);
        let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
        for skin in 0..=65 {
            let mut studio = SkinStudioNative::new(
                AppearanceRecord {
                    builtin: skin,
                    ..Default::default()
                },
                MenuStorage::at(None),
            )
            .unwrap();
            studio.open(true);
            studio.step(230.);
            for sources in [&missing, &full] {
                for now in [230., 355., 730.] {
                    studio.step(now);
                    let (preview, resources) =
                        studio.prepare_preview(extent, dpr, Some(sources)).unwrap();
                    if let Some(antenna) = sources.skin(skin, &[]).unwrap().antenna {
                        let bulb = sources.role(antenna.source).unwrap();
                        assert!(
                            preview.draws.iter().any(|d| d.asset == bulb),
                            "skin {skin} omitted ready antenna"
                        );
                    }
                    let held = resources.clone();
                    let held_draws = preview.draws.clone();
                    let (retry, _) = studio.prepare_preview(extent, dpr, Some(sources)).unwrap();
                    assert_eq!(retry.draws, held_draws);
                    assert!(held.iter().all(|r| !r.image.pixels.is_empty()));
                    assert!(preview.draws.len() <= 1024);
                    maximum = maximum.max(preview.draws.len());
                    let panel = studio.state.panel([960., 540.], &studio.status);
                    input.sync(&panel, &mut edit);
                    let controls = menu
                        .prepare_panel(
                            &mut text,
                            &panel,
                            &input,
                            &edit,
                            &mut scroll,
                            Default::default(),
                            extent,
                            dpr,
                        )
                        .unwrap_or_else(|e| panic!("DPR {dpr} skin {skin} clock {now}: {e}"));
                    assert!(controls.frame.draws.len() <= 384);
                    assert!(controls.resources.len() <= 40); // At least 24 of the 64 Menu slots remain.
                    for (namespace, slots, bytes) in
                        [(8, 20, 2 * 1024 * 1024), (7, 8, 2 * 1024 * 1024)]
                    {
                        let group: Vec<_> = controls
                            .resources
                            .iter()
                            .filter(|r| r.asset.handle >> 60 == namespace)
                            .collect();
                        assert!(group.len() <= slots);
                        assert!(group.iter().map(|r| r.image.pixels.len()).sum::<usize>() <= bytes);
                    }
                    assert!(
                        controls
                            .resources
                            .iter()
                            .map(|r| r.image.pixels.len())
                            .sum::<usize>()
                            <= 8 * 1024 * 1024
                    );
                    assert!(controls.frame.draws.len() + preview.draws.len() <= 1536);
                    // Prepared pictures retain resources across subsequent plans and retirement.
                    preview
                        .validate(&held.iter().map(|r| (r.asset.into(), r.asset)).collect())
                        .unwrap();
                    studio.presented();
                }
            }
        }
    }
    assert!(maximum > 100);
}

#[test]
fn settled_antenna_poses_fit_the_clear_region_without_clipping_or_truncation() {
    use slither_wgpu_slice::cosmetics::Cosmetics;
    let mut png = std::io::Cursor::new(vec![]);
    image::RgbaImage::from_pixel(16, 16, image::Rgba([140, 190, 100, 255]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let full = Cosmetics::load(|_| Ok(png.get_ref().clone()), None).unwrap();
    for skin in [24, 25, 37, 39, 42, 45, 46, 47, 48, 49, 59, 62] {
        let mut studio = SkinStudioNative::new(
            AppearanceRecord {
                builtin: skin,
                ..Default::default()
            },
            MenuStorage::at(None),
        )
        .unwrap();
        studio.open(true);
        for time in (0..=2000).step_by(16) {
            studio.step(f64::from(time));
            let (preview, resources) = studio.prepare_preview([960, 540], 1., Some(&full)).unwrap();
            let bulb = full
                .role(full.skin(skin, &[]).unwrap().antenna.unwrap().source)
                .unwrap();
            assert!(
                preview.draws.iter().any(|d| d.asset == bulb),
                "skin {skin} omitted ready antenna"
            );
            let bounds = StudioState::preview_bounds([960., 540.]);
            for quad in preview
                .quads(
                    &resources
                        .iter()
                        .map(|r| (r.asset.into(), r.asset))
                        .collect(),
                )
                .unwrap()
            {
                for vertex in quad.vertices {
                    let x = f64::from(vertex[0] + 1.) * 480.;
                    let y = f64::from(1. - vertex[1]) * 270.;
                    assert!(
                        x >= bounds.x - 0.01
                            && x <= bounds.x + bounds.width + 0.01
                            && y >= bounds.y - 0.01
                            && y <= bounds.y + bounds.height + 0.01,
                        "skin {skin} at {time}: ({x},{y}) outside {bounds:?}"
                    );
                }
            }
            studio.presented();
        }
    }
}
