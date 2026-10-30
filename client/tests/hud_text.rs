//! Authored synthetic shared HUD typography and layout, without fonts or GPU.
use slither_client::presentation::{
    HudOpacity, HudTextView as View, LeaderboardRow, TextAlign as Align, TextWeight as Weight,
    plan_hud_text,
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
#[test]
fn text_runs_match_readout_and_leaderboard_content_styles_and_anchors() {
    let p = plan_hud_text(&view(), [960, 540], 1.).unwrap();
    assert_eq!(p.lines.len(), 33);
    let length = &p.lines[0];
    assert_eq!(length.position, [8., 513.]);
    assert_eq!(length.align, Align::Left);
    assert_eq!(
        length
            .spans
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>(),
        ["Your length: ", "10000"]
    );
    assert_eq!(
        length
            .spans
            .iter()
            .map(|s| (s.style.size, s.style.weight, s.style.color, s.alpha))
            .collect::<Vec<_>>(),
        [
            (14., Weight::Normal, [255; 3], 0.4),
            (14., Weight::Bold, [255; 3], 0.8)
        ]
    );
    let rank = &p.lines[1];
    assert_eq!(rank.position, [8., 530.]);
    assert_eq!(
        rank.spans
            .iter()
            .map(|s| (s.text.as_str(), s.style.size, s.style.weight, s.alpha))
            .collect::<Vec<_>>(),
        [
            ("Your rank: ", 12., Weight::Normal, 0.3),
            ("5", 12., Weight::Normal, 0.35),
            (" of ", 12., Weight::Normal, 0.3),
            ("100", 12., Weight::Normal, 0.35)
        ]
    );
    let heading = &p.lines[2];
    assert_eq!(heading.position, [813.5, 23.]);
    assert_eq!(heading.align, Align::Center);
    assert_eq!(heading.spans[0].text, "Leaderboard");
    assert_eq!(heading.spans[0].style.size, 21.);
    assert_eq!(heading.spans[0].style.weight, Weight::Bold);
    assert_eq!(heading.spans[0].alpha, 0.85);
    for i in 0..10 {
        let row = &p.lines[3 + i * 3..6 + i * 3];
        assert_eq!(
            row.iter().map(|l| l.position).collect::<Vec<_>>(),
            [
                [931., 45. + i as f64 * 18.],
                [716., 45. + i as f64 * 18.],
                [700., 45. + i as f64 * 18.]
            ]
        );
        assert_eq!(
            row.iter().map(|l| l.align).collect::<Vec<_>>(),
            [Align::Center, Align::Left, Align::Right]
        );
        assert_eq!(row[0].spans[0].text, "12345");
        assert_eq!(row[1].spans[0].text, format!("Player{}", i + 1));
        assert_eq!(row[2].spans[0].text, format!("#{}", i + 1));
        for (col, line) in row.iter().enumerate() {
            let span = &line.spans[0];
            assert_eq!(span.style.size, 12.);
            assert_eq!(span.style.color, [128, 208, 208]);
            assert_eq!(span.alpha, 0.5);
            assert_eq!(
                span.style.weight,
                if col == 1 && i == 4 {
                    Weight::Bold
                } else {
                    Weight::Normal
                }
            );
        }
    }
}
#[test]
fn projected_text_is_not_rescored_reordered_or_normalized() {
    let mut v = view();
    v.rows[0].name_text = "Two\u{a0}words caf\u{e9} \u{ff}".into();
    v.rows[0].score_text = "NaN".into();
    v.rows[0].score = f64::NAN;
    v.rows[1].score_text = "-9007199254740991".into();
    let p = plan_hud_text(&v, [960, 540], 1.).unwrap();
    assert_eq!(p.lines[3].spans[0].text, "NaN");
    assert_eq!(p.lines[4].spans[0].text, v.rows[0].name_text);
    assert_eq!(p.lines[6].spans[0].text, "-9007199254740991");
}
#[test]
fn logical_dpr_layout_and_ordered_short_window_prefix_are_bounded() {
    let v = view();
    assert_eq!(
        plan_hud_text(&v, [960, 540], 1.).unwrap(),
        plan_hud_text(&v, [1920, 1080], 2.).unwrap()
    );
    for extent in [
        [0, 0],
        [0, 540],
        [32, 24],
        [64, 48],
        [160, 120],
        [320, 120],
        [320, 200],
        [640, 80],
        [640, 100],
        [960, 120],
        [640, 360],
        [960, 540],
        [1920, 1080],
        [8192, 8192],
    ] {
        for scale in [1., 1.25, 2., 4., 32.] {
            let p = plan_hud_text(&v, extent, scale).unwrap();
            assert!(
                p.lines.iter().map(|l| l.spans.len()).sum::<usize>()
                    <= slither_client::presentation::MAX_HUD_TEXT_SPANS
            );
            let ids: Vec<_> = p
                .lines
                .iter()
                .flat_map(|l| &l.spans)
                .filter(|s| s.text.starts_with('#'))
                .map(|s| s.text.clone())
                .collect();
            assert_eq!(
                ids,
                (1..=ids.len()).map(|i| format!("#{i}")).collect::<Vec<_>>()
            );
        }
    }
    let short = plan_hud_text(&v, [320, 180], 1.).unwrap();
    let rows = short
        .lines
        .iter()
        .filter(|l| l.spans[0].text.starts_with('#'))
        .count();
    assert!((1..10).contains(&rows));
    for extent in [[640, 360], [400, 700]] {
        assert_eq!(plan_hud_text(&v, extent, 1.).unwrap().lines.len(), 33);
    }
    for scale in [0., -1., f64::NAN, f64::INFINITY, 1e-300] {
        assert!(plan_hud_text(&v, [960, 540], scale).is_err());
    }
    assert!(plan_hud_text(&v, [8193, 540], 1.).is_err());
}
#[test]
fn lifecycle_opacity_multiplies_fields_and_header_independently() {
    let mut v = view();
    v.opacity.readout = 0.25;
    v.opacity.header = 0.2;
    v.opacity.rows = 0.4;
    let p = plan_hud_text(&v, [960, 540], 1.).unwrap();
    assert_eq!(p.lines[0].spans[0].alpha, 0.1);
    assert_eq!(p.lines[0].spans[1].alpha, 0.2);
    assert_eq!(p.lines[1].spans[0].alpha, 0.075);
    assert_eq!(p.lines[2].spans[0].alpha, 0.2);
    assert_eq!(p.lines[3].spans[0].alpha, 0.2);
    v.playing = false;
    let p = plan_hud_text(&v, [960, 540], 1.).unwrap();
    assert_eq!(p.lines[0].spans[0].text, "CONNECTED");
    assert_eq!(p.lines[1].spans[0].text, "Last length: 9000");
    v.playing = true;
    v.score = "--".into();
    let p = plan_hud_text(&v, [960, 540], 1.).unwrap();
    assert_eq!(p.lines[0].spans[0].text, "Leaderboard");
}
