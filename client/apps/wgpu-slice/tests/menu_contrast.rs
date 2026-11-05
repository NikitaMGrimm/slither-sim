//! Theme pill labels keep WCAG AA text contrast in every enabled state.
use egui::Color32;
use slither_wgpu_slice::egui_theme::{PillState, pill_palette};

/// WCAG 2.2 relative luminance of an opaque sRGB colour.
fn luminance(color: Color32) -> f64 {
    let [r, g, b, a] = color.to_array();
    assert_eq!(a, 255, "pill fills and labels are opaque");
    let channel = |c: u8| {
        let c = f64::from(c) / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}
fn contrast(a: Color32, b: Color32) -> f64 {
    let [light, dark] = {
        let (a, b) = (luminance(a), luminance(b));
        [a.max(b), a.min(b)]
    };
    (light + 0.05) / (dark + 0.05)
}

#[test]
fn pill_labels_keep_aa_contrast_against_every_fill_stop() {
    // Focus adds only an outside ring and checked state only the box, so
    // normal, focused and checked labels share the idle fill. Disabled pills
    // fade with their panel and are exempt (WCAG 1.4.3 inactive components).
    for primary in [true, false] {
        for state in [PillState::Idle, PillState::Hovered, PillState::Pressed] {
            let (fill, label) = pill_palette(primary, state);
            // Every channel moves the same way between the stops, so the
            // gradient's luminance is extreme at a stop.
            for stop in fill {
                let ratio = contrast(label, stop);
                assert!(
                    ratio >= 4.5,
                    "primary {primary} {state:?}: {ratio:.2}:1 against {stop:?}"
                );
            }
        }
    }
    // The checked box's white tick sits on the idle primary green.
    let ratio = contrast(Color32::WHITE, pill_palette(true, PillState::Idle).0[0]);
    assert!(ratio >= 4.5, "tick {ratio:.2}:1");
}
