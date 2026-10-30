//! Authored synthetic HUD projections and platform-text submission; no server.
use slither_client::presentation::{BodyBlend, BodyRect, HudOpacity, LeaderboardRow};
use slither_wgpu_slice::{
    bank::Bank,
    frame::{Draw, Frame, MAX_DRAWS},
    hud::{self, View},
    render,
    text::{Align, Baseline, CACHE_BYTES, CACHE_RUNS, TextRun, TextStyle, Weight},
};
fn view() -> View {
    View {
        playing: true,
        opacity: HudOpacity {
            readout: 1.,
            header: 0.85,
            rows: 1.,
            announcement: 1.,
        },
        status: "CONNECTED".into(),
        score: "10000".into(),
        final_score: "9000".into(),
        rank: "5".into(),
        population: "100".into(),
        rows: (1..=10)
            .map(|i| LeaderboardRow {
                ordinal: i,
                number_text: format!("#{i}"),
                score: 12345.,
                score_text: "12345".into(),
                name_text: format!("Player{i}"),
                css_color: "#80d0d0",
                own: i == 5,
                opacity_request: 0.5,
            })
            .collect(),
        message: None,
    }
}
fn frame(extent: [u32; 2], scale: f64) -> Frame {
    Frame {
        identity: 53,
        tick: 20,
        extent,
        viewport_scale: scale,
        draws: vec![],
        snakes: vec![],
    }
}
fn finish(v: &View, bank: &mut Bank, frame: Frame) -> render::PreparedFrame {
    let resources = bank.resources(&frame).unwrap();
    render::finish_hud(render::PreparedFrame { frame, resources }, bank, v).unwrap()
}
#[cfg(windows)]
#[test]
fn actual_text_draws_clip_names_to_columns_and_preserve_world_order() {
    let mut v = view();
    v.rows[0].name_text = "W".repeat(255);
    let mut bank = Bank::new().unwrap();
    let asset = bank.solid([255, 0, 0]);
    let world = Draw {
        corners: None,
        repeat_uv: None,
        black_effect: false,
        asset,
        source: None,
        destination: BodyRect {
            x: 100.,
            y: 100.,
            width: 5.,
            height: 5.,
        },
        translation: [0., 0.],
        rotation: 0.,
        effective_alpha: 0.5,
        blend: BodyBlend::Lighter,
    };
    let mut f = frame([960, 540], 1.);
    f.draws.push(world.clone());
    let out = finish(&v, &mut bank, f);
    assert_eq!(out.frame.draws[0], world);
    assert_eq!(out.frame.identity, 53);
    assert_eq!(out.frame.tick, 20);
    assert!(out.frame.draws.len() > 30);
    let names: Vec<_> = out
        .frame
        .draws
        .iter()
        .filter(|d| {
            d.destination.y >= 32.
                && d.destination.y < 50.
                && d.destination.x >= 716.
                && d.destination.x < 896.
        })
        .collect();
    assert!(!names.is_empty());
    for d in names {
        assert!(d.destination.x + d.destination.width <= 896.);
        assert!(d.source.is_some());
    }
    let resident = out
        .resources
        .iter()
        .map(|r| (r.asset.into(), r.asset))
        .collect();
    assert!(out.frame.quads(&resident).is_ok());
}
#[cfg(windows)]
#[test]
fn changing_scores_replace_slots_without_evicting_nicknames_or_growing_work() {
    let mut bank = Bank::new().unwrap();
    let mut v = view();
    let nickname = |text| TextRun {
        text,
        style: TextStyle {
            size: 15.,
            weight: Weight::Normal,
            color: [255; 3],
        },
        position: [100., 100.],
        align: Align::Center,
        baseline: Baseline::Middle,
        alpha: 1.,
    };
    let original = bank.text.draw(nickname("World nickname"), 1.).unwrap()[0]
        .0
        .asset;
    let first = finish(&v, &mut bank, frame([960, 540], 1.));
    let first_pixels = first.resources.clone();
    let runs = bank.text.cached_runs();
    assert_eq!(runs, 38);
    for i in 0..300 {
        v.score = (10000 + i).to_string();
        for (row, n) in v.rows.iter_mut().zip(0..) {
            row.score_text = (10000 + i * 10 + n).to_string();
        }
        let out = finish(&v, &mut bank, frame([960, 540], 1.));
        assert!(out.frame.draws.len() <= hud::MAX_DRAWS);
        assert_eq!(bank.text.cached_runs(), runs);
        assert!(bank.text.cached_runs() <= CACHE_RUNS);
        assert!(bank.text.cached_bytes() <= CACHE_BYTES);
    }
    assert_eq!(
        bank.text.draw(nickname("World nickname"), 1.).unwrap()[0]
            .0
            .asset,
        original
    );
    for (old, saved) in first.resources.iter().zip(first_pixels) {
        assert_eq!(old.asset, saved.asset);
        assert_eq!(old.image.pixels, saved.image.pixels);
    }
    let resident = first
        .resources
        .iter()
        .map(|r| (r.asset.into(), r.asset))
        .collect();
    assert!(first.frame.quads(&resident).is_ok());
}
#[cfg(windows)]
#[test]
fn a_full_or_nearly_full_frame_never_overruns_the_draw_limit() {
    let v = view();
    let mut bank = Bank::new().unwrap();
    let asset = bank.solid([255; 3]);
    let draw = Draw {
        corners: None,
        repeat_uv: None,
        black_effect: false,
        asset,
        source: None,
        destination: BodyRect {
            x: 1.,
            y: 1.,
            width: 1.,
            height: 1.,
        },
        translation: [0., 0.],
        rotation: 0.,
        effective_alpha: 1.,
        blend: BodyBlend::SourceOver,
    };
    for count in [MAX_DRAWS, MAX_DRAWS - 1, MAX_DRAWS - 40] {
        let mut f = frame([960, 540], 1.);
        f.draws = vec![draw.clone(); count];
        let out = finish(&v, &mut bank, f);
        assert!(out.frame.draws.len() <= MAX_DRAWS);
        assert_eq!(&out.frame.draws[..count], vec![draw.clone(); count]);
    }
}

#[cfg(windows)]
#[test]
fn ten_long_names_are_clipped_before_raster_allocation_and_reused_at_high_dpr() {
    let mut v = view();
    for row in &mut v.rows {
        row.name_text = "W".repeat(255);
    }
    for dpr in [1., 2., 4.] {
        let mut bank = Bank::new().unwrap();
        let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
        let first = finish(&v, &mut bank, frame(extent, dpr));
        assert_eq!(bank.text.cached_runs(), 37);
        assert!(first.frame.draws.len() <= hud::MAX_DRAWS);
        let assets: Vec<_> = first.frame.draws.iter().map(|d| d.asset).collect();
        let second = finish(&v, &mut bank, frame(extent, dpr));
        assert_eq!(
            assets,
            second
                .frame
                .draws
                .iter()
                .map(|d| d.asset)
                .collect::<Vec<_>>()
        );
        assert!(bank.text.cached_bytes() <= CACHE_BYTES);
    }
}

#[cfg(windows)]
#[test]
fn fractional_dpr_text_crops_stay_inside_their_texture_tiles() {
    let mut v = view();
    v.rows[0].name_text = "W".repeat(255);
    let mut bank = Bank::new().unwrap();
    for scale in [1.25, 1.75, 2.75, 3.5] {
        for extent in [[960, 540], [(960. * scale) as u32, (540. * scale) as u32]] {
            let out = finish(&v, &mut bank, frame(extent, scale));
            assert!(!out.frame.draws.is_empty());
            assert!(out.frame.draws.len() <= hud::MAX_DRAWS);
            for draw in &out.frame.draws {
                let source = draw.source.unwrap();
                assert!(source.x + source.width <= f64::from(draw.asset.width));
                assert!(source.y + source.height <= f64::from(draw.asset.height));
            }
        }
    }
}
