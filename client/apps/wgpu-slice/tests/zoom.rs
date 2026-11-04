//! Authored synthetic properties; no windows, sockets, GPU or exact-pixel oracle.
#[path = "support/zoom_world.rs"]
mod dense;
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::{presentation::*, session_policy::Phase, state::Point};
use slither_wgpu_slice::{frame::MAX_DRAWS, game::LocalGame, render};

#[test]
fn crowded_wheel_views_keep_retained_food_and_frozen_retry_inputs() {
    for notches in [0., -1., -4., 0.065_f64.ln() / 1.12_f64.ln(), -1000.] {
        let mut game = tape::playing();
        let head = game.client().own_snake().unwrap().metrics().raw_head;
        let mut food = vec![b'F', (head.x / 100.) as u8, (head.y / 100.) as u8];
        for i in 0..1024_u16 {
            food.extend([7, (i % 256) as u8, (i / 256) as u8, 20]);
        }
        game.receive(game.generation(), &tape::bundle(&[food]), 30.)
            .unwrap();
        game.wheel_zoom(notches);
        for index in 0..24 {
            game.begin_frame(60. + f64::from(index) * 16., [1500, 844], 1.)
                .unwrap();
            tape::effects(&mut game);
            let picture = game.take_frame().unwrap();
            if index == 23 {
                assert!(picture.frame.food_batch.is_some(), "notches={notches}");
                let frozen = picture.frame.canonical().unwrap();
                game.wheel_zoom(2.);
                game.retry_frame(picture).unwrap();
                assert_eq!(
                    game.take_frame().unwrap().frame.canonical().unwrap(),
                    frozen
                );
            }
            game.presented(true).unwrap();
        }
    }
}

#[cfg(feature = "frame-bench")]
#[test]
fn benchmark_food_counts_follow_retained_batch_and_picture_retry() {
    let mut game = tape::playing();
    let mut packet = vec![b'F', 11, 10];
    for i in 0..1024_u16 {
        packet.extend([0, (i % 32) as u8, (i / 32) as u8, 20]);
    }
    game.receive(game.generation(), &tape::bundle(&[packet]), 40.)
        .unwrap();
    game.begin_frame(110., [960, 540], 1.).unwrap();
    tape::effects(&mut game);
    let picture = game.take_frame().unwrap();
    assert!(picture.frame.food_batch.is_some());
    let food = picture.food_submission.unwrap();
    assert!(!food.budget_applied);
    assert_eq!(food.counts.visible_foods, 1024);
    assert_eq!(food.counts.before_zoom_foods, 1024);
    assert_eq!(food.retained_pellets, 1025);
    let identity = picture.frame.identity;
    game.retry_frame(picture).unwrap();
    let retry = game.take_frame().unwrap();
    assert_eq!(retry.frame.identity, identity);
    assert_eq!(retry.food_submission, Some(food));
    game.presented(true).unwrap();
}

#[cfg(feature = "frame-bench")]
#[test]
fn benchmark_far_zoom_submits_every_candidate_and_freezes_census() {
    let mut game = tape::playing();
    let mut radius = vec![b'z'];
    radius.extend(tape::u24(10000));
    let mut food = vec![b'F', 11, 10];
    for i in 0..1000_u16 {
        food.extend([
            0,
            (i % 32) as u8,
            (i / 32) as u8,
            if i < 200 { 100 } else { 20 },
        ]);
    }
    let mut outside = vec![b'F', 200, 200];
    for i in 0..24_u8 {
        outside.extend([0, i, 0, 100]);
    }
    game.receive(
        game.generation(),
        &tape::bundle(&[radius, vec![b'c', 41, 4, 3, 232], food, outside]),
        40.,
    )
    .unwrap();
    assert_eq!(game.client().foods().foods().len(), 1024);
    assert!(game.wheel_zoom(-20.));
    for i in 0..120 {
        game.begin_frame(60. + f64::from(i) * 16., [1500, 844], 1.)
            .unwrap();
        tape::effects(&mut game);
        let picture = game.take_frame().unwrap();
        if i == 119 {
            assert!(
                game.client()
                    .foods()
                    .foods()
                    .iter()
                    .all(|f| f.snapshot().scale == 1.)
            );
            let batch = picture
                .frame
                .food_batch
                .as_ref()
                .expect("continuous zoom must use the retained path");
            let food = picture.food_submission.unwrap();
            assert!(!food.budget_applied);
            assert_eq!(food.retained_pellets, 1024);
            assert_eq!(food.counts.before_zoom_foods, 1000);
            assert_eq!(food.counts.visible_foods, 1000);
            assert_eq!(batch.draw_count(), 1000 * 3);
            let canonical = picture.frame.canonical().unwrap();
            let identity = picture.frame.identity;
            game.retry_frame(picture).unwrap();
            let retry = game.take_frame().unwrap();
            assert_eq!(retry.frame.identity, identity);
            assert_eq!(retry.frame.canonical().unwrap(), canonical);
            assert_eq!(retry.food_submission, Some(food));
        }
        game.presented(true).unwrap();
    }
}

#[cfg(feature = "frame-bench")]
#[test]
fn pressure_food_admission_is_continuous_across_factor_one() {
    let mut game = tape::playing();
    let mut packets = Vec::new();
    for id in 8..264_u16 {
        let mut snake = tape::snake((id % 9) as u8);
        snake[1..3].copy_from_slice(&id.to_be_bytes());
        packets.push(snake);
    }
    for chunk in 0..5_u16 {
        let mut food = vec![b'F', 11, 10];
        for i in chunk * 1000..(chunk + 1) * 1000 {
            food.extend([0, ((i % 80) * 3) as u8, ((i / 80) * 3) as u8, 20]);
        }
        packets.push(food);
    }
    game.receive(game.generation(), &tape::bundle(&packets), 40.)
        .unwrap();
    tape::pictures(&mut game);
    assert_eq!(game.client().received().snakes.len(), 257);
    assert!(game.client().detail_pressure());
    let food_handles: std::collections::HashSet<_> = game
        .client()
        .foods()
        .foods()
        .iter()
        .flat_map(|f| {
            let s = f.snapshot().sources;
            [s.outline, s.body, s.inner, s.outer].map(|s| s.asset().handle)
        })
        .collect();
    let quality = game.quality();
    let mut stock: Option<Vec<slither_wgpu_slice::frame::Draw>> = None;
    for requested in [1_f64, 1. - 1e-12, 1. - 1e-6, 1. - 1e-5] {
        let mut world = game.client().clone();
        world.wheel_zoom(requested.ln() / 1.12_f64.ln());
        let picture =
            render::prepare_gameplay(&mut world, &mut game.bank, [1500, 844], 1., 100, quality)
                .unwrap();
        let factor = world.camera().user_zoom_factor();
        assert!(factor >= requested && factor <= 1.);
        let food = picture.food_submission.unwrap();
        assert!(food.budget_applied, "pressure must stay active at {factor}");
        assert_eq!(food.counts.visible_foods, 4681);
        assert_eq!(food.counts.before_zoom_foods, 4681);
        assert!(picture.draw_count() <= MAX_DRAWS);
        let draws: Vec<_> = picture
            .frame
            .draws
            .iter()
            .filter(|d| food_handles.contains(&d.asset.handle))
            .cloned()
            .collect();
        assert_eq!(draws.len(), food.counts.visible_foods * 7);
        if let Some(stock) = &stock {
            for (a, b) in draws.iter().zip(stock) {
                assert_eq!(a.asset, b.asset);
                assert_eq!(a.source, b.source);
                assert_eq!(a.blend, b.blend);
                for (x, y) in [
                    (a.destination.x, b.destination.x),
                    (a.destination.y, b.destination.y),
                    (a.destination.width, b.destination.width),
                    (a.destination.height, b.destination.height),
                    (a.effective_alpha, b.effective_alpha),
                ] {
                    assert!((x - y).abs() < 0.01, "food jump at {factor}: {x} vs {y}");
                }
            }
        } else {
            stock = Some(draws);
        }
        game.bank.recycle_frame(picture);
    }
}

// Native trig/Debug float text is platform-specific. This independent golden was
// captured on Windows and paired with a pristine-main projection excluding only
// the removed background draws. The original full-frame fixture remains unchanged.
#[cfg(windows)]
#[test]
fn factor_one_stationary_and_repeated_cursor_preserve_entity_pictures_and_commands() {
    let pin: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/zoom-stock-black.json")).unwrap();
    for stationary in [false, true] {
        let mut game = tape::playing();
        assert!(!game.wheel_zoom(0.));
        assert!(!game.wheel_zoom(f64::NAN));
        for (index, row) in pin["rows"].as_array().unwrap().iter().enumerate() {
            let time = row["timeMs"].as_f64().unwrap();
            if !stationary || index == 0 {
                game.pointer(850., 130., 960., 540., false).unwrap();
            }
            // Host observations are not admitted pointer callbacks.
            game.observe_cursor(100. + index as f64, 420., 960., 540.)
                .unwrap();
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
            let debug = format!(
                "{:?}\n{:?}",
                picture.frame.canonical().unwrap(),
                picture.overlay
            )
            .replace(", food_batch: None", "");
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

fn inject_camera_corrections(game: &mut LocalGame, extent: [u32; 2]) {
    // Separate head/camera rings advance between the two absolute anchors.
    for (received, x, y, drawn) in [(40., 1200u16, 1100u16, 60.), (80., 1240, 1140, 90.)] {
        let mut anchor = vec![b'=', 0, 7, 0, 0];
        anchor.extend(x.to_be_bytes());
        anchor.extend(y.to_be_bytes());
        game.receive(game.generation(), &tape::bundle(&[anchor]), received)
            .unwrap();
        game.begin_frame(drawn, extent, 1.).unwrap();
        tape::effects(game);
        tape::pictures(game);
    }
}

#[test]
fn stationary_cursor_tracks_wheel_camera_and_zoom_smoothing_through_the_heading_gate() {
    use std::f64::consts::{PI, TAU};
    for notches in [-8., -3., 2.] {
        let mut game = tape::playing();
        inject_camera_corrections(&mut game, [960, 540]);
        let cursor = [505., 270.];
        game.pointer(cursor[0], cursor[1], 960., 540., false)
            .unwrap();
        assert!(game.wheel_zoom(notches));
        let mut previous_send: Option<f64> = None;
        let mut corrected_heading_sent = false;
        let mut held_heading = 0.;
        for index in 0..90 {
            let now = 100. + f64::from(index) * 10.;
            let head = displayed_head(&game, [960., 540.], [960, 540]);
            let dx = cursor[0] - head[0];
            let dy = cursor[1] - head[1];
            assert!(
                dx.hypot(dy) > 16.,
                "fixture must stay outside the CSS dead zone"
            );
            let expected = dy.atan2(dx);
            game.begin_frame(now, [960, 540], 1.).unwrap();
            let packets = tape::effects(&mut game);
            let headings: Vec<_> = packets
                .iter()
                .filter(|p| p.len() == 1 && p[0] < 251)
                .collect();
            assert!(headings.len() <= 1);
            for packet in headings {
                if let Some(previous) = previous_send {
                    assert!(now - previous > 33., "heading cadence changed");
                }
                previous_send = Some(now);
                let sent = f64::from(packet[0]) * TAU / 251.;
                held_heading = sent;
                let error = (sent - expected + PI).rem_euclid(TAU) - PI;
                assert!(
                    error.abs() <= TAU / 251. + 1e-10,
                    "stale stationary projection: {packet:?}, error {error}"
                );
                corrected_heading_sent |= packet[0] != 0;
            }
            tape::pictures(&mut game);
        }
        assert!(
            corrected_heading_sent,
            "wheel/camera changes must steer without a pointer callback"
        );
        let head = displayed_head(&game, [960., 540.], [960, 540]);
        let settled = (cursor[1] - head[1]).atan2(cursor[0] - head[0]);
        let error = (held_heading - settled + PI).rem_euclid(TAU) - PI;
        assert!(
            error.abs() <= TAU / 251. + 1e-10,
            "held heading must converge after smoothing"
        );
        // Stock retains the originally admitted CSS displacement even across a
        // resize. Returning to one must restore that cache, not reproject it.
        let extent = game.window_viewport([800, 800], 1.).unwrap();
        let factor = game.client().camera().user_zoom_factor();
        assert!(game.wheel_zoom(-factor.ln() / 1.12_f64.ln()));
        for index in 0..130 {
            game.begin_frame(1000. + f64::from(index) * 10., extent, 1.)
                .unwrap();
            for packet in tape::effects(&mut game) {
                if packet.len() == 1 && packet[0] < 251 {
                    held_heading = f64::from(packet[0]) * TAU / 251.;
                }
            }
            tape::pictures(&mut game);
        }
        assert_eq!(game.client().camera().user_zoom_factor(), 1.);
        assert_eq!(
            held_heading, 0.,
            "factor one restores the stock rightward command"
        );
        game.pointer(cursor[0], cursor[1], 800., 800., false)
            .unwrap();
        game.begin_frame(2340., extent, 1.).unwrap();
        assert_eq!(
            heading(&tape::effects(&mut game)),
            215,
            "a real callback uses the resized centre"
        );
        tape::pictures(&mut game);
    }
}

#[test]
fn host_handoff_seeds_empty_or_stale_zoom_cursor_and_reprojects_physical_position_at_new_dpr() {
    use slither_client::settings::WorldResolution::{Native, Original};
    use slither_wgpu_slice::world_resolution::WorldWindow;
    use std::f64::consts::{PI, TAU};
    for previous_game_cursor in [false, true] {
        for requested in [Original, Native] {
            let mut game = tape::playing();
            let mut window = WorldWindow {
                physical: [1920, 1080],
                dpr: 2.,
                requested,
            };
            let extent = window.viewport(&mut game).unwrap();
            inject_camera_corrections(&mut game, extent);
            if previous_game_cursor {
                game.pointer(850., 130., 960., 540., false).unwrap();
            }
            // The menu consumed the latest physical move. No gameplay pointer
            // callback accompanies Play or the later change from DPR 2 to 1.
            let physical_cursor = [1010., 540.];
            window.observe_cursor(&mut game, physical_cursor).unwrap();
            assert!(game.wheel_zoom(-8.));
            let mut fresh_sent = false;
            let mut after_dpr_sent = false;
            for index in 0..90 {
                if index == 8 {
                    window.dpr = 1.;
                }
                let extent = window.viewport(&mut game).unwrap();
                window.observe_cursor(&mut game, physical_cursor).unwrap();
                let css = if index < 8 {
                    [960., 540.]
                } else {
                    [1920., 1080.]
                };
                let cursor = if index < 8 {
                    [505., 270.]
                } else {
                    [1010., 540.]
                };
                let head = displayed_head(&game, css, extent);
                let expected = (cursor[1] - head[1]).atan2(cursor[0] - head[0]);
                let zoomed = game.client().camera().user_zoom_factor() != 1.;
                game.begin_frame(100. + f64::from(index) * 10., extent, 1.)
                    .unwrap();
                for packet in tape::effects(&mut game) {
                    if zoomed && packet.len() == 1 && packet[0] < 251 {
                        let sent = f64::from(packet[0]) * TAU / 251.;
                        let error = (sent - expected + PI).rem_euclid(TAU) - PI;
                        assert!(
                            error.abs() <= TAU / 251. + 1e-10,
                            "stale handoff/DPR cursor: {packet:?}, {error}"
                        );
                        fresh_sent = true;
                        after_dpr_sent |= index >= 8;
                    }
                }
                tape::pictures(&mut game);
            }
            assert!(fresh_sent && after_dpr_sent);
        }
    }
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
            inject_camera_corrections(&mut game, extent);
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
                let cursor = [head[0] + 25. * angle.cos(), head[1] + 25. * angle.sin()];
                game.pointer(cursor[0], cursor[1], css[0], css[1], false)
                    .unwrap();
                game.begin_frame(now + 10., extent, 1.).unwrap();
                let early = tape::effects(&mut game);
                assert!(
                    !early.iter().any(|p| p.len() == 1 && p[0] < 251),
                    "cadence changed: {early:?}"
                );
                tape::pictures(&mut game);
                let head = displayed_head(&game, css, extent);
                let expected = (cursor[1] - head[1]).atan2(cursor[0] - head[0]);
                game.begin_frame(now + 34., extent, 1.).unwrap();
                let packets = tape::effects(&mut game);
                let sent = f64::from(heading(&packets)) * TAU / 251.;
                let error = (sent - expected + PI).rem_euclid(TAU) - PI;
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
            #[cfg(feature = "frame-bench")]
            {
                assert!(p.frame.food_batch.is_some());
                let food = p.food_submission.unwrap();
                assert_eq!(food.counts.before_zoom_foods, food.counts.visible_foods);
                assert!(!food.budget_applied);
            }
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
