//! Authored policy fixtures; fake metrics make no platform font/parity claim.
use slither_client::presentation::*;

fn style() -> TextStyle {
    TextStyle {
        size: 16.,
        family: TextFamily::Arial,
        slant: TextSlant::Normal,
        weight: TextWeight::Normal,
        color: [255; 3],
    }
}
#[derive(Default)]
struct Metrics {
    calls: usize,
    bytes: usize,
    tiny: bool,
    invalid: bool,
}
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, style: TextStyle) -> Result<TextMeasurement, &'static str> {
        self.calls += 1;
        self.bytes += text.len();
        if self.invalid && text.contains('!') {
            return Ok(TextMeasurement {
                advance: f64::NAN,
                ascent: 12.,
                descent: 4.,
                font_ascent: style.size * 0.75,
                font_descent: style.size * 0.25,
            });
        }
        Ok(TextMeasurement {
            advance: text.chars().count() as f64 * if self.tiny { 0.01 } else { style.size / 2. },
            ascent: if text.contains(char::is_uppercase) {
                style.size * 0.75
            } else if text.chars().all(|c| c == '.') {
                2.
            } else {
                style.size * 0.5
            },
            descent: if text.contains('g') { 4. } else { 0. },
            font_ascent: style.size * 0.75,
            font_descent: style.size * 0.25,
        })
    }
}
fn layout(
    fragments: &[PanelTextFragment<'_>],
    width: f64,
    height: f64,
    m: &mut Metrics,
) -> Result<PanelTextPlan, &'static str> {
    layout_panel_text(
        &PanelTextBlock {
            text: &PanelText::new(fragments)?,
            bounds: BodyRect {
                x: 2.,
                y: 3.,
                width,
                height,
            },
            line_height: 16.,
            device_scale: 1.,
            max_runs: 64,
        },
        m,
    )
}
#[test]
fn words_collapse_whitespace_and_explicit_breaks_retain_empty_lines() {
    let f = [
        PanelTextFragment::Text("  one\t two\n", style()),
        PanelTextFragment::Break,
        PanelTextFragment::Break,
        PanelTextFragment::Text("three four", style()),
    ];
    let p = layout(&f, 64., 80., &mut Metrics::default()).unwrap();
    assert_eq!(
        p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["one two", "three", "four"]
    );
    assert_eq!(
        p.runs.iter().map(|r| r.position[1]).collect::<Vec<_>>(),
        [15., 47., 63.]
    );
}
#[test]
fn overlong_utf8_words_split_without_splitting_scalars_and_clip_last_cell() {
    let f = [PanelTextFragment::Text("é🙂é🙂é", style())];
    let p = layout(&f, 16., 20., &mut Metrics::default()).unwrap();
    assert_eq!(
        p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["é🙂", "é🙂"]
    );
    assert_eq!(p.runs[1].clip.height, 4.);
    for r in p.runs {
        assert!(r.clip.x >= 2. && r.clip.x + r.clip.width <= 18.);
    }
}
#[test]
fn style_changes_inside_words_do_not_create_wrap_opportunities() {
    let mut bold = style();
    bold.weight = TextWeight::Bold;
    let f = [
        PanelTextFragment::Text("a ", style()),
        PanelTextFragment::Text("bc", bold),
        PanelTextFragment::Text("de", style()),
    ];
    let p = layout(&f, 40., 48., &mut Metrics::default()).unwrap();
    assert_eq!(
        p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["a", "bc", "de"]
    );
    assert_eq!(p.runs[1].position[1], 31.);
    assert_eq!(p.runs[2].position, [18., 31.]);
}
#[test]
fn artificial_chunks_keep_complete_run_measurements_and_safe_utf8() {
    let text = "é".repeat(900);
    let mut m = Metrics {
        tiny: true,
        ..Metrics::default()
    };
    let p = layout(
        &[PanelTextFragment::Text(&text, style())],
        600.,
        16.,
        &mut m,
    )
    .unwrap();
    assert_eq!(p.runs.len(), 2);
    assert_eq!(
        p.runs.iter().map(|r| r.text.len()).sum::<usize>(),
        text.len()
    );
    assert!(p.runs.iter().all(|r| r.text.len() <= 1024));
    assert!((p.runs[1].position[0] - 7.12).abs() < 1e-9);
    assert!(m.bytes > text.len());
}
#[test]
fn invisible_suffix_and_pathological_whitespace_have_fixed_work_bounds() {
    let mut m = Metrics::default();
    let prefix = "word ".repeat(10);
    let text = format!("{prefix}{}", "!".repeat(1_000_000));
    m.invalid = true;
    let p = layout(&[PanelTextFragment::Text(&text, style())], 40., 16., &mut m).unwrap();
    assert_eq!(p.runs[0].text, "word");
    assert!(p.scanned_bytes < 20 && m.bytes < 200);
    let spaces = " ".repeat(1_000_000);
    let p = layout(
        &[PanelTextFragment::Text(&spaces, style())],
        300.,
        228.,
        &mut m,
    )
    .unwrap();
    assert_eq!(p.scanned_bytes, 1);
    let text = "x".repeat(1_000_000);
    m.tiny = true;
    let p = layout(
        &[PanelTextFragment::Text(&text, style())],
        600.,
        228.,
        &mut m,
    )
    .unwrap();
    assert_eq!(p.scanned_bytes, PANEL_SCAN_BYTES);
    assert!(p.runs.len() <= 64);
}
#[test]
fn invalid_metrics_are_transactional_and_invisible_fragments_are_unvisited() {
    let mut m = Metrics {
        invalid: true,
        ..Metrics::default()
    };
    let f = [
        PanelTextFragment::Text("good", style()),
        PanelTextFragment::Break,
        PanelTextFragment::Text("!", style()),
    ];
    assert!(layout(&f, 100., 32., &mut m).is_err());
    assert!(layout(&f, 100., 16., &mut m).is_ok());
    let mut bad = style();
    bad.size = f64::INFINITY;
    assert!(layout(&[PanelTextFragment::Text("bad", bad)], 100., 32., &mut m).is_err());
}

#[test]
fn slot_limit_stops_before_invalid_metrics_in_the_invisible_styled_suffix() {
    let mut fragments = Vec::new();
    for i in 0..64 {
        fragments.push(PanelTextFragment::Text(
            "a",
            TextStyle {
                color: [i as u8; 3],
                ..style()
            },
        ));
    }
    fragments.push(PanelTextFragment::Text("!", style()));
    let mut m = Metrics {
        invalid: true,
        ..Metrics::default()
    };
    let p = layout(&fragments, 600., 16., &mut m).unwrap();
    assert_eq!(p.runs.len(), 64);
    assert!(p.scanned_bytes <= 65);
}

#[test]
fn collapsed_space_prefix_does_not_spend_visible_quota() {
    for count in [4096, 1_000_000] {
        let text = " ".repeat(count) + "hello";
        let admitted = PanelText::new(&[PanelTextFragment::Text(&text, style())]).unwrap();
        let block = PanelTextBlock {
            text: &admitted,
            bounds: BodyRect {
                x: 0.,
                y: 0.,
                width: 100.,
                height: 16.,
            },
            line_height: 16.,
            device_scale: 1.,
            max_runs: 64,
        };
        let mut m = Metrics::default();
        for _ in 0..2 {
            let p = layout_panel_text(&block, &mut m).unwrap();
            assert_eq!(p.runs[0].text, "hello");
            assert_eq!(p.scanned_bytes, 6);
        }
    }
    let too_long = " ".repeat(PANEL_INPUT_BYTES + 1);
    assert!(PanelText::new(&[PanelTextFragment::Text(&too_long, style())]).is_err());
    assert!(PanelText::new(&vec![PanelTextFragment::Break; PANEL_INPUT_FRAGMENTS + 1]).is_err());
}
#[test]
fn collapsed_space_keeps_its_first_inline_style_and_advance() {
    let f = [
        PanelTextFragment::Text("a", style()),
        PanelTextFragment::Text(
            " ",
            TextStyle {
                size: 24.,
                ..style()
            },
        ),
        PanelTextFragment::Text(
            " \t\n",
            TextStyle {
                size: 8.,
                ..style()
            },
        ),
        PanelTextFragment::Text("b", style()),
    ];
    let p = layout(&f, 100., 16., &mut Metrics::default()).unwrap();
    assert_eq!(
        p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["a", " ", "b"]
    );
    assert_eq!(p.runs[1].style.size, 24.);
    assert_eq!(p.runs[2].position, [22., 17.]);
}
#[test]
fn cache_chunks_and_overlong_word_wraps_preserve_combining_clusters() {
    let text = "i".repeat(1023) + "e\u{301}";
    let mut m = Metrics {
        tiny: true,
        ..Metrics::default()
    };
    let p = layout(
        &[PanelTextFragment::Text(
            &text,
            TextStyle {
                size: 1.,
                ..style()
            },
        )],
        600.,
        16.,
        &mut m,
    )
    .unwrap();
    assert_eq!(
        p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["i".repeat(1023).as_str(), "e\u{301}"]
    );
    assert!(p.runs.iter().all(|r| r.text.len() <= 1024));
    let p = layout(
        &[PanelTextFragment::Text("ae\u{301}", style())],
        16.,
        32.,
        &mut Metrics::default(),
    )
    .unwrap();
    assert_eq!(
        p.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["a", "e\u{301}"]
    );
    assert_eq!(p.runs[1].position[1], 31.);
    let f = [
        PanelTextFragment::Text("e", style()),
        PanelTextFragment::Text(
            "\u{301}",
            TextStyle {
                color: [1; 3],
                ..style()
            },
        ),
    ];
    let p = layout(&f, 100., 16., &mut Metrics::default()).unwrap();
    assert_eq!(p.runs[0].text, "e\u{301}");
    assert_eq!(p.runs[0].style, style());
}
#[test]
fn one_cluster_over_the_cache_cap_has_an_explicit_bounded_projection() {
    let text = "e".to_owned() + &"\u{301}".repeat(512) + " ok";
    let p = layout(
        &[PanelTextFragment::Text(&text, style())],
        100.,
        16.,
        &mut Metrics::default(),
    )
    .unwrap();
    assert_eq!(p.runs[0].text, "\u{fffd} ok");
    assert_eq!(text.len(), 1028);
}
#[test]
fn line_baselines_follow_centered_font_struts_not_varying_ink() {
    let f = [
        PanelTextFragment::Text("ace", style()),
        PanelTextFragment::Break,
        PanelTextFragment::Text("Ace", style()),
        PanelTextFragment::Break,
        PanelTextFragment::Text("...", style()),
    ];
    let p = layout(&f, 100., 48., &mut Metrics::default()).unwrap();
    assert_eq!(
        p.runs.iter().map(|r| r.position[1]).collect::<Vec<_>>(),
        [15., 31., 47.]
    );
    let mut m = Metrics::default();
    assert_ne!(
        m.measure("ace", style()).unwrap().ascent,
        m.measure("Ace", style()).unwrap().ascent
    );
    assert_ne!(
        m.measure("...", style()).unwrap().ascent,
        m.measure("Ace", style()).unwrap().ascent
    );
}

#[test]
fn mixed_font_maximum_ascent_and_descent_form_one_centered_strut() {
    struct MixedMetrics;
    impl TextMetrics for MixedMetrics {
        fn measure(
            &mut self,
            text: &str,
            style: TextStyle,
        ) -> Result<TextMeasurement, &'static str> {
            let (a, d) = if style.family == TextFamily::Arial {
                (22., 5.)
            } else {
                (20., 8.)
            };
            Ok(TextMeasurement {
                advance: text.len() as f64 * 8.,
                ascent: 10.,
                descent: 0.,
                font_ascent: a,
                font_descent: d,
            })
        }
    }
    let arial = TextStyle {
        size: 24.,
        ..style()
    };
    let lucida = TextStyle {
        size: 18.,
        family: TextFamily::Lucida,
        ..style()
    };
    for reverse in [false, true] {
        let mut fragments = [
            PanelTextFragment::Text("Arial ", arial),
            PanelTextFragment::Text("Lucida", lucida),
        ];
        if reverse {
            fragments.reverse();
        }
        let admitted = PanelText::new(&fragments).unwrap();
        for (height, expected) in [(32., 23.), (16., 15.)] {
            let p = layout_panel_text(
                &PanelTextBlock {
                    text: &admitted,
                    bounds: BodyRect {
                        x: 0.,
                        y: 3.,
                        width: 300.,
                        height,
                    },
                    line_height: height,
                    device_scale: 1.,
                    max_runs: 48,
                },
                &mut MixedMetrics,
            )
            .unwrap();
            assert_eq!(p.runs.len(), 2);
            assert!(p.runs.iter().all(|r| r.position[1] == 3. + expected));
            assert!(
                p.runs
                    .iter()
                    .all(|r| r.clip.y == 3. && r.clip.height == height)
            );
        }
    }
}

#[test]
fn logical_struts_and_compact_cells_at_fractional_dpr() {
    struct FontTables {
        sizes: Vec<f64>,
    }
    impl TextMetrics for FontTables {
        fn measure(
            &mut self,
            text: &str,
            style: TextStyle,
        ) -> Result<TextMeasurement, &'static str> {
            self.sizes.push(style.size);
            let (a, d) = if style.family == TextFamily::Arial {
                (1854., 434.)
            } else {
                (2246., 901.)
            };
            Ok(TextMeasurement {
                advance: text.len() as f64 * style.size / 2.,
                ascent: style.size / 2.,
                descent: 0.,
                font_ascent: style.size * a / 2048.,
                font_descent: style.size * d / 2048.,
            })
        }
    }
    let arial = TextStyle {
        size: 24.,
        ..style()
    };
    let lucida = TextStyle {
        size: 18.,
        family: TextFamily::Lucida,
        ..style()
    };
    for scale in [1., 1.25, 2.] {
        for (styles, height, expected) in [
            (vec![arial, lucida], 32., 22.90380859375),
            (vec![arial, lucida], 16., 14.90380859375),
            (vec![lucida], 32., 21.91064453125),
            (vec![arial], 16.984375, 16.8125),
            (
                vec![
                    TextStyle {
                        size: 23.5,
                        ..arial
                    },
                    TextStyle {
                        size: 18.25,
                        ..lucida
                    },
                ],
                31.5,
                22.37249755859375,
            ),
        ] {
            let input: Vec<_> = styles
                .iter()
                .map(|s| PanelTextFragment::Text("A", *s))
                .collect();
            let text = PanelText::new(&input).unwrap();
            let mut metrics = FontTables { sizes: Vec::new() };
            let block = PanelTextBlock {
                text: &text,
                bounds: BodyRect {
                    x: 0.,
                    y: 0.,
                    width: 300.,
                    height,
                },
                line_height: height,
                device_scale: scale,
                max_runs: 48,
            };
            let p = layout_panel_text(&block, &mut metrics).unwrap();
            assert!(
                p.runs
                    .iter()
                    .all(|r| (r.position[1] - expected).abs() < 1e-10)
            );
            assert!(p.runs.iter().all(|r| r.clip.height == height));
            assert!(metrics.sizes.contains(&(styles[0].size * scale)));
        }
    }
    let text = PanelText::new(&[PanelTextFragment::Text("A", arial)]).unwrap();
    for scale in [0., -1., f64::NAN, f64::INFINITY, 8193.] {
        let mut m = FontTables { sizes: Vec::new() };
        assert!(
            layout_panel_text(
                &PanelTextBlock {
                    text: &text,
                    bounds: BodyRect {
                        x: 0.,
                        y: 0.,
                        width: 300.,
                        height: 32.
                    },
                    line_height: 32.,
                    device_scale: scale,
                    max_runs: 48
                },
                &mut m
            )
            .is_err()
        );
        assert!(m.sizes.is_empty());
    }
}
