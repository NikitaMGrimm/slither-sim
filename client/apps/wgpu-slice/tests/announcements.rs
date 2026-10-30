//! Authored a/m/l/v tapes through production LocalGame; no socket or window.
#![cfg(windows)]
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::presentation::*;
use slither_wgpu_slice::{
    announcement::Announcement, frame::Draw, game::LocalGame, render, text::*,
};
use tape::*;

fn message(name: &str, text: &str) -> Vec<u8> {
    let mut p = vec![b'm', 0, 0, 1, 255, 255, 255, name.len() as u8];
    p.extend(name.bytes());
    p.extend(text.bytes());
    p
}
fn ready(name: &str, text: &str) -> LocalGame {
    let mut g = negotiating();
    g.receive(
        g.generation(),
        &bundle(&[setup(), snake(44), message(name, text)]),
        3.,
    )
    .unwrap();
    pictures(&mut g);
    g
}
fn draw(g: &mut LocalGame, now: f64) -> render::PreparedFrame {
    g.begin_frame(now, [960, 540], 1.).unwrap();
    effects(g);
    g.take_frame().unwrap()
}
fn announcement(p: &render::PreparedFrame) -> Vec<&Draw> {
    p.frame
        .draws
        .iter()
        .filter(|d| {
            d.asset.handle >> 60 == 4
                && d.destination.x + d.translation[0] < 308.
                && d.destination.y + d.translation[1] < 232.
        })
        .collect()
}
fn assets(p: &render::PreparedFrame) -> Vec<u64> {
    announcement(p).iter().map(|d| d.asset.handle).collect()
}
fn board_at(g: &mut LocalGame, now: f64) {
    g.receive(g.generation(), &bundle(&[board()]), now).unwrap();
}
#[test]
fn production_crowded_world_keeps_all_geometry_with_hidden_or_visible_announcements() {
    use slither_wgpu_slice::{bank::Bank, frame::MAX_DRAWS};
    let mut g = negotiating();
    let mut commands = vec![setup(), snake(44)];
    // Authored mandatory geometry: 259 world draws + 1109*7 food draws +
    // 5*6 prey draws = 8052. This frame has no visible nickname yet.
    for i in 0..1109 {
        commands.push(vec![b'F', 11, 10, 41, (i % 40) as u8, (i / 40) as u8, 20]);
    }
    for id in 1000_u16..1005 {
        let mut packet = prey();
        packet[1..3].copy_from_slice(&id.to_be_bytes());
        commands.push(packet);
    }
    g.receive(g.generation(), &bundle(&commands), 3.).unwrap();
    pictures(&mut g);
    let old_capacity = 8052;
    let quality = g.quality();
    let alive = AnnouncementLifecycle {
        death_pending: false,
        return_active: false,
        return_completed: false,
        return_fraction: 1.,
    };
    let prepare = |client: &AnimatedClient, bank: &mut Bank, life, identity| {
        // Every candidate starts from the same retained world/clock. Advancing
        // the owner makes all food/prey calls valid without hand-building draws.
        let mut world = client.clone();
        world
            .advance_with_control::<ClientPresentationError, _>(40., false, |_, _| {
                Ok(ClientControlActions {
                    connected: Some(true),
                    announcement_lifecycle: life,
                    ..Default::default()
                })
            })
            .unwrap();
        let view = world.announcement_view();
        let prepared =
            render::prepare_gameplay(&mut world, bank, [960, 540], 1., identity, quality);
        (view, prepared)
    };
    let client = g.client().clone();
    let (view, before_record) = prepare(&client, &mut g.bank, Some(alive), 100);
    assert!(!view.present && view.text.blocks.is_empty());
    let before_record = before_record.expect("no record must retain the old world capacity");
    assert_eq!(before_record.frame.draws.len(), old_capacity);
    // Production finishing still admits the HUD and validates the complete frame.
    let mut hud = g.hud_view();
    let ScoreStatus::Ready(score) = g.client().score_view().status else {
        panic!("ready score");
    };
    hud.score = score.to_string();
    hud.opacity.readout = 1.;
    hud.opacity.header = 0.85;
    hud.opacity.rows = 1.;
    let finished = render::finish_hud(before_record, &mut g.bank, &hud).unwrap();
    assert!(finished.frame.draws.len() > old_capacity);
    assert!(finished.frame.draws.len() <= MAX_DRAWS);
    assert!(finished.pixel_bytes() <= slither_wgpu_slice::bank::PIXEL_BUDGET);

    board_at(&mut g, 4.);
    let client = g.client().clone();
    let (view, no_projection) = prepare(&client, &mut g.bank, Some(alive), 101);
    assert!(view.opacity > 0. && !view.present && view.text.blocks.is_empty());
    assert_eq!(no_projection.unwrap().frame.draws.len(), old_capacity);

    g.receive(g.generation(), &bundle(&[message("N", "retained")]), 5.)
        .unwrap();
    let client = g.client().clone();
    let (view, zero_fade) = prepare(&client, &mut g.bank, None, 102);
    assert!(view.present && !view.text.blocks.is_empty());
    assert_eq!(view.opacity, 0.);
    assert_eq!(zero_fade.unwrap().frame.draws.len(), old_capacity);
    for (identity, fraction) in [(103, 0.01), (104, 0.5), (105, 1.)] {
        let (view, visible) = prepare(
            &client,
            &mut g.bank,
            Some(AnnouncementLifecycle {
                death_pending: true,
                return_active: true,
                return_completed: false,
                return_fraction: fraction,
            }),
            identity,
        );
        assert!(view.present);
        assert_eq!(view.opacity, fraction);
        let visible = visible.unwrap();
        assert!(visible.frame.draws.len() > old_capacity);
        // World order/geometry is identical; the panel is appended after it.
        let baseline = prepare(&client, &mut g.bank, None, identity + 100)
            .1
            .unwrap();
        assert_eq!(&visible.frame.draws[..old_capacity], baseline.frame.draws);
    }
    let (view, returned) = prepare(
        &client,
        &mut g.bank,
        Some(AnnouncementLifecycle {
            death_pending: false,
            return_active: true,
            return_completed: true,
            return_fraction: 0.,
        }),
        106,
    );
    assert!(view.present);
    assert_eq!(view.opacity, -1.);
    assert_eq!(returned.unwrap().frame.draws.len(), old_capacity);

    // An actual next Setup hides the record while retaining the same geometry.
    g.receive(g.generation(), &bundle(&[setup()]), 6.).unwrap();
    pictures(&mut g);
    let client = g.client().clone();
    let (view, setup_hidden) = prepare(&client, &mut g.bank, Some(alive), 107);
    assert!(view.present);
    assert_eq!(view.opacity, 0.);
    assert_eq!(setup_hidden.unwrap().frame.draws.len(), old_capacity);
}
#[test]
fn all_four_branches_preserve_fragment_alpha_gap_geometry_and_fade_resources() {
    for (name, message) in [("N", "M"), ("", "M"), ("N", ""), ("", "")] {
        let mut g = ready(name, message);
        let hidden = draw(&mut g, 20.);
        assert!(announcement(&hidden).is_empty());
        g.presented(true).unwrap();
        board_at(&mut g, 21.);
        let early = draw(&mut g, 40.);
        let early_draws = announcement(&early);
        assert!(!early_draws.is_empty());
        g.presented(true).unwrap();
        let fade = g.client().announcement_view().opacity;
        let expected = if !message.is_empty() {
            vec![0.5, 1., 0.5, 0.5]
        } else {
            vec![0.5, 0.75]
        };
        for (d, a) in early_draws.iter().zip(expected) {
            assert_eq!(d.effective_alpha, fade * a);
        }
        for d in &early_draws {
            assert!(d.destination.x >= 8. && d.destination.y >= 4.);
            assert!(d.destination.x + d.destination.width <= 308.);
            assert!(d.destination.y + d.destination.height <= 232.);
        }
        if !message.is_empty() {
            let mut submission = Announcement::default();
            let (plan, alpha) = submission
                .plan(&g.client().announcement_view(), &mut g.bank.text, 1.)
                .unwrap();
            assert_eq!(&alpha[..3], &[0.5, 1., 0.5]);
            // Explicit typography policy: round(1.15 * 17) = 20px.
            assert_eq!(plan.runs[0].clip.height, 20.);
            assert_eq!(plan.runs[3].clip.y, 29.);
            assert_eq!(plan.runs[0].text, "\"");
            assert_eq!(plan.runs[1].text, "M");
            assert_eq!(plan.runs[2].text, "\"");
            assert_eq!(
                plan.runs[3].clip.y - (plan.runs[0].clip.y + plan.runs[0].clip.height),
                5.
            );
        }
        let later = draw(&mut g, 60.);
        assert_eq!(
            assets(&early),
            assets(&later),
            "fade must reuse immutable textures"
        );
        assert!(announcement(&later)[0].effective_alpha > early_draws[0].effective_alpha);
        g.presented(true).unwrap();
    }
}
#[test]
fn invalid_tail_rolls_back_and_replacement_does_not_restart_fade() {
    let mut g = ready("Old", "first");
    board_at(&mut g, 4.);
    pump(&mut g, 20.);
    let before = g.client().announcement_view();
    assert!(
        g.receive(
            g.generation(),
            &bundle(&[message("Bad", "discard"), vec![b'm']]),
            21.
        )
        .is_err()
    );
    assert_eq!(g.client().announcement_view(), before);
    g.receive(
        g.generation(),
        &bundle(&[message("New", "replacement")]),
        22.,
    )
    .unwrap();
    let replaced = g.client().announcement_view();
    assert_eq!(replaced.opacity, before.opacity);
    assert_eq!(replaced.lines[0].text, "\"replacement\"");
    pump(&mut g, 40.);
    assert!(g.client().announcement_view().opacity > before.opacity);
}
#[test]
fn death_hold_return_reset_and_next_setup_retain_content() {
    let mut g = ready("N", "held");
    board_at(&mut g, 4.);
    for t in (20..=1000).step_by(20) {
        pump(&mut g, t as f64);
        g.receive(g.generation(), &bundle(&[vec![b'p']]), t as f64)
            .unwrap();
    }
    assert_eq!(g.client().announcement_view().opacity, 1.);
    let text = g.client().announcement_view().text;
    g.receive(g.generation(), &bundle(&[vec![b'v', 0]]), 1001.)
        .unwrap();
    for t in (1020..=2600).step_by(20) {
        pump(&mut g, t as f64);
        g.receive(g.generation(), &bundle(&[vec![b'p']]), t as f64)
            .unwrap();
        assert_eq!(g.client().announcement_view().opacity, 1.);
    }
    pump(&mut g, 2620.);
    assert!(g.client().announcement_view().opacity < 1.);
    for t in (2640..=4800).step_by(20) {
        pump(&mut g, t as f64);
        g.receive(g.generation(), &bundle(&[vec![b'p']]), t as f64)
            .unwrap();
    }
    assert_eq!(g.client().announcement_view().opacity, -1.);
    assert_eq!(g.client().announcement_view().text, text);
    g.play("ws://127.0.0.1:19491/slither", "Again", 0).unwrap();
    assert_eq!(g.client().announcement_view().text, text);
    // The shared owner, rather than adapter cleanup, hides retained text on Setup.
    let mut client = g.client().clone();
    client.reset();
    assert_eq!(client.announcement_view().text, text);
    // Fresh accepted Setup in the current admitted session, before death, is
    // also covered independently to avoid inventing a reconnect handshake.
    let mut next = ready("N", "held");
    board_at(&mut next, 4.);
    pump(&mut next, 20.);
    next.receive(next.generation(), &bundle(&[setup()]), 21.)
        .unwrap();
    assert_eq!(next.client().announcement_view().text, text);
    assert_eq!(next.client().announcement_view().opacity, 0.);
}
#[test]
fn suspension_resize_retry_and_deferred_packet_keep_the_captured_picture() {
    let mut g = ready("N", "old");
    board_at(&mut g, 4.);
    let old = draw(&mut g, 20.);
    let commands = old.frame.draws.clone();
    let pixels: Vec<_> = old
        .resources
        .iter()
        .map(|r| r.image.pixels.clone())
        .collect();
    g.receive(g.generation(), &bundle(&[message("N", "new")]), 21.)
        .unwrap();
    assert_eq!(g.client().announcement_view().lines[0].text, "\"old\"");
    g.retry_frame(old).unwrap();
    g.viewport([0, 0], 1.).unwrap();
    assert!(g.take_frame().is_none());
    g.viewport([960, 540], 1.).unwrap();
    let retry = g.take_frame().unwrap();
    assert_eq!(retry.frame.draws, commands);
    assert_eq!(
        retry
            .resources
            .iter()
            .map(|r| r.image.pixels.clone())
            .collect::<Vec<_>>(),
        pixels
    );
    g.presented(true).unwrap();
    pump(&mut g, 40.);
    assert_eq!(g.client().announcement_view().lines[0].text, "\"new\"");
    let new = draw(&mut g, 60.);
    let captured_alpha = announcement(&new)
        .iter()
        .map(|d| d.effective_alpha)
        .collect::<Vec<_>>();
    let live_opacity = g.client().announcement_view().opacity;
    g.retry_frame(new).unwrap();
    g.viewport([1920, 1080], 2.).unwrap();
    let resized = g.take_frame().unwrap();
    assert!(!announcement(&resized).is_empty());
    assert_eq!(
        announcement(&resized)
            .iter()
            .map(|d| d.effective_alpha)
            .collect::<Vec<_>>(),
        captured_alpha
    );
    assert_eq!(g.client().announcement_view().opacity, live_opacity);
    g.presented(true).unwrap();
    assert!(g.client().announcement_view().opacity > 0.);
}
#[test]
fn setup_pictures_capture_command_time_announcement_not_later_live_text() {
    let mut g = ready("N", "old");
    board_at(&mut g, 4.);
    pump(&mut g, 20.);
    g.receive(
        g.generation(),
        &bundle(&[
            setup(),
            message("N", "later"),
            board(),
            setup(),
            message("N", "last"),
        ]),
        21.,
    )
    .unwrap();
    assert_eq!(g.client().announcement_view().lines[0].text, "\"last\"");
    for _ in 0..2 {
        let picture = g.take_frame().unwrap();
        assert!(announcement(&picture).is_empty());
        g.retry_frame(picture).unwrap();
        let retry = g.take_frame().unwrap();
        assert!(announcement(&retry).is_empty());
        g.presented(true).unwrap();
    }
    assert!(!g.has_frame());
    board_at(&mut g, 22.);
    let p = draw(&mut g, 40.);
    assert!(!announcement(&p).is_empty());
    g.presented(true).unwrap();
}
#[test]
fn changed_content_keeps_nickname_and_hud_slots_at_every_supported_dpr() {
    for scale in [1., 1.25, 2., 3., 4.] {
        let mut g = ready("N", "old");
        board_at(&mut g, 4.);
        pump(&mut g, 20.);
        let run = TextRun {
            text: "nickname",
            style: TextStyle {
                size: 12.,
                weight: Weight::Normal,
                family: Family::Arial,
                slant: Slant::Normal,
                color: [255; 3],
            },
            position: [400., 250.],
            align: Align::Left,
            baseline: Baseline::Alphabetic,
            alpha: 1.,
        };
        let first = g.bank.text.draw(TextRun { ..run }, scale).unwrap();
        let hud_before = g
            .bank
            .text
            .draw_hud(0, TextRun { ..run }, scale, None)
            .unwrap()
            .1;
        let mut submit = Announcement::default();
        let mut view = g.client().announcement_view();
        view.opacity = 1.;
        let (p, a) = submit.plan(&view, &mut g.bank.text, scale).unwrap();
        g.bank
            .text
            .draw_panel_composition_with_run_alphas(
                &[PanelSubmission {
                    group: PanelGroup::Announcement,
                    plan: &p,
                    alpha: 1.,
                }],
                scale,
                &[&a],
            )
            .unwrap();
        view.text = project_announcement_text(&AnnouncementContent {
            score: 123.,
            name: "Changed".into(),
            message: "wideword ".repeat(300),
        });
        let (p, a) = submit.plan(&view, &mut g.bank.text, scale).unwrap();
        g.bank
            .text
            .draw_panel_composition_with_run_alphas(
                &[PanelSubmission {
                    group: PanelGroup::Announcement,
                    plan: &p,
                    alpha: 1.,
                }],
                scale,
                &[&a],
            )
            .unwrap();
        let after = g.bank.text.draw(TextRun { ..run }, scale).unwrap();
        assert_eq!(first[0].1.asset, after[0].1.asset);
        assert_eq!(
            hud_before[0].1.asset,
            g.bank.text.draw_hud(0, run, scale, None).unwrap().1[0]
                .1
                .asset
        );
    }
}

#[test]
fn near_budget_world_long_record_ten_row_hud_and_maximal_minimap_compose_in_order() {
    use slither_wgpu_slice::{
        bank::Bank,
        frame::{Frame, MAX_DRAWS},
    };
    let mut g = ready("Long name", &"wideword ".repeat(300));
    let mut board = board();
    let row = board[6..].to_vec();
    for _ in 1..10 {
        board.extend(&row);
    }
    g.receive(g.generation(), &bundle(&[board, vec![b'M', 2, 0, 64]]), 4.)
        .unwrap();
    pump(&mut g, 20.);
    for scale in [1., 1.25, 2., 3., 4.] {
        let mut bank = Bank::new().unwrap();
        let base = draw(&mut g, 40. + scale);
        let mut frame = Frame {
            identity: base.frame.identity,
            tick: 0,
            extent: [960, 540],
            viewport_scale: scale,
            draws: Vec::new(),
            snakes: Vec::new(),
        };
        let overlay_capacity = render::PANEL_PRODUCER_DRAWS
            + slither_wgpu_slice::minimap::MAX_DRAWS
            + slither_wgpu_slice::hud::MAX_DRAWS;
        // Synthetic near-limit geometry: same valid world quad, repeated without
        // texture growth. This is an admission fixture, not dense-gameplay evidence.
        let mut world = base.frame.draws[0].clone();
        world.asset = bank.solid([0, 0, 0]);
        frame.draws.resize(MAX_DRAWS - overlay_capacity, world);
        let mut resources = bank.resources(&frame).unwrap();
        let world_end = frame.draws.len();
        bank.announcement
            .append(
                &g.client().announcement_view(),
                &mut bank.text,
                &mut frame,
                &mut resources,
            )
            .unwrap();
        let announcement_end = frame.draws.len();
        assert!(announcement_end > world_end);
        let client = g.client().clone();
        // The production map owner reads the same retained 512x512 snapshot.
        bank.minimap
            .append(&client, &mut bank.text, &mut frame, &mut resources)
            .unwrap();
        let map_end = frame.draws.len();
        assert!(map_end > announcement_end);
        assert_eq!(g.hud_view().rows.len(), 10);
        let out = render::finish_hud(
            render::PreparedFrame {
                frame,
                overlay: None,
                resources,
            },
            &mut bank,
            &g.hud_view(),
        )
        .unwrap();
        assert!(out.frame.draws.len() > map_end && out.frame.draws.len() <= MAX_DRAWS);
        assert!(out.pixel_bytes() <= slither_wgpu_slice::bank::PIXEL_BUDGET);
        let resident = out
            .resources
            .iter()
            .map(|r| (r.asset.into(), r.asset))
            .collect();
        out.frame.quads(&resident).unwrap();
        g.presented(true).unwrap();
    }
}
