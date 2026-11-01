//! Passive render diagnostics remain separate from the latency trace schema.
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_wgpu_slice::{latency, render};
thread_local! {
    static PHASES: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) };
}
fn observe(stage: &'static str, _: u64) {
    PHASES.with_borrow_mut(|rows| rows.push(stage));
}
#[test]
fn render_observer_preserves_pictures_and_does_not_extend_latency_vocabulary() {
    let mut a = tape::playing();
    let mut b = tape::playing();
    let mut ca = a.client().clone();
    let mut cb = b.client().clone();
    let quality = a.quality();
    let hud = a.hud_view();
    let ordinary_recorder = latency::Recorder::new(64);
    let ordinary_recording = ordinary_recorder.attach();
    let p = render::prepare_gameplay(&mut ca, &mut a.bank, [960, 540], 1., 8000, quality).unwrap();
    let p = render::finish_hud(p, &mut a.bank, &hud).unwrap();
    drop(ordinary_recording);
    let recorder = latency::Recorder::new(64);
    let recording = recorder.attach();
    let old = render::set_preparation_observer(Some(observe));
    let q = render::prepare_gameplay(&mut cb, &mut b.bank, [960, 540], 1., 8000, quality).unwrap();
    let q = render::finish_hud(q, &mut b.bank, &hud).unwrap();
    render::set_preparation_observer(old);
    drop(recording);
    assert_eq!(p.frame, q.frame);
    assert_eq!(p.overlay, q.overlay);
    assert_eq!(p.resources.len(), q.resources.len());
    for (a, b) in p.resources.iter().zip(&q.resources) {
        assert_eq!(a.asset, b.asset);
        assert_eq!(a.image.pixels, b.image.pixels);
    }
    PHASES.with_borrow(|rows| {
        assert_eq!(
            rows.as_slice(),
            [
                "build_begin",
                "curves_end",
                "background_end",
                "food_end",
                "prey_end",
                "bodies_end",
                "arena_end",
                "names_end",
                "resources_end",
                "minimap_end"
            ]
        )
    });
    let (rows, dropped) = recorder.snapshot();
    assert_eq!(dropped, 0);
    let (ordinary_rows, ordinary_dropped) = ordinary_recorder.snapshot();
    assert_eq!(ordinary_dropped, 0);
    assert_eq!(
        rows.iter().map(|r| r.stage).collect::<Vec<_>>(),
        ordinary_rows.iter().map(|r| r.stage).collect::<Vec<_>>()
    );
}
