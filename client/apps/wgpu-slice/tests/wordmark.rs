//! Authored ASCII, geometry, motion and raster bounds; no external inputs.
use slither_client::menu_layout::MenuRect;
use slither_wgpu_slice::wordmark::*;

fn holder() -> MenuRect {
    MenuRect {
        x: 735.,
        y: 50.,
        width: 450.,
        height: 115.,
    }
}
fn make(text: &str, width: f64) -> Layout {
    layout(
        text,
        MenuRect {
            x: width / 2. - 225.,
            ..holder()
        },
        width,
        |s| Ok(s.chars().count() as f64 * 7.),
    )
    .unwrap()
}

#[test]
fn complete_printable_ascii_and_whole_word_fallback() {
    for c in ' '..='~' {
        assert!(glyph(c).is_some(), "{c}");
        let l = make(&c.to_string(), 1920.);
        assert!(samples(&l, 1.).is_ok(), "{c}");
        if c != ' ' {
            assert!(!samples(&l, 1.).unwrap().is_empty(), "{c}");
        }
    }
    let l = make("Hello Wørld 123", 1920.);
    assert_eq!(
        l.words.iter().map(|w| w.fallback).collect::<Vec<_>>(),
        [false, true, false]
    );
    assert_eq!(l.words[1].text, "Wørld");
    assert!(samples(&make("Wørld", 1920.), 1.).unwrap().is_empty());
    assert_eq!(
        make("SlitherClient", 1920.)
            .words
            .iter()
            .map(|w| w.color)
            .collect::<Vec<_>>(),
        [0, 1]
    );
}

#[test]
fn layout_is_deterministic_natural_size_and_uniform_narrow_fit() {
    let a = make(DEFAULT_TEXT, 1920.);
    assert_eq!(a, make(DEFAULT_TEXT, 1920.));
    assert!(a.rect.width > 450.);
    assert_eq!(a.rect.height, 115.);
    assert!(make("HelloHello", 1920.).rect.width > make("Hello", 1920.).rect.width);
    let narrow = make(DEFAULT_TEXT, 320.);
    assert!((narrow.rect.x - 16.).abs() < 1e-8);
    assert!((narrow.rect.width - 288.).abs() < 1e-8);
    assert!((narrow.rect.width / a.rect.width - narrow.rect.height / a.rect.height).abs() < 1e-8);
    assert!((narrow.rect.y + narrow.rect.height / 2. - 107.5).abs() < 1e-8);
    assert!(layout(&"a".repeat(MAX_CHARACTERS + 1), holder(), 1920., |_| Ok(1.)).is_err());
}

#[test]
fn every_word_uses_one_snapped_origin_and_rounded_physical_advance() {
    for dpr in [1., 1.25, 1.5, 1.75, 2., 3., 5.] {
        for offset in [0., 0.13, 0.49, 0.77] {
            let l = layout(
                "Hello Wørld 123",
                MenuRect {
                    x: holder().x + offset,
                    ..holder()
                },
                1920.,
                |s| Ok(s.chars().count() as f64 * 7.),
            )
            .unwrap();
            let origin = ((l.rect.x + l.words[0].x * l.unit) * dpr).round();
            for word in &l.words {
                assert_eq!(
                    l.word_x_pixels(word, dpr) - origin,
                    ((word.x - l.words[0].x) * l.unit * dpr).round()
                );
                assert_eq!(l.word_x_pixels(word, dpr).fract(), 0.);
            }
        }
    }
}

#[test]
fn painted_second_word_stem_is_centred_on_its_rounded_advance() {
    let l = make("lL", 1920.);
    for dpr in [1., 1.25, 1.5, 1.75, 2.] {
        let image = raster(&l, dpr, 1.).unwrap();
        // l advances 2 units plus 1.25 tracking. Derive the L's actual
        // painted stem from the first word's snapped origin, not a second
        // independently rounded absolute position.
        let center = ((l.rect.x + 3. * l.unit) * dpr).round() - (l.rect.x * dpr).round()
            + (3.25 * l.unit * dpr).round();
        let y = (6. * l.unit * dpr).round() as u32;
        let mut total = 0.;
        let mut moment = 0.;
        for x in (center - 1.1 * l.unit * dpr).floor() as u32
            ..(center + 1.1 * l.unit * dpr).ceil() as u32
        {
            let blue = f64::from(image.pixels[((y * image.width + x) * 4 + 2) as usize]);
            total += blue;
            moment += (f64::from(x) + 0.5) * blue;
        }
        assert!(
            (moment / total - center).abs() < 0.08,
            "DPR {dpr}: painted {} expected {center}",
            moment / total
        );
    }
}

#[test]
fn swelling_and_ripple_settle_exactly_and_progress_is_bounded() {
    let l = make(DEFAULT_TEXT, 1920.);
    let settled = samples(&l, 1.).unwrap();
    assert!(samples(&l, 0.).unwrap().is_empty());
    assert_eq!(samples(&l, -1.).unwrap().len(), 0);
    let early = samples(&l, 0.15).unwrap();
    assert!(!early.is_empty() && early.len() < settled.len());
    assert!(early.iter().any(|s| s.radius < settled[0].radius * 0.5));
    let mid = samples(&l, 0.5).unwrap();
    let inflated = samples(&l, 0.32).unwrap();
    assert_eq!(inflated.len(), settled.len());
    assert!(
        inflated
            .iter()
            .zip(&settled)
            .all(|(a, b)| a.radius == b.radius)
    );
    assert_eq!(mid.len(), settled.len());
    assert!(mid.iter().zip(&settled).any(|(a, b)| a.center != b.center));
    assert!(mid.iter().zip(&settled).all(|(a, b)| a.radius == b.radius));
    assert_eq!(settled.len(), samples(&l, 9.).unwrap().len());
    for (a, b) in settled.iter().zip(samples(&l, 9.).unwrap()) {
        assert_eq!(a.center, b.center);
        assert_eq!(a.radius, b.radius);
    }
    for s in settled.iter().chain(&early).chain(&mid) {
        assert!(s.radius.is_finite() && s.radius > 0. && s.radius <= 0.86 * l.unit);
        assert!(s.center[0] - s.radius >= 0. && s.center[0] + s.radius < l.rect.width);
        assert!(s.center[1] - s.radius >= 0. && s.center[1] + s.radius < l.rect.height);
    }
    assert!(samples(&l, f64::NAN).is_err());
}

#[test]
fn raster_budget_premultiplication_and_reveal_endpoints_at_dpr_1_2_5() {
    let l = make(DEFAULT_TEXT, 1920.);
    for dpr in [1., 2., 5.] {
        let size = raster_size(&l, dpr).unwrap();
        for progress in [0., 0.15, 0.5, 1.] {
            let image = raster(&l, dpr, progress).unwrap();
            assert_eq!([image.width, image.height], size);
            assert_eq!(image.pixels.len(), size[0] as usize * size[1] as usize * 4);
            assert!(image.pixels.len() <= MAX_RASTER_BYTES);
            assert!(
                image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[..3].iter().all(|c| *c <= p[3]))
            );
            assert_eq!(image.pixels.iter().all(|n| *n == 0), progress == 0.);
        }
    }
    assert!(raster_size(&l, 0.).is_err());
    assert!(raster_size(&l, f64::INFINITY).is_err());
}

#[test]
fn extreme_dpr_fits_strokes_without_changing_ordinary_layouts() {
    let original = make(DEFAULT_TEXT, 1920.);
    let center = [
        original.rect.x + original.rect.width / 2.,
        original.rect.y + original.rect.height / 2.,
    ];
    for dpr in [1., 2., 5., 6., 8.] {
        let mut layout = original.clone();
        layout.fit_raster(dpr).unwrap();
        let size = raster_size(&layout, dpr).unwrap();
        assert!(size[0] as usize * size[1] as usize * 4 <= MAX_RASTER_BYTES);
        assert!(size[1] <= 2048);
        assert!(layout.words.iter().all(|w| !w.fallback));
        assert!((layout.rect.x + layout.rect.width / 2. - center[0]).abs() < 1e-9);
        assert!((layout.rect.y + layout.rect.height / 2. - center[1]).abs() < 1e-9);
        if dpr <= 5. {
            assert_eq!(layout, original);
        }
    }
    for dpr in [0., f64::NAN, f64::INFINITY] {
        assert!(original.clone().fit_raster(dpr).is_err());
    }
}
