//! Authored synthetic properties; no windows, sockets, GPU or exact-pixel oracle.
#[path = "support/zoom_world.rs"]
mod dense;
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::{presentation::*, session_policy::Phase, state::Point};
use slither_wgpu_slice::{frame::MAX_DRAWS, game::LocalGame, render};

#[test]
fn no_wheel_preserves_stock_pictures_and_command_bytes() {
    let mut stock = tape::playing();
    let mut zero = tape::playing();
    assert!(!zero.wheel_zoom(0.));
    assert!(!zero.wheel_zoom(f64::NAN));
    for time in [60., 100., 140.] {
        for game in [&mut stock, &mut zero] {
            game.pointer(850., 130., 960., 540., false).unwrap();
            game.begin_frame(time, [960, 540], 1.).unwrap();
        }
        assert_eq!(tape::effects(&mut stock), tape::effects(&mut zero));
        let a = stock.take_frame().unwrap();
        let b = zero.take_frame().unwrap();
        assert_eq!(a.frame, b.frame);
        assert_eq!(a.overlay, b.overlay);
        stock.presented(true).unwrap();
        zero.presented(true).unwrap();
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

#[test]
fn zoomed_steering_uses_the_displayed_transform() {
    for notches in [-8., 2.] {
        let mut game = tape::playing();
        assert!(game.wheel_zoom(notches));
        tape::pump(&mut game, 60.);
        for (index, [x, y]) in [[800., 100.], [200., 420.], [850., 400.]]
            .into_iter()
            .enumerate()
        {
            let v = game.client().control_view();
            // The followed head is at the view centre; inverse uniform scale
            // changes target distance, not heading or the stock CSS dead zone.
            let dx = (x - 480.) / v.zoom;
            let dy = (y - 270.) / v.zoom;
            let angle = dy.atan2(dx).rem_euclid(std::f64::consts::TAU);
            let expected = (251. * angle / std::f64::consts::TAU).floor() as u8;
            game.pointer(x, y, 960., 540., false).unwrap();
            let packets = tape::pump(&mut game, 110. + index as f64 * 50.);
            assert!(
                packets.contains(&vec![expected]),
                "{packets:?}, expected {expected}"
            );
            // The explicit non-following policy must use that displayed scale
            // for camera/head offsets, too. Check its inverse world projection.
            let head = v.own.unwrap().raw_head;
            let projected = slither_client::desktop_input::project_pointer(&[
                x, y, 960., 540., 0., 1., v.camera.x, v.camera.y, head.x, head.y, v.zoom,
            ])
            .unwrap();
            let world_dx = v.camera.x + dx - head.x;
            let world_dy = v.camera.y + dy - head.y;
            assert!((projected.gaze_heading - world_dy.atan2(world_dx)).abs() < 1e-12);
        }
    }
}

#[test]
fn zoom_preserves_following_controls_across_dprs_and_viewports() {
    for (physical, dpr) in [([1920, 1080], 1.), ([1920, 1080], 2.), ([900, 1600], 1.25)] {
        let mut game = tape::playing();
        let mut stock = tape::playing();
        assert!(game.wheel_zoom(-5.));
        let extent = game.window_viewport(physical, dpr).unwrap();
        stock.window_viewport(physical, dpr).unwrap();
        for current in [&mut game, &mut stock] {
            current.begin_frame(60., extent, 1.).unwrap();
            tape::effects(current);
            tape::pictures(current);
        }
        let [width, height] = physical.map(|n| f64::from(n) / dpr);
        let x = width * 0.85;
        let y = height * 0.2;
        for current in [&mut game, &mut stock] {
            current.pointer(x, y, width, height, false).unwrap();
            current.begin_frame(110., extent, 1.).unwrap();
        }
        assert_eq!(tape::effects(&mut game), tape::effects(&mut stock));
        tape::pictures(&mut game);
        tape::pictures(&mut stock);
        assert!(game.client().camera().user_zoom_factor() < 1.);
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
    assert_eq!(plan.stroke.len(), 42);
    for point in plan.stroke {
        assert!(((point.x - 480.).hypot(point.y - 270.) - 230.).abs() < 1e-8);
    }
}
