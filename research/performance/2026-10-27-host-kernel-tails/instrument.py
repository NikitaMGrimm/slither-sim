"""Extend #201's isolated snapshot with optional buffered socket-stage traces."""
import importlib.util
from pathlib import Path


def instrument(source, original, harness):
    source = Path(source)
    previous = Path(__file__).parent.parent / '2026-10-27-epyc-host-recheck/instrument.py'
    spec = importlib.util.spec_from_file_location('previous', previous)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.instrument(source, original, harness)
    path = source / 'src/client/server.rs'
    text = path.read_text(encoding='utf-8')
    start = text.index('impl Gateway {\n    fn run(')
    end = text.index('\n    fn retain_sessions(', start)
    body = text[start:end]
    body = body.replace('        while !self.gate.shared.stopped() {', '''        study_stage(0, self.gate.shared.clock.wall(Instant::now()), 0, 0);
        while !self.gate.shared.stopped() {''', 1)
    body = body.replace('        Ok(())\n    }', '''        if let Ok(path) = std::env::var("STUDY_STAGES_JSON") {
            STUDY_STAGES.with(|rows| {
                let out = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
                serde_json::to_writer(out, &*rows.borrow()).unwrap();
            });
        }
        Ok(())
    }''', 1)
    body = body.replace('        let now = Instant::now();\n        self.accept(now);', '''        let now = Instant::now();
        study_stage(1, 0., 0, 0);
        self.accept(now);''', 1)
    body = body.replace('        self.retain_sessions();', '        study_stage(2, 0., 0, 0);\n        self.retain_sessions();', 1)
    text = text[:start] + body + text[end:]
    text = text.replace('                // Socket slots share route 0.', '''                if slot == 0 {
                    for (due, frame) in &frames {
                        study_stage(8, *due + offset, frame.len() as u64, study_families(frame));
                    }
                }
                // Socket slots share route 0.''', 1)
    text = text.replace('        let file = std::fs::File::create(std::env::var("STUDY_TICKS_JSON")', '''        if let Ok(path) = std::env::var("STUDY_STAGES_JSON") {
            STUDY_STAGES.with(|rows| {
                let out = std::io::BufWriter::new(std::fs::File::create(path + ".simulation").unwrap());
                serde_json::to_writer(out, &*rows.borrow()).unwrap();
            });
        }
        let file = std::fs::File::create(std::env::var("STUDY_TICKS_JSON")''', 1)
    text = text.replace('            Delivery::Frames { frames, .. } => {', '''            Delivery::Frames { frames, .. } => {
                if slot == 0 {
                    for (due, frame) in &frames {
                        study_stage(3, *due, frame.len() as u64, study_families(frame));
                    }
                }''', 1)
    text = text.replace('            if let Err(reason) = self.send(message) {', '''            if self.phase == Phase::Playing(0) {
                study_stage(4, study_due, message.len() as u64, study_families(&message));
            }
            if let Err(reason) = self.send(message) {''', 1)
    text = text.replace('            let (_, message) = self.queued.pop_front().unwrap();',
                        '            let (study_due, message) = self.queued.pop_front().unwrap();', 1)
    text += '''

// Study-only; disabled qualification arms retain no stage records.
type StudyStage = (u8, f64, f64, u64, u64);
thread_local! {
    static STUDY_STAGES: std::cell::RefCell<Vec<StudyStage>> = const { std::cell::RefCell::new(Vec::new()) };
}
fn study_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("STUDY_STAGES_JSON").is_some())
}
fn study_stage(kind: u8, due: f64, bytes: u64, flags: u64) {
    if !study_enabled() { return; }
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
    STUDY_STAGES.with(|rows| rows.borrow_mut().push((kind, unix, due, bytes, flags)));
}
fn study_families(frame: &[u8]) -> u64 {
    if !study_enabled() { return 0; }
    protocol::split_frame(frame).unwrap().iter().fold(0, |bits, p| bits | match p[0] {
        b'u' | b'U' | b'M' | b'V' => 1,
        b'l' => 2,
        _ => 0,
    })
}
'''
    assert text.count('study_stage(1,') == 1
    assert text.count('study_stage(2,') == 1
    assert text.count('study_stage(3,') == 1
    assert text.count('study_stage(4,') == 1
    path.write_text(text, encoding='utf-8', newline='\n')
