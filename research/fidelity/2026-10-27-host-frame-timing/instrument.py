"""Apply temporary buffered host instrumentation; keep out of production."""
from pathlib import Path

p = Path('src/client/server.rs')
s = p.read_text(encoding='utf-8')
needle = 'impl Simulation {\n    fn run(&mut self) -> Result<(), String> {'
s = s.replace(needle, '''struct TimingTrace {
    name: &'static str,
    rows: Vec<(f64, f64, f64, f64)>,
}
impl TimingTrace {
    fn new(name: &'static str) -> Self { Self { name, rows: Vec::new() } }
}
impl Drop for TimingTrace {
    fn drop(&mut self) {
        if let Ok(root) = std::env::var("SLITHER_TIMING") {
            use std::io::Write;
            let mut file = std::io::BufWriter::new(std::fs::File::create(format!("{root}-{}.csv", self.name)).unwrap());
            for (a,b,c,d) in &self.rows { writeln!(file, "{a:.9},{b:.9},{c:.9},{d:.9}").unwrap(); }
        }
    }
}
impl Simulation {
    fn run(&mut self) -> Result<(), String> {
        let mut trace = TimingTrace::new("simulation");''')
s = s.replace('                self.apply_commands()?;\n                self.advance_tick()?;', '''                let wake = self.shared.clock.wall(Instant::now());
                let deadline = (self.ticks + self.deferred + 1) as f64 * dt;
                self.apply_commands()?;
                let advance = self.shared.clock.wall(Instant::now());
                self.advance_tick()?;
                let encoded = self.shared.clock.wall(Instant::now());
                trace.rows.push((deadline, wake, advance, encoded));''')
s = s.replace('impl Gateway {\n    fn run(&mut self) -> Result<(), String> {', '''impl Gateway {
    fn run(&mut self) -> Result<(), String> {
        let mut trace = TimingTrace::new("gateway");''')
s = s.replace('            self.poll()?;\n            let next', '''            let before = self.gate.shared.clock.wall(Instant::now());
            self.poll()?;
            trace.rows.push((before, self.gate.shared.clock.wall(Instant::now()), 0., 0.));
            let next''')
# Thread-local stream records socket release and early-handoff separately.
s = s.replace('struct TimingTrace {', '''thread_local! { static RELEASE: std::cell::RefCell<TimingTrace> = std::cell::RefCell::new(TimingTrace::new("release"));
static EARLY: std::cell::RefCell<TimingTrace> = std::cell::RefCell::new(TimingTrace::new("early")); }
struct TimingTrace {''')
s = s.replace('                batches[0].push(delivery(slot, generations[slot], frames, offset));', '''                EARLY.with(|t| { let at = shared.clock.wall(Instant::now()); for (due, _) in &frames { t.borrow_mut().rows.push((*due + offset, at, slot as f64, 0.)); } });
                batches[0].push(delivery(slot, generations[slot], frames, offset));''')
s = s.replace('            let (_, message) = self.queued.pop_front().unwrap();', '''            let (due, message) = self.queued.pop_front().unwrap();
            RELEASE.with(|t| t.borrow_mut().rows.push((due, gate.shared.clock.wall(Instant::now()), wall, 0.)));''')
p.write_text(s, encoding='utf-8', newline='\n')
