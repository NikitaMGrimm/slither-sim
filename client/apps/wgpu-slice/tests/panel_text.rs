//! Authored bounded panels; no game server or native window.
use slither_client::presentation::*;
use slither_wgpu_slice::{
    render::{PANEL_CONTROL_DRAWS, PANEL_PRODUCER_DRAWS},
    text::*,
};

fn style() -> TextStyle {
    TextStyle {
        size: 18.,
        family: TextFamily::Arial,
        slant: TextSlant::Italic,
        weight: TextWeight::Normal,
        color: [255; 3],
    }
}
#[test]
fn aggregate_clipped_allocation_envelope_includes_overhang_rounding_and_tiles() {
    // 15 partial/full cells in each 228px panel at the minimum 16px line height.
    // Full rectangular coverage plus 2px rounding per row, per-run one-em ink
    // overhang on both sides, rounding per run, and both tile neighbours.
    let scale = 3_usize;
    let lines = (228_f64 / PANEL_MIN_LINE_HEIGHT).ceil() as usize;
    let overhang = 2 * PANEL_MAX_FONT_SIZE as usize * scale + 2;
    let max_height = PANEL_MAX_LINE_HEIGHT as usize * scale + 2;
    let slots = [48, 16];
    let widths = [300 * scale, 600 * scale];
    let text_draws =
        std::array::from_fn(|i| slots[i] + (widths[i] * lines + slots[i] * overhang) / 510);
    let rect = widths.iter().sum::<usize>() * (228 * scale + 2 * lines);
    let overlaps = slots.iter().sum::<usize>() * overhang * max_height;
    let seams = 2 * text_draws.iter().sum::<usize>() * max_height;
    let bytes = 4 * (rect + overlaps + seams);
    assert_eq!((lines, text_draws, bytes), (15, [88, 73], 11_500_272));
    preflight_panel_totals(bytes, text_draws).unwrap();
    assert!(bytes > 8 * 1024 * 1024 && bytes <= PANEL_CACHE_BYTES);
    assert!(preflight_panel_totals(PANEL_CACHE_BYTES + 1, [0, 0]).is_err());
    assert!(preflight_panel_totals(0, [89, 0]).is_err());
    assert_eq!(
        (PANEL_CACHE_RUNS, TOTAL_CACHE_RUNS, TOTAL_CACHE_BYTES),
        (64, 386, 22 * 1024 * 1024)
    );
    assert_eq!((PANEL_PRODUCER_DRAWS, PANEL_CONTROL_DRAWS), (96, 8));
}

#[cfg(windows)]
fn plan(cache: &mut TextCache, text: &str, width: f64, max_runs: usize) -> PanelTextPlan {
    layout_panel_text(
        &PanelTextBlock {
            text: &PanelText::new(&[PanelTextFragment::Text(text, style())]).unwrap(),
            bounds: BodyRect {
                x: 0.,
                y: 0.,
                width,
                height: 228.,
            },
            line_height: 24.,
            device_scale: 1.,
            max_runs,
        },
        cache,
    )
    .unwrap()
}
#[cfg(windows)]
#[test]
fn native_faces_are_real_and_family_slant_weight_change_pixels() {
    let mut c = TextCache::default();
    let mut images = Vec::new();
    for (family, slant, weight, file) in [
        (Family::Arial, Slant::Normal, Weight::Normal, "arial.ttf"),
        (Family::Arial, Slant::Italic, Weight::Normal, "ariali.ttf"),
        (Family::Arial, Slant::Italic, Weight::Bold, "arialbi.ttf"),
        (Family::Lucida, Slant::Normal, Weight::Normal, "l_10646.ttf"),
    ] {
        let style = TextStyle {
            family,
            slant,
            weight,
            ..style()
        };
        assert_eq!(TextCache::native_face(style), file);
        let mut p = plan(&mut c, "AV fi café", 300., 48);
        p.runs[0].style = style;
        let out = c
            .draw_panel_composition(
                &[PanelSubmission {
                    group: PanelGroup::Announcement,
                    plan: &p,
                    alpha: 1.,
                }],
                1.,
            )
            .unwrap();
        images.push(out[0][0].1.image.pixels.clone());
    }
    for i in 0..images.len() {
        for j in 0..i {
            assert_ne!(images[i], images[j]);
        }
    }
    assert_eq!(
        TextCache::native_face(TextStyle {
            family: Family::Lucida,
            ..style()
        }),
        "l_10646.ttf"
    );
}

#[cfg(windows)]
#[test]
fn chunk_positions_keep_boundary_kerning_from_complete_native_run() {
    let mut c = TextCache::default();
    // A tiny but valid style fits >1024 bytes in one box; AV kerns at the seam.
    let s = TextStyle {
        size: 1.,
        slant: Slant::Normal,
        ..style()
    };
    let text = format!("{}AV", "i".repeat(1023));
    let p = layout_panel_text(
        &PanelTextBlock {
            text: &PanelText::new(&[PanelTextFragment::Text(&text, s)]).unwrap(),
            bounds: BodyRect {
                x: 0.,
                y: 0.,
                width: 600.,
                height: 16.,
            },
            line_height: 16.,
            device_scale: 1.,
            max_runs: 48,
        },
        &mut c,
    )
    .unwrap();
    assert_eq!(p.runs.len(), 2);
    let before = c.measure_panel(&text[..1024], s).unwrap().advance;
    let full = c.measure_panel(&text, s).unwrap().advance;
    let v = c.measure_panel("V", s).unwrap().advance;
    assert!((p.runs[1].position[0] - (full - v)).abs() < 1e-8);
    assert!((p.runs[1].position[0] - before).abs() > 1e-5);
}

#[cfg(windows)]
#[test]
fn panels_replace_slots_preserve_other_groups_and_old_prepared_resources_at_dpr_1_2_16() {
    use slither_wgpu_slice::{bank::Bank, frame::Frame, render::PreparedFrame};
    let mut c = TextCache::default();
    let run = TextRun {
        text: "HUD nickname minimap",
        style: TextStyle {
            slant: Slant::Normal,
            ..style()
        },
        position: [0., 0.],
        align: Align::Left,
        baseline: Baseline::Alphabetic,
        alpha: 1.,
    };
    let (_, hud) = c.draw_hud(0, TextRun { ..run }, 1., None).unwrap();
    let nick = c.draw(TextRun { ..run }, 1.).unwrap();
    let mini = c
        .draw_minimap(TextRun { ..run }, 1., [0., 1.], 8., 160.)
        .unwrap();
    let old_runs = c.cached_runs();
    let old_bytes = c.cached_bytes();
    let a = plan(
        &mut c,
        &"AV announcement italic words ".repeat(100),
        300.,
        48,
    );
    let b = plan(&mut c, &"Result and editor text ".repeat(100), 600., 16);
    let mut first = None;
    for dpr in [1., 2., 16.] {
        let out = c
            .draw_panel_composition(
                &[
                    PanelSubmission {
                        group: PanelGroup::Announcement,
                        plan: &a,
                        alpha: 0.6,
                    },
                    PanelSubmission {
                        group: PanelGroup::Result,
                        plan: &b,
                        alpha: 1.,
                    },
                ],
                dpr,
            )
            .unwrap();
        assert!(c.cached_bytes() <= old_bytes + PANEL_CACHE_BYTES);
        assert!(c.cached_runs() <= old_runs + 64);
        let draws = out.iter().flatten().map(|p| p.0.clone()).collect();
        let resources = out.into_iter().flatten().map(|p| p.1).collect();
        let p = PreparedFrame {
            frame: Frame {
                identity: 1,
                extent: [960, 540],
                viewport_scale: 1.,
                tick: 0,
                draws,
                snakes: vec![],
            },
            resources,
        };
        let resident = p
            .resources
            .iter()
            .map(|r| (r.asset.into(), r.asset))
            .collect();
        p.frame.quads(&resident).unwrap();
        if first.is_none() {
            first = Some(p);
        }
    }
    let runs = c.cached_runs();
    // Moving a block and changing alpha leave relative clips and raster keys fixed.
    let out = c
        .draw_panel_composition(
            &[
                PanelSubmission {
                    group: PanelGroup::Announcement,
                    plan: &a,
                    alpha: 0.1,
                },
                PanelSubmission {
                    group: PanelGroup::Result,
                    plan: &b,
                    alpha: 0.4,
                },
            ],
            16.,
        )
        .unwrap();
    let asset = out[0][0].0.asset;
    let mut moved = a.clone();
    for r in &mut moved.runs {
        r.position[0] += 10.;
        r.clip.x += 10.;
    }
    let out = c
        .draw_panel_composition(
            &[
                PanelSubmission {
                    group: PanelGroup::Announcement,
                    plan: &moved,
                    alpha: 0.4,
                },
                PanelSubmission {
                    group: PanelGroup::Result,
                    plan: &b,
                    alpha: 0.8,
                },
            ],
            16.,
        )
        .unwrap();
    assert_eq!(out[0][0].0.asset, asset);
    assert_eq!(c.cached_runs(), runs);
    for i in 0..12 {
        let a = plan(&mut c, &format!("replacement {i}"), 300., 48);
        c.draw_panel_composition(
            &[PanelSubmission {
                group: PanelGroup::Announcement,
                plan: &a,
                alpha: 1.,
            }],
            1.,
        )
        .unwrap();
        assert_eq!(c.cached_runs(), old_runs + 1);
    }
    // Failed preflight preserves existing panel slots and handles.
    let bad = PanelTextPlan {
        runs: vec![a.runs[0].clone(); 49],
        scanned_bytes: 0,
    };
    let count = c.cached_runs();
    let bytes = c.cached_bytes();
    assert!(
        c.draw_panel_composition(
            &[PanelSubmission {
                group: PanelGroup::Announcement,
                plan: &bad,
                alpha: 1.
            }],
            1.
        )
        .is_err()
    );
    assert_eq!((c.cached_runs(), c.cached_bytes()), (count, bytes));
    c.draw_panel_composition(&[], 1.).unwrap();
    assert_eq!((c.cached_runs(), c.cached_bytes()), (old_runs, old_bytes));
    assert_eq!(
        c.draw_hud(0, TextRun { ..run }, 1., None).unwrap().1[0]
            .0
            .asset,
        hud[0].0.asset
    );
    assert_eq!(
        c.draw(TextRun { ..run }, 1.).unwrap()[0].0.asset,
        nick[0].0.asset
    );
    assert_eq!(
        c.draw_minimap(TextRun { ..run }, 1., [0., 1.], 8., 160.)
            .unwrap()[0]
            .0
            .asset,
        mini[0].0.asset
    );
    let p = first.unwrap();
    let mut bank = Bank::new().unwrap();
    bank.stage_text(1, p.resources.clone()).unwrap();
    let resources = bank.resources(&p.frame).unwrap();
    assert_eq!(resources.len(), p.resources.len());
    assert_eq!(resources[0].image.pixels, p.resources[0].image.pixels);
}

#[cfg(windows)]
#[test]
fn late_raster_failure_does_not_publish_any_replacement_or_retirement() {
    let mut c = TextCache::default();
    let a = plan(&mut c, "retained announcement", 300., 48);
    let out = c
        .draw_panel_composition(
            &[PanelSubmission {
                group: PanelGroup::Announcement,
                plan: &a,
                alpha: 1.,
            }],
            1.,
        )
        .unwrap();
    let asset = out[0][0].0.asset;
    let stats = (c.cached_runs(), c.cached_bytes());
    let replacement = plan(&mut c, "new announcement", 300., 48);
    let mut bad = plan(&mut c, "result", 600., 16);
    bad.runs[0].text = "x".repeat(1025);
    assert!(
        c.draw_panel_composition(
            &[
                PanelSubmission {
                    group: PanelGroup::Announcement,
                    plan: &replacement,
                    alpha: 1.
                },
                PanelSubmission {
                    group: PanelGroup::Result,
                    plan: &bad,
                    alpha: 1.
                }
            ],
            1.
        )
        .is_err()
    );
    assert_eq!((c.cached_runs(), c.cached_bytes()), stats);
    let out = c
        .draw_panel_composition(
            &[PanelSubmission {
                group: PanelGroup::Announcement,
                plan: &a,
                alpha: 0.3,
            }],
            1.,
        )
        .unwrap();
    assert_eq!(out[0][0].0.asset, asset);
}
#[cfg(windows)]
#[test]
fn new_faces_use_the_existing_fill_luminance_coverage_transfer() {
    let mut c = TextCache::default();
    for family in [Family::Arial, Family::Lucida] {
        for slant in [Slant::Normal, Slant::Italic] {
            for weight in [Weight::Normal, Weight::Bold] {
                let mut p = plan(&mut c, "AV café", 300., 48);
                p.runs[0].style = TextStyle {
                    family,
                    slant,
                    weight,
                    color: [0; 3],
                    ..style()
                };
                let dark = c
                    .draw_panel_composition(
                        &[PanelSubmission {
                            group: PanelGroup::Announcement,
                            plan: &p,
                            alpha: 1.,
                        }],
                        1.,
                    )
                    .unwrap();
                p.runs[0].style.color = [255; 3];
                let light = c
                    .draw_panel_composition(
                        &[PanelSubmission {
                            group: PanelGroup::Announcement,
                            plan: &p,
                            alpha: 1.,
                        }],
                        1.,
                    )
                    .unwrap();
                let alpha = |v: &PanelDraws| {
                    v[0].iter()
                        .flat_map(|(_, r)| {
                            r.image
                                .pixels
                                .as_chunks::<4>()
                                .0
                                .iter()
                                .map(|p| u64::from(p[3]))
                        })
                        .sum::<u64>()
                };
                assert!(alpha(&dark) < alpha(&light));
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn replacement_admission_uses_final_pixels_in_both_producer_orders() {
    // Deliberately overlapping, authored cache-pressure plans. Their aggregate
    // bytes, not the normal layout envelope, exercise transaction admission.
    let text = "\u{c1}gj".repeat(30);
    let run = PanelTextRun {
        text,
        style: TextStyle {
            size: 24.,
            slant: Slant::Normal,
            ..style()
        },
        position: [0., 24.],
        clip: BodyRect {
            x: 0.,
            y: 0.,
            width: 160.,
            height: 32.,
        },
    };
    let mut a = PanelTextPlan {
        runs: vec![run.clone(); 48],
        scanned_bytes: 0,
    };
    a.runs[0].clip.width = 20.;
    let mut b = PanelTextPlan {
        runs: vec![
            PanelTextRun {
                clip: BodyRect {
                    width: 20.,
                    ..run.clip
                },
                ..run
            };
            16
        ],
        scanned_bytes: 0,
    };
    let draw = |c: &mut TextCache, a: &PanelTextPlan, b: &PanelTextPlan, reverse: bool| {
        let mut input = [
            PanelSubmission {
                group: PanelGroup::Announcement,
                plan: a,
                alpha: 1.,
            },
            PanelSubmission {
                group: PanelGroup::Result,
                plan: b,
                alpha: 1.,
            },
        ];
        if reverse {
            input.reverse();
        }
        c.draw_panel_composition(&input, 3.)
    };
    for reverse in [false, true] {
        let mut c = TextCache::default();
        let (mut lo, mut hi) = (20_u32, 600_u32);
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            for r in &mut b.runs {
                r.clip.width = f64::from(mid);
            }
            if draw(&mut c, &a, &b, false).is_ok() {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        for r in &mut b.runs {
            r.clip.width = f64::from(lo);
        }
        let old = draw(&mut c, &a, &b, false).unwrap();
        let before = c.cached_bytes();
        assert!(
            PANEL_CACHE_BYTES - before < 30_000,
            "headroom {}",
            PANEL_CACHE_BYTES - before
        );
        let unchanged = old[0][1].0.asset;
        let mut grown = a.clone();
        grown.runs[0].clip.width = 160.;
        // Growing alone cannot fit; a failed transaction must preserve all old
        // pixels/handles, including the slots tentatively retired before work.
        assert!(draw(&mut c, &grown, &b, reverse).is_err());
        assert_eq!(c.cached_bytes(), before);
        assert_eq!(
            draw(&mut c, &a, &b, false).unwrap()[0][1].0.asset,
            unchanged
        );
        let mut shrunk = b.clone();
        shrunk.runs[15].clip.width = 20.;
        let new = draw(&mut c, &grown, &shrunk, reverse).unwrap();
        assert_eq!(c.cached_runs(), 64);
        assert!(c.cached_bytes() < before);
        let ann = if reverse { &new[1] } else { &new[0] };
        assert_eq!(ann[1].0.asset, unchanged);
    }
}
#[cfg(windows)]
#[test]
fn installed_windows_font_boxes_are_independent_of_ink_and_lucida_keeps_its_family() {
    let mut c = TextCache::default();
    for (family, file) in [
        (Family::Arial, "arial.ttf"),
        (Family::Lucida, "l_10646.ttf"),
    ] {
        let s = TextStyle {
            family,
            slant: Slant::Normal,
            ..style()
        };
        let reference = c.measure_panel("", s).unwrap();
        assert!(reference.font_ascent > 0. && reference.font_descent > 0.);
        for text in ["ace", "Ace", "..."] {
            let m = c.measure_panel(text, s).unwrap();
            assert_eq!(
                (m.font_ascent, m.font_descent),
                (reference.font_ascent, reference.font_descent)
            );
        }
        if family == Family::Lucida {
            for weight in [Weight::Normal, Weight::Bold] {
                for slant in [Slant::Normal, Slant::Italic] {
                    assert_eq!(
                        TextCache::native_face(TextStyle { weight, slant, ..s }),
                        file
                    );
                    assert_eq!(
                        c.measure_panel("Length", TextStyle { weight, slant, ..s })
                            .unwrap()
                            .font_ascent,
                        reference.font_ascent
                    );
                }
            }
        }
    }
    assert_ne!(
        c.measure_panel("ace", style()).unwrap().ascent,
        c.measure_panel("Ace", style()).unwrap().ascent
    );
}

#[cfg(windows)]
#[test]
fn mixed_installed_fonts_center_one_maximum_strut() {
    let mut cache = TextCache::default();
    let arial = TextStyle {
        size: 24.,
        slant: Slant::Normal,
        ..style()
    };
    let lucida = TextStyle {
        size: 18.,
        family: Family::Lucida,
        ..arial
    };
    let text = PanelText::new(&[
        PanelTextFragment::Text("Arial ", arial),
        PanelTextFragment::Text("Lucida", lucida),
    ])
    .unwrap();
    let p = layout_panel_text(
        &PanelTextBlock {
            text: &text,
            bounds: BodyRect {
                x: 0.,
                y: 0.,
                width: 300.,
                height: 32.,
            },
            line_height: 32.,
            device_scale: 1.,
            max_runs: 48,
        },
        &mut cache,
    )
    .unwrap();
    assert_eq!(p.runs.len(), 2);
    let a = cache.measure_panel("", arial).unwrap();
    let l = cache.measure_panel("", lucida).unwrap();
    let expected =
        (32. + a.font_ascent.max(l.font_ascent) - a.font_descent.max(l.font_descent)) / 2.;
    assert!(p.runs.iter().all(|r| r.position[1] == expected));
    assert!((expected - 24.).abs() <= 2.);
}
