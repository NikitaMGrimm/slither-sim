//! Authored synthetic properties; no windows, sockets, GPU or exact-pixel oracle.
#[path = "support/zoom_world.rs"]
mod dense;
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::{presentation::*, session_policy::Phase, state::Point};
use slither_wgpu_slice::{frame::MAX_DRAWS, game::LocalGame, render};

// Native trig/Debug float text is platform-specific. This independent golden was
// captured on the product Windows target at the source revision in the fixture;
// the cross-revision feel comparator separately checks numeric presentation.
#[cfg(windows)]
#[test]
fn factor_one_matches_pinned_main_pictures_and_command_bytes() {
    let pin: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/zoom-stock-main.json")).unwrap();
    let mut game = tape::playing();
    assert!(!game.wheel_zoom(0.));
    assert!(!game.wheel_zoom(f64::NAN));
    for row in pin["rows"].as_array().unwrap() {
        let time = row["timeMs"].as_f64().unwrap();
        game.pointer(850., 130., 960., 540., false).unwrap();
        game.begin_frame(time, [960, 540], 1.).unwrap();
        assert_eq!(
            serde_json::to_value(tape::effects(&mut game)).unwrap(),
            row["packets"]
        );
        let picture = game.take_frame().unwrap();
        assert_eq!(
            picture.draw_count(),
            row["draws"].as_u64().unwrap() as usize
        );
        let debug = format!("{:?}\n{:?}", picture.frame, picture.overlay);
        let hash = debug.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        assert_eq!(
            format!("{hash:016x}"),
            row["frameFnv1a64"].as_str().unwrap()
        );
        game.presented(true).unwrap();
    }
}

#[test]
fn wheel_round_trip_restores_stock_pictures_and_command_bytes() {
    for notches in [0.25, 1., 2.] {
        let mut stock = tape::playing();
        let mut adjusted = tape::playing();
        assert!(adjusted.wheel_zoom(-notches));
        assert!(adjusted.wheel_zoom(notches));
        for time in [60., 110.] {
            for game in [&mut stock, &mut adjusted] {
                game.pointer(850., 130., 960., 540., false).unwrap();
                game.begin_frame(time, [960, 540], 1.).unwrap();
            }
            assert_eq!(tape::effects(&mut stock), tape::effects(&mut adjusted));
            let a = stock.take_frame().unwrap();
            let b = adjusted.take_frame().unwrap();
            assert_eq!(a.frame, b.frame);
            stock.presented(true).unwrap();
            adjusted.presented(true).unwrap();
        }
    }
}

#[test]
fn menus_and_dead_sessions_do_not_zoom_and_new_launch_is_default() {
    let mut menu = LocalGame::new(0.).unwrap();
    assert!(!menu.wheel_zoom(-4.));
    assert!(!menu.client().camera().user_zoom_active());
    let mut opening = tape::negotiating();
    assert!(!opening.wheel_zoom(-4.));
    assert!(!opening.client().camera().user_zoom_active());
    let mut game = tape::playing();
    #[cfg(windows)]
    {
        let login = slither_wgpu_slice::login_native::NativeLogin::new("Player").unwrap();
        // A login overlay retains its wheel even when a live world is underneath.
        assert!(!login.wheel_zoom(&mut game, -4.));
        assert!(!game.client().camera().user_zoom_active());
    }
    game.receive(game.generation(), &tape::bundle(&[vec![b'v', 0]]), 30.)
        .unwrap();
    assert!(!game.wheel_zoom(-4.));
}

#[test]
fn zoom_survives_death_reset_respawn_and_an_outstanding_picture() {
    let mut game = tape::playing();
    game.begin_frame(60., [960, 540], 1.).unwrap();
    tape::effects(&mut game);
    let immutable = game.take_frame().unwrap();
    assert!(game.wheel_zoom(-4.));
    game.retry_frame(immutable.clone()).unwrap();
    let retry = game.take_frame().unwrap();
    assert_eq!(retry.frame, immutable.frame);
    game.presented(true).unwrap();
    assert!(game.client().camera().user_zoom_active());
    game.receive(game.generation(), &tape::bundle(&[vec![b'v', 0]]), 70.)
        .unwrap();
    game.stop();
    tape::effects(&mut game);
    tape::pump(&mut game, 80.);
    assert!(game.can_play());
    assert!(game.client().camera().user_zoom_active());
    game.play("ws://127.0.0.1:19491/slither", "Player", 44)
        .unwrap();
    tape::pump(&mut game, 90.);
    tape::effects(&mut game);
    game.opened(game.generation(), 91.).unwrap();
    tape::effects(&mut game);
    game.receive(game.generation(), &tape::bundle(&[tape::challenge()]), 92.)
        .unwrap();
    tape::effects(&mut game);
    game.receive(
        game.generation(),
        &tape::bundle(&[tape::setup(), tape::snake(44)]),
        93.,
    )
    .unwrap();
    tape::pictures(&mut game);
    tape::pump(&mut game, 110.);
    assert_eq!(game.phase(), Phase::Playing);
    assert!(game.client().camera().user_zoom_active());
    assert!(game.client().camera().user_zoom_factor() < 1.);
}

fn displayed_head(game: &LocalGame, css: [f64; 2], extent: [u32; 2]) -> [f64; 2] {
    let view = game.client().control_view();
    let head = game.client().own_snake().unwrap().metrics().corrected_head;
    [
        css[0] / 2. + (head.x - view.camera.x) * view.zoom * css[0] / f64::from(extent[0]),
        css[1] / 2. + (head.y - view.camera.y) * view.zoom * css[1] / f64::from(extent[1]),
    ]
}

fn heading(packets: &[Vec<u8>]) -> u8 {
    packets.iter().find(|p| p.len() == 1 && p[0] < 251).unwrap()[0]
}

#[test]
fn zoomed_steering_starts_at_the_corrected_displayed_head_without_changing_cadence() {
    use std::f64::consts::{PI, TAU};
    for notches in [-8., -3., 2.] {
        for (physical, dpr) in [([960, 540], 1.), ([1920, 1080], 2.), ([900, 1600], 1.25)] {
            let mut game = tape::playing();
            assert!(game.wheel_zoom(notches));
            let extent = game.window_viewport(physical, dpr).unwrap();
            // Absolute anchor jump installs independent head/camera correction
            // rings. The displayed head is then measurably off centre.
            let mut anchor = vec![b'=', 0, 7, 0, 0];
            anchor.extend(1200u16.to_be_bytes());
            anchor.extend(1100u16.to_be_bytes());
            game.receive(game.generation(), &tape::bundle(&[anchor]), 40.)
                .unwrap();
            game.begin_frame(60., extent, 1.).unwrap();
            tape::effects(&mut game);
            tape::pictures(&mut game);
            let mut anchor = vec![b'=', 0, 7, 0, 0];
            anchor.extend(1240u16.to_be_bytes());
            anchor.extend(1140u16.to_be_bytes());
            game.receive(game.generation(), &tape::bundle(&[anchor]), 80.)
                .unwrap();
            game.begin_frame(90., extent, 1.).unwrap();
            tape::effects(&mut game);
            tape::pictures(&mut game);
            let css = physical.map(|n| f64::from(n) / dpr);
            assert_ne!(game.client().camera().user_zoom_factor(), 1.);
            let head = displayed_head(&game, css, extent);
            assert!((head[0] - css[0] / 2.).hypot(head[1] - css[1] / 2.) > 0.1);
            // Seed a different heading so the rightward command cannot be
            // suppressed as identical to the initial zero heading.
            game.pointer(head[0] - 25., head[1], css[0], css[1], false)
                .unwrap();
            game.begin_frame(110., extent, 1.).unwrap();
            assert!(
                (f64::from(heading(&tape::effects(&mut game))) * TAU / 251. - PI).abs()
                    < TAU / 251.
            );
            tape::pictures(&mut game);
            for (index, angle) in [0_f64, -1.3, 2.1].into_iter().enumerate() {
                let now = 110. + index as f64 * 50.;
                let head = displayed_head(&game, css, extent);
                game.pointer(
                    head[0] + 25. * angle.cos(),
                    head[1] + 25. * angle.sin(),
                    css[0],
                    css[1],
                    false,
                )
                .unwrap();
                game.begin_frame(now + 10., extent, 1.).unwrap();
                let early = tape::effects(&mut game);
                assert!(
                    !early.iter().any(|p| p.len() == 1 && p[0] < 251),
                    "cadence changed: {early:?}"
                );
                tape::pictures(&mut game);
                game.begin_frame(now + 34., extent, 1.).unwrap();
                let packets = tape::effects(&mut game);
                let sent = f64::from(heading(&packets)) * TAU / 251.;
                let error = (sent - angle + PI).rem_euclid(TAU) - PI;
                assert!(
                    error.abs() <= TAU / 251. + 1e-10,
                    "{packets:?}: {error} at factor {}",
                    game.client().camera().user_zoom_factor()
                );
                tape::pictures(&mut game);
            }
        }
    }
}

#[test]
fn maximum_zoom_out_fits_current_radius_and_preserves_draw_budgets() {
    let mut game = dense::world().unwrap();
    assert!(game.wheel_zoom(-1000.));
    let quality = game.quality();
    let mut world = game.client().clone();
    for index in 0..110 {
        let p = render::prepare_gameplay(
            &mut world,
            &mut game.bank,
            dense::EXTENT,
            1.,
            1000 + index,
            quality,
        )
        .unwrap();
        assert!(p.draw_count() <= MAX_DRAWS);
        if index == 109 {
            assert_eq!(p.frame.snakes.len(), 65, "LOD retains every snake");
            assert!(p.frame.snakes.iter().all(|s| s.body_commands > 0));
        }
        assert!(p.frame.draws.capacity() <= MAX_DRAWS);
        assert!(p.pixel_bytes() <= 64 * 1024 * 1024);
        game.bank.recycle_frame(p);
    }
    for radius in [11500., 18000., 6000.] {
        // The shared camera property is also exercised against real z packets.
        let mut z = vec![b'z'];
        z.extend(tape::u24(radius as u32));
        world
            .apply_frame(
                &tape::bundle(&[z]),
                slither_client::protocol::ElapsedHeader::Absent,
            )
            .unwrap();
        let p =
            render::prepare_gameplay(&mut world, &mut game.bank, dense::EXTENT, 1., 2000, quality)
                .unwrap();
        let setup = world.received().setup.as_ref().unwrap();
        let c = world.camera();
        for (side, position) in dense::EXTENT
            .into_iter()
            .zip([c.position().x, c.position().y])
        {
            assert!(
                2. * (radius + (position - setup.grid_center).abs()) * c.zoom()
                    <= f64::from(side) + 1e-8
            );
        }
        game.bank.recycle_frame(p);
    }
}

#[test]
fn full_arena_boundary_has_bounded_geometry_and_covers_the_circle() {
    let plan = plan_arena_edge_with_zoom(
        ArenaEdgeInput {
            view: ArenaEdgeView::new(12000., 0., 0.).unwrap(),
            camera: Point {
                x: 12000.,
                y: 12000.,
            },
            zoom: 0.02,
            width: 960,
            height: 540,
            radius: 11500.,
        },
        true,
    )
    .unwrap()
    .unwrap();
    assert!(plan.stroke().len() <= MAX_ARENA_EDGE_SEGMENTS + 2);
    for point in plan.stroke() {
        assert!(((point.x - 480.).hypot(point.y - 270.) - 230.).abs() < 1e-8);
    }
}
