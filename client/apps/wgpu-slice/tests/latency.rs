//! Authored synthetic CPU schedule, not a network/display latency measurement.
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_wgpu_slice::{
    game::{EffectKind, LocalGame},
    latency,
    render::PreparedFrame,
};

fn effects(g: &mut LocalGame) -> Vec<Vec<u8>> {
    let mut commands = Vec::new();
    for _ in 0..16 {
        let Some(e) = g.effect() else {
            return commands;
        };
        if let EffectKind::Send(ref bytes) = e.kind {
            commands.push(bytes.clone());
            // Explicit synthetic transport receipts, only in this CPU test.
            latency::accepted(e.id);
            latency::flushed();
        }
        if matches!(e.kind, EffectKind::Connect(_)) {
            g.construct(e.generation, e.id, 1.).unwrap();
        }
        g.acknowledge(e.generation, e.id, true).unwrap();
    }
    panic!("effect loop");
}
fn schedule() -> (Vec<Vec<u8>>, Vec<PreparedFrame>) {
    let mut g = LocalGame::new(0.).unwrap();
    g.play("ws://127.0.0.1:19491/slither", "Player", 44)
        .unwrap();
    g.begin_frame(1., [960, 540], 1.).unwrap();
    let picture = g.take_frame().unwrap();
    g.presented(true).unwrap();
    effects(&mut g);
    g.opened(g.generation(), 2.).unwrap();
    effects(&mut g);
    g.receive(g.generation(), &tape::bundle(&[tape::challenge()]), 2.)
        .unwrap();
    effects(&mut g);
    g.receive(
        g.generation(),
        &tape::bundle(&[tape::setup(), tape::snake(44), tape::food()]),
        3.,
    )
    .unwrap();
    tape::pictures(&mut g);
    drop(picture);
    let mut commands = Vec::new();
    let mut pictures = Vec::new();
    for (index, now) in [33., 33.00001, 50., 50.00001, 67., 100., 150., 203.]
        .into_iter()
        .enumerate()
    {
        g.pointer(650., 200. + index as f64, 960., 540., false)
            .unwrap();
        g.key("ArrowLeft", index % 2 == 0);
        g.key("Space", index % 3 == 0);
        g.begin_redraw(now, [960, 540], 1.).unwrap();
        commands.extend(effects(&mut g));
        if let Some(p) = g.take_frame() {
            let id = p.frame.identity;
            // Both issued and returned-for-retry pictures fence ingress.
            g.receive(g.generation(), &tape::bundle(&[tape::food()]), now + 0.1)
                .unwrap();
            g.retry_frame(p).unwrap();
            let p = g.take_frame().unwrap();
            assert_eq!(p.frame.identity, id);
            pictures.push(p);
            g.presented(true).unwrap();
            assert!(g.presented(true).is_err());
            g.service_ingress().unwrap();
            commands.extend(effects(&mut g));
        }
    }
    // Exercise retained replacement by resize, then Setup supersession.
    g.begin_redraw(240., [960, 540], 1.).unwrap();
    commands.extend(effects(&mut g));
    g.viewport([800, 450], 1.).unwrap();
    if let Some(p) = g.take_frame() {
        pictures.push(p);
        g.presented(true).unwrap();
    }
    g.receive(
        g.generation(),
        &tape::bundle(&[tape::setup(), tape::setup()]),
        250.,
    )
    .unwrap();
    g.begin_redraw(260., [800, 450], 1.).unwrap();
    commands.extend(effects(&mut g));
    if let Some(p) = g.take_frame() {
        pictures.push(p);
        g.presented(true).unwrap();
    }
    g.stop();
    commands.extend(effects(&mut g));
    (commands, pictures)
}
fn same_picture(a: &PreparedFrame, b: &PreparedFrame) {
    assert_eq!(a.frame, b.frame);
    assert_eq!(a.overlay, b.overlay);
    assert_eq!(a.resources.len(), b.resources.len());
    for (a, b) in a.resources.iter().zip(&b.resources) {
        assert_eq!(a.asset, b.asset);
        assert_eq!(a.image.pixels, b.image.pixels);
    }
}
#[test]
fn trace_preserves_commands_complete_pictures_deferral_and_retained_identity() {
    let off = schedule();
    let recorder = latency::Recorder::new(8192);
    let guard = recorder.attach();
    let on = schedule();
    drop(guard);
    assert_eq!(off.0, on.0);
    assert_eq!(off.1.len(), on.1.len());
    for (a, b) in off.1.iter().zip(&on.1) {
        same_picture(a, b);
    }
    let (rows, dropped) = recorder.snapshot();
    assert_eq!(dropped, 0);
    for r in rows.iter().filter(|r| r.stage == "deferred") {
        assert_eq!(
            rows.iter()
                .filter(|a| a.stage == "message" && a.id == r.id)
                .count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|a| a.stage == "apply_end" && a.id == r.id)
                .count(),
            1
        );
    }
    for r in rows.iter().filter(|r| r.stage == "retried") {
        assert_eq!(
            rows.iter()
                .filter(|a| a.stage == "issued" && a.id == r.id)
                .count(),
            2
        );
        assert_eq!(
            rows.iter()
                .filter(|a| a.stage == "presented" && a.id == r.id)
                .count(),
            1
        );
    }
    assert!(rows.iter().any(|r| r.stage == "superseded"));
}

#[test]
fn serialized_cpu_sidecar_joins_in_node_and_native_analysis_rejects_missing_reads() {
    let recorder = latency::Recorder::new(8192);
    {
        let _guard = recorder.attach();
        schedule();
    }
    let path = std::env::temp_dir().join(format!(
        "slither-latency-{}-{}.jsonl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    recorder.write(&path, "2f34a000", true).unwrap();
    let analyzer =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/latency-trace.mjs");
    let result = std::process::Command::new("node")
        .arg(&analyzer)
        .arg("--cpu")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}; retained {}",
        String::from_utf8_lossy(&result.stderr),
        path.display()
    );
    let result = std::process::Command::new("node")
        .arg(&analyzer)
        .arg(&path)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("no read span"));
    std::fs::remove_file(path).unwrap();
}
