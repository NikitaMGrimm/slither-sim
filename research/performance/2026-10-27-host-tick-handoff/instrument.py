"""Apply #201 counters and buffered sampled-input tracing to isolated sources."""
import importlib.util
from pathlib import Path


def instrument(source, original, harness):
    source = Path(source)
    server = source/'src/client/server.rs'
    text = server.read_text(encoding='utf-8').replace(
        '        let dt = shared.clock.dt;', '        let dt = self.shared.clock.dt;', 1)
    server.write_text(text, encoding='utf-8', newline='\n')
    prior = Path(__file__).parent.parent/'2026-10-27-epyc-host-recheck/instrument.py'
    spec = importlib.util.spec_from_file_location('previous', prior)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.instrument(source, original, harness)
    text = server.read_text(encoding='utf-8')
    assert 'let study_origin = Instant::now();' in text
    text += '\npub(crate) mod study_input;\n'
    text = text.replace('        let study_origin = Instant::now();', '''        study_input::origin(self.shared.clock.wall(Instant::now()));
        let study_origin = Instant::now();''', 1)
    text = text.replace('    fn forward(&self, command: Command) -> Result<(), String> {', '''    fn forward(&self, command: Command) -> Result<(), String> {
        if let Command::Message { slot, generation, bytes, .. } = &command {
            study_input::received(*slot, *generation, bytes);
        }''', 1)
    text = text.replace('        gate.send(Command::Connect {', '''        study_input::register(slot, self.generation, &appearance.nickname);
        gate.send(Command::Connect {''', 1)
    server.write_text(text, encoding='utf-8', newline='\n')
    (source/'src/client/server/study_input.rs').write_text(INPUT, encoding='utf-8', newline='\n')
    path = source/'src/client/simulation/connection.rs'
    text = path.read_text(encoding='utf-8').replace(
        '            let (_, message) = self.slots[slot].connection.uplink.pop_front().unwrap();',
        '''            let (_, message) = self.slots[slot].connection.uplink.pop_front().unwrap();
            if let Uplink::Message(bytes) = &message {
                crate::client::server::study_input::ingest(&self.profiles[slot].nickname, bytes, now);
            }''', 1)
    assert 'study_input::ingest' in text
    path.write_text(text, encoding='utf-8', newline='\n')
    path = source/'examples/host_capacity_load.rs'
    text = path.read_text(encoding='utf-8')
    text = text.replace('        frame_records: Vec<(u64, u64, f64, u64, usize)>,',
                        '        frame_records: Vec<(u64, u64, f64, u64, usize)>,\n        input_records: Vec<(u64,u64,u8,f64)>,', 1)
    text = text.replace('            frame_records: Vec::new(),',
                        '            frame_records: Vec::new(),\n            input_records: Vec::new(),', 1)
    text = text.replace('            self.state.record_sent(bytes, at)?;', '''            if self.id % 64 == 0 && bytes.len() == 1 && bytes[0] <= 250 && ctx.in_window(at) {
                let queued = ctx.now() * 1000.;
                ctx.input_records.push((self.id, self.connections, bytes[0], queued));
            }
            self.state.record_sent(bytes, at)?;''', 1)
    text = text.replace('        frames_out.flush().map_err(|e| e.to_string())?;', '''        frames_out.flush().map_err(|e| e.to_string())?;
        std::fs::write(args.output.with_extension("inputs.json"),
            serde_json::to_vec(&(epoch_unix_ms, &ctx.input_records)).unwrap()).map_err(|e| e.to_string())?;''', 1)
    assert text.count('input_records: Vec') == 2
    path.write_text(text, encoding='utf-8', newline='\n')
    path = source/'scripts/run_trial.py'
    text = path.read_text(encoding='utf-8').replace(
        "            p = launch([str(Path(args.bin).resolve()/'slither-sim')",
        "            env['STUDY_INPUT_PREFIX']=str((out/'input').resolve())\n            p = launch([str(Path(args.bin).resolve()/'slither-sim')", 1)
    text = text.replace('        for reader in readers: reader.join(timeout=5)', '''        # Keep sampled traces outside the capacity reducer's load-*.json census.
        for sample in out.glob('load-*.inputs.json'):
            sample.rename(sample.with_name(sample.name.replace('load-', 'inputs-', 1)))
        for reader in readers: reader.join(timeout=5)''', 1)
    path.write_text(text, encoding='utf-8', newline='\n')


INPUT = r'''
use std::{cell::RefCell, collections::BTreeMap, time::{SystemTime, UNIX_EPOCH}};
type Event = (u8,u64,u8,f64,f64,u64);
struct Trace(Vec<Event>);
impl Drop for Trace {
    fn drop(&mut self) {
        if let Ok(prefix) = std::env::var("STUDY_INPUT_PREFIX") {
            let thread = std::thread::current();
            let name = thread.name().unwrap_or("unknown");
            std::fs::write(format!("{prefix}-{name}.json"), serde_json::to_vec(&self.0).unwrap()).unwrap();
        }
    }
}
thread_local! {
    static DATA: RefCell<Trace> = const { RefCell::new(Trace(Vec::new())) };
    static IDS: RefCell<BTreeMap<usize,(u64,u64)>> = const { RefCell::new(BTreeMap::new()) };
}
fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("STUDY_INPUT_PREFIX").is_some())
}
fn id(nickname: &[u8]) -> Option<u64> {
    let name = std::str::from_utf8(nickname).ok()?;
    let id: u64 = name.strip_prefix('L')?.parse().ok()?;
    (id % 64 == 0).then_some(id)
}
fn event(kind: u8, id: u64, byte: u8, virtual_time: f64, generation: u64) {
    let unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
    DATA.with(|rows| rows.borrow_mut().0.push((kind,id,byte,unix,virtual_time,generation)));
}
pub(crate) fn register(slot: usize, generation: u64, nickname: &[u8]) {
    if !enabled() { return; }
    IDS.with(|ids| {
        let mut ids = ids.borrow_mut();
        ids.remove(&slot);
        if let Some(id) = id(nickname) { ids.insert(slot,(id,generation)); }
    });
}
pub(crate) fn received(slot: usize, generation: u64, bytes: &[u8]) {
    if !enabled() || bytes.len()!=1 || bytes[0]>250 { return; }
    IDS.with(|ids| {
        if let Some(&(id, expected)) = ids.borrow().get(&slot) {
            if generation == expected { event(0,id,bytes[0],0.,generation); }
        }
    });
}
pub(crate) fn ingest(nickname: &[u8], bytes: &[u8], now: f64) {
    if !enabled() || bytes.len()!=1 || bytes[0]>250 { return; }
    if let Some(id) = id(nickname) { event(1,id,bytes[0],now,0); }
}
pub(crate) fn origin(wall: f64) {
    if enabled() { event(2,0,0,wall,0); }
}
'''
