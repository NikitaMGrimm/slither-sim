//! Authored synthetic production pictures; no window, GPU, socket or reference assets.
#![cfg(windows)]
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_wgpu_slice::{game::LocalGame, render};

fn snapshot(game: &mut LocalGame, identity: u64) -> render::PreparedFrame {
    let mut world = game.client().clone();
    let quality = game.quality();
    let extent = render::backing_size([960., 540.]).unwrap();
    let p = render::prepare_gameplay(&mut world, &mut game.bank, extent, 1., identity, quality)
        .unwrap();
    let mut hud = game.hud_view();
    hud.score = "12345".into();
    hud.opacity.readout = 1.;
    hud.opacity.header = 0.85;
    hud.opacity.rows = 1.;
    render::finish_hud(p, &mut game.bank, &hud).unwrap()
}

#[test]
fn retired_storage_reuses_world_and_overlay_without_changing_an_issued_retry() {
    let mut game = tape::playing();
    game.window_viewport([960, 540], 1.).unwrap();
    game.begin_redraw(40., [960, 540], 1.).unwrap();
    tape::effects(&mut game);
    let retained = game.take_frame().unwrap();
    let original = retained.frame.clone();
    let original_overlay = retained.overlay.clone();
    game.retry_frame(retained).unwrap();

    let retired = snapshot(&mut game, 100);
    let pointers = [
        retired.frame.draws.as_ptr(),
        retired.overlay.as_ref().unwrap().draws.as_ptr(),
    ];
    let expected = snapshot(&mut game, 101);
    game.bank.recycle_frame(retired);
    let reused = snapshot(&mut game, 101);
    assert_eq!(reused.frame, expected.frame);
    assert_eq!(reused.overlay, expected.overlay);
    assert_eq!(reused.frame.draws.as_ptr(), pointers[0]);
    assert_eq!(reused.overlay.as_ref().unwrap().draws.as_ptr(), pointers[1]);

    let retry = game.take_frame().unwrap();
    assert_eq!(retry.frame, original);
    assert_eq!(retry.overlay, original_overlay);
    game.presented(true).unwrap();
    game.bank.recycle_frame(retry);
}

#[test]
fn physical_hud_keeps_css_anchors_and_leaves_world_quads_and_pixels_unchanged() {
    let mut game = tape::negotiating();
    let mut named = tape::snake(44);
    named[22] = 4;
    named.splice(23..23, b"Name".iter().copied());
    game.receive(
        game.generation(),
        &tape::bundle(&[
            tape::setup(),
            named,
            tape::board(),
            vec![b'm', 0, 0, 1, 255, 255, 255, 1, b'R', b'H', b'i'],
            vec![b'M', 0, 8, 64],
        ]),
        3.,
    )
    .unwrap();
    tape::pictures(&mut game);
    tape::pump(&mut game, 20.);
    let before = snapshot(&mut game, 100);
    game.window_viewport([3840, 2160], 1.5).unwrap();
    let after = snapshot(&mut game, 100);
    let overlay = after.overlay.as_ref().unwrap();
    assert_eq!(after.frame.extent, [1500, 845]);
    assert_eq!(after.frame.viewport_scale, 1.);
    assert_eq!(overlay.extent, [3840, 2160]);
    assert_eq!(overlay.viewport_scale, 1.5);
    assert!(overlay.snakes.is_empty());
    assert_eq!(after.frame.snakes, before.frame.snakes);
    assert_eq!(
        after.frame.draws,
        before.frame.draws[..after.frame.draws.len()]
    );
    for draw in &after.frame.draws {
        let source = |p: &render::PreparedFrame| {
            p.resources
                .iter()
                .find(|r| r.asset == draw.asset)
                .unwrap()
                .image
                .pixels
                .clone()
        };
        assert_eq!(source(&after), source(&before));
    }
    assert!(
        after
            .frame
            .draws
            .iter()
            .any(|d| d.asset.handle >> 60 == 4 && d.translation != [0., 0.])
    );
    let map = overlay
        .draws
        .iter()
        .find(|d| d.asset.handle >> 60 == 5)
        .unwrap();
    assert_eq!(map.translation, [2560. - 16. - 32., 1440. - 16. - 32.]);
    assert!(overlay.draws.iter().any(|d| d.destination.x >= 8.
        && d.destination.x < 308.
        && d.destination.y >= 4.
        && d.destination.y < 232.));
    // The first readout span sits at the original 8 CSS px left inset. Its
    // source/destination ratio is physical DPR, independently of the world cap.
    let length = overlay
        .draws
        .iter()
        .find(|d| d.destination.x == 8. && d.destination.y > 1300.)
        .unwrap();
    let crop = length.source.unwrap();
    assert!((crop.width / length.destination.width - 1.5).abs() < 1e-12);
    assert!((crop.height / length.destination.height - 1.5).abs() < 1e-12);
    assert!(length.destination.y + length.destination.height <= 1440. - 4.);
    let header = overlay
        .draws
        .iter()
        .find(|d| d.destination.y < 32. && d.destination.x > 2200.)
        .unwrap();
    assert!(header.destination.x + header.destination.width <= 2560. - 4.);
    let resources = after
        .resources
        .iter()
        .map(|r| (r.asset.into(), r.asset))
        .collect();
    after.frame.validate(&resources).unwrap();
    overlay.validate(&resources).unwrap();
}

#[test]
fn fixed_scale_reuses_text_rasters_across_frames_and_window_resize() {
    let mut game = tape::playing();
    game.window_viewport([3840, 2160], 1.5).unwrap();
    let first = snapshot(&mut game, 100);
    let second = snapshot(&mut game, 101);
    game.window_viewport([3600, 2025], 1.5).unwrap();
    let resized = snapshot(&mut game, 102);
    let text_assets = |p: &render::PreparedFrame| {
        p.overlay
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .filter(|d| d.asset.handle >> 60 == 4)
            .map(|d| d.asset)
            .collect::<Vec<_>>()
    };
    assert!(!text_assets(&first).is_empty());
    assert_eq!(text_assets(&first), text_assets(&second));
    for resource in &first.resources {
        if let Some(next) = second.resources.iter().find(|r| r.asset == resource.asset) {
            assert!(std::sync::Arc::ptr_eq(&resource.image, &next.image));
        }
    }
    let cloned = first.clone();
    for (original, copy) in first.resources.iter().zip(&cloned.resources) {
        assert!(std::sync::Arc::ptr_eq(&original.image, &copy.image));
    }
    assert_eq!(text_assets(&first), text_assets(&resized));
    game.window_viewport([3840, 2160], 1.).unwrap();
    let scale_changed = snapshot(&mut game, 103);
    assert_ne!(text_assets(&first), text_assets(&scale_changed));
    assert!(text_assets(&first)[0].height > text_assets(&scale_changed)[0].height);
}
#[test]
fn fractional_display_scales_keep_hud_text_on_physical_texels() {
    let mut game = tape::playing();
    for dpr in [1.25, 1.5, 1.75] {
        game.window_viewport([3840, 2160], dpr).unwrap();
        let picture = snapshot(&mut game, 100);
        let text: Vec<_> = picture
            .overlay
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .filter(|d| d.asset.handle >> 60 == 4)
            .collect();
        assert!(!text.is_empty());
        for d in text {
            let s = d.source.unwrap();
            for v in [
                (d.destination.x + d.translation[0]) * dpr,
                (d.destination.y + d.translation[1]) * dpr,
            ] {
                assert!((v - v.round()).abs() < 1e-9);
            }
            assert!((d.destination.width * dpr - s.width).abs() < 1e-9);
            assert!((d.destination.height * dpr - s.height).abs() < 1e-9);
        }
    }
}

#[test]
fn high_dpr_hud_and_login_keep_exact_rasters_within_texture_budgets() {
    use slither_wgpu_slice::{hud, login_native::NativeLogin};
    let physical = [6400, 3600];
    let mut game = tape::playing();
    // DPR 5 covers Windows' maximum; DPR 8 crosses the former 128px guard
    // with the actual largest HUD span (the 21px leaderboard heading).
    for dpr in [5., 8.] {
        game.window_viewport(physical, dpr).unwrap();
        let plan = hud::plan(&game.hud_view(), physical, dpr).unwrap();
        assert_eq!(
            plan.lines
                .iter()
                .flat_map(|l| &l.spans)
                .map(|s| s.style.size)
                .fold(0_f64, f64::max),
            21.
        );
        let picture = snapshot(&mut game, 100);
        let resident = picture
            .resources
            .iter()
            .map(|r| (r.asset.into(), r.asset))
            .collect();
        picture.frame.validate(&resident).unwrap();
        let overlay = picture.overlay.as_ref().unwrap();
        overlay.validate(&resident).unwrap();
        for draw in overlay.draws.iter().filter(|d| d.asset.handle >> 60 == 4) {
            let source = draw.source.unwrap();
            assert!((draw.destination.width * dpr - source.width).abs() < 1e-9);
            assert!((draw.destination.height * dpr - source.height).abs() < 1e-9);
        }
    }
    let mut login = NativeLogin::new("Player").unwrap();
    login.step(0., true, false).unwrap();
    let menu = login.prepare_host(physical, 5., true, None).unwrap();
    let resident = menu
        .resources
        .iter()
        .map(|r| (r.asset.into(), r.asset))
        .collect();
    menu.frame.validate(&resident).unwrap();
    let widest = menu
        .resources
        .iter()
        .filter(|r| r.asset.handle >> 60 == 7)
        .max_by_key(|r| r.asset.width)
        .unwrap();
    assert_eq!(widest.asset.width, 1640);
    assert!(
        menu.resources
            .iter()
            .all(|r| r.asset.width <= 2048 && r.asset.height <= 2048)
    );
    assert!(
        menu.resources
            .iter()
            .map(|r| r.image.pixels.len())
            .sum::<usize>()
            <= slither_wgpu_slice::text::MENU_CACHE_BYTES
    );
}

#[test]
fn login_clone_reserves_final_draw_storage_and_preserves_retry_picture() {
    use slither_wgpu_slice::login_native::NativeLogin;
    for separate_overlay in [false, true] {
        let mut game = tape::playing();
        let extent = game.window_viewport([960, 540], 1.).unwrap();
        game.begin_redraw(40., extent, 1.).unwrap();
        tape::effects(&mut game);
        let mut picture = game.take_frame().unwrap();
        if !separate_overlay {
            picture
                .frame
                .draws
                .extend(picture.overlay.take().unwrap().draws);
        }
        picture
            .frame
            .draws
            .resize(29_804, picture.frame.draws[0].clone());
        let original = picture.frame.clone();
        let original_overlay = picture.overlay.clone();
        let ui_extent = picture.ui_frame().extent;
        let ui_scale = picture.ui_frame().viewport_scale;
        let mut login = NativeLogin::new("Player").unwrap();
        let mut expected = picture.clone();
        login
            .prepare_host(ui_extent, ui_scale, true, None)
            .unwrap()
            .append_to(&mut expected)
            .unwrap();
        let composed = login
            .prepare_host(ui_extent, ui_scale, true, None)
            .unwrap()
            .compose_over(&picture)
            .unwrap();
        assert_eq!(composed.frame, expected.frame);
        assert_eq!(composed.overlay, expected.overlay);
        assert_eq!(composed.frame.draws.capacity(), composed.frame.draws.len());
        if let Some(overlay) = &composed.overlay {
            assert_eq!(overlay.draws.capacity(), overlay.draws.len());
        }
        assert_eq!(
            composed
                .resources
                .iter()
                .map(|r| r.asset)
                .collect::<Vec<_>>(),
            expected
                .resources
                .iter()
                .map(|r| r.asset)
                .collect::<Vec<_>>()
        );
        game.retry_frame(picture).unwrap();
        let retry = game.take_frame().unwrap();
        assert_eq!(retry.frame, original);
        assert_eq!(retry.overlay, original_overlay);
        game.presented(true).unwrap();
    }
}

#[test]
fn setup_retry_and_resize_keep_immutable_overlay_with_its_picture() {
    let mut game = tape::negotiating();
    let backing = game.window_viewport([3840, 2160], 1.5).unwrap();
    game.receive(
        game.generation(),
        &tape::bundle(&[tape::setup(), tape::snake(44), tape::board()]),
        3.,
    )
    .unwrap();
    let setup = game.take_frame().unwrap();
    assert_eq!(setup.overlay.as_ref().unwrap().extent, [3840, 2160]);
    let saved = setup.overlay.clone();
    game.retry_frame(setup).unwrap();
    assert_eq!(game.take_frame().unwrap().overlay, saved);
    game.presented(true).unwrap();
    game.begin_redraw(20., backing, 1.).unwrap();
    tape::effects(&mut game);
    let current = game.take_frame().unwrap();
    assert_eq!(current.overlay.as_ref().unwrap().extent, [3840, 2160]);
    game.retry_frame(current).unwrap();
    // Same world backing, different physical/CSS viewport: rebuild queued work.
    assert_eq!(game.window_viewport([1920, 1080], 1.).unwrap(), backing);
    let resized = game.take_frame().unwrap();
    assert_eq!(resized.overlay.as_ref().unwrap().extent, [1920, 1080]);
    assert_eq!(resized.overlay.as_ref().unwrap().viewport_scale, 1.);
    assert_eq!(saved.unwrap().extent, [3840, 2160]);
    game.presented(true).unwrap();
}

#[test]
fn zero_window_suspends_and_restoring_the_same_size_resumes_frames() {
    let mut game = tape::playing();
    let physical = [960, 540];
    let backing = game.window_viewport(physical, 1.).unwrap();
    game.begin_redraw(30., backing, 1.).unwrap();
    tape::effects(&mut game);
    let pending = game.take_frame().unwrap();
    game.retry_frame(pending).unwrap();

    assert_eq!(game.window_viewport([0, 0], 1.).unwrap(), [0, 0]);
    assert!(game.take_frame().is_none());

    assert_eq!(game.window_viewport(physical, 1.).unwrap(), backing);
    let restored = game.take_frame().unwrap();
    assert_eq!(restored.frame.extent, backing);
    assert_eq!(restored.overlay.as_ref().unwrap().extent, physical);
    game.presented(true).unwrap();

    game.begin_redraw(50., backing, 1.).unwrap();
    tape::effects(&mut game);
    assert!(game.take_frame().is_some());
    game.presented(true).unwrap();
}

#[test]
fn login_crossfade_covers_complete_world_and_hud_on_the_physical_overlay() {
    use slither_wgpu_slice::login_native::NativeLogin;
    let mut game = tape::playing();
    let physical = [3840, 2160];
    game.window_viewport(physical, 1.5).unwrap();
    let mut picture = snapshot(&mut game, 100);
    let world = picture.frame.draws.clone();
    let hud = picture.overlay.as_ref().unwrap().draws.clone();
    let mut login = NativeLogin::new("Player").unwrap();
    login
        .state
        .action(slither_client::start_page::MenuAction::Play, true);
    login.state.step(0., false).unwrap();
    login.state.begin_outgoing();
    login.state.step(125., false).unwrap();
    let menu = login.prepare_host(physical, 1.5, false, None).unwrap();
    assert_eq!(login.state.view(false).world_alpha, 0.25);
    let page_asset = menu.backdrop.as_ref().unwrap().asset;
    menu.append_to(&mut picture).unwrap();

    let page = &picture.frame.draws[0];
    assert_eq!(page.asset, page_asset);
    assert_eq!(page.effective_alpha, 1.);
    assert_eq!((page.destination.x, page.destination.y), (0., 0.));
    assert_eq!(
        (page.destination.width, page.destination.height),
        (1500., 845.)
    );
    assert_eq!(picture.frame.draws[1..], world);
    let overlay = picture.overlay.as_ref().unwrap();
    assert_eq!(overlay.draws[1..=hud.len()], hud);
    let cover = &overlay.draws[0];
    assert_eq!(cover.asset, page_asset);
    assert_eq!(cover.effective_alpha, 0.75);
    assert_eq!((cover.destination.x, cover.destination.y), (0., 0.));
    assert_eq!(
        (cover.destination.width, cover.destination.height),
        (2560., 1440.)
    );
    assert_eq!(
        picture
            .resources
            .iter()
            .find(|r| r.asset == page_asset)
            .unwrap()
            .image
            .pixels,
        [22, 28, 34, 255]
    );
    let resident = picture
        .resources
        .iter()
        .map(|r| (r.asset.into(), r.asset))
        .collect();
    picture.frame.validate(&resident).unwrap();
    overlay.validate(&resident).unwrap();
}

#[test]
fn webgpu_midpoint_cover_precedes_the_independently_faded_hud() {
    let mut game = tape::playing();
    game.bank.overlay_viewport = Some(([960, 540], 1.));
    let mut picture = snapshot(&mut game, 100);
    let world = picture.frame.draws.clone();
    let overlay = picture.overlay.as_mut().unwrap();
    for draw in &mut overlay.draws {
        draw.effective_alpha = 0.5;
    }
    let hud = overlay.draws.clone();
    assert!(!hud.is_empty());
    slither_wgpu_slice::menu::MenuFrame::world_crossfade([960, 540], 1., 0.5)
        .unwrap()
        .append_to(&mut picture)
        .unwrap();
    assert_eq!(picture.frame.draws[1..], world);
    assert_eq!(picture.frame.draws[0].effective_alpha, 1.);
    let overlay = picture.overlay.as_ref().unwrap();
    assert_eq!(overlay.draws[0].effective_alpha, 0.5);
    assert_eq!(overlay.draws[1..], hud);
    assert!(overlay.draws[1..].iter().all(|d| d.effective_alpha == 0.5));
    let resident = picture
        .resources
        .iter()
        .map(|r| (r.asset.into(), r.asset))
        .collect();
    picture.frame.validate(&resident).unwrap();
    overlay.validate(&resident).unwrap();
}
