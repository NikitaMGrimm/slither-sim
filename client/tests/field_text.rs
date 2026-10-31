//! Authored font metrics exercise a rule shared by every single-line field.
use slither_client::menu_layout::{MenuRect, single_line_field_placement};

#[test]
fn field_line_and_caret_share_the_content_centre_at_each_dpr() {
    for (width, height, inset) in [(220., 24., [2., 0.]), (340., 44., [12., 6.])] {
        let field = MenuRect {
            x: 17.3,
            y: 31.7,
            width,
            height,
        };
        for size in [12., 15., 20., 24., 32.] {
            for (ascent, descent) in [(size * 1.0967, size * 0.44), (size * 0.9053, size * 0.2119)]
            {
                let p = single_line_field_placement(field, inset, size, ascent, descent);
                let center = field.y + field.height / 2.;
                assert_eq!(p.origin[0], field.x + inset[0]);
                assert!((p.origin[1] - ascent + (ascent + descent) / 2. - center).abs() < 1e-10);
                assert!(p.caret_height <= size && p.caret_height <= height - 2. * inset[1]);
                for dpr in [1., 1.25, 1.5, 1.75, 2., 3., 5.] {
                    let snap = |v: f64| (v * dpr).round() / dpr;
                    let line_center =
                        (snap(p.origin[1] - ascent) + snap(p.origin[1] + descent)) / 2.;
                    let caret_center =
                        (snap(p.caret_top) + snap(p.caret_top + p.caret_height)) / 2.;
                    assert!((line_center - center).abs() <= 0.5 / dpr + 1e-10);
                    assert!((caret_center - center).abs() <= 0.5 / dpr + 1e-10);
                }
            }
        }
    }
}
