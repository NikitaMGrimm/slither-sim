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
            // Windows Arial's normal 17px cell includes its 1px rounded hhea gap;
            // font-box ascent/descent alone produces 19px and shifts later blocks.
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
        let reserve = render::PanelDrawReserve {
            announcement: true,
            result: false,
        }
        .late_draws();
        assert_eq!(reserve, 236);
        // Synthetic near-limit geometry: same valid world quad, repeated without
        // texture growth. This is an admission fixture, not dense-gameplay evidence.
        let mut world = base.frame.draws[0].clone();
        world.asset = bank.solid([0, 0, 0]);
        frame.draws.resize(MAX_DRAWS - reserve, world);
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
            render::PreparedFrame { frame, resources },
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
