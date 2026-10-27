"""Temporary broadcast/encoding and admission profiler; separate from timed pairs."""
from pathlib import Path

p = Path('src/client/simulation/broadcast.rs')
s = p.read_text(encoding='utf-8')
needle = '        self.broadcast_with(early, hand_over)?;'
assert s.count(needle) == 1
s = s.replace(needle, '''        let encode_started = std::time::Instant::now();
        self.broadcast_with(early, hand_over)?;
        ENCODE_TIMES.with(|t| t.borrow_mut().0.push((self.time(), encode_started.elapsed().as_secs_f64())));''')
s += '''
struct EncodeTimes(Vec<(f64, f64)>);
impl Drop for EncodeTimes {
    fn drop(&mut self) {
        if let Ok(root) = std::env::var("SLITHER_TIMING") {
            use std::io::Write;
            let mut file = std::io::BufWriter::new(std::fs::File::create(format!("{root}-encode.csv")).unwrap());
            for (tick, duration) in &self.0 { writeln!(file, "{tick:.9},{duration:.9}").unwrap(); }
        }
    }
}
thread_local! { static ENCODE_TIMES: std::cell::RefCell<EncodeTimes> = const { std::cell::RefCell::new(EncodeTimes(Vec::new())) }; }
'''
p.write_text(s, encoding='utf-8', newline='\n')

p = Path('src/client/server.rs')
s = p.read_text(encoding='utf-8')
assert 'struct TimingTrace' in s
needle = '        self.admit_waiting()?;'
assert s.count(needle) == 1
s = s.replace(needle, '''        let admit_started = Instant::now();
        self.admit_waiting()?;
        ADMISSION.with(|t| t.borrow_mut().rows.push((self.shared.clock.wall(admit_started), self.shared.clock.wall(Instant::now()), 0., 0.)));''')
s += '''
thread_local! { static ADMISSION: std::cell::RefCell<TimingTrace> = std::cell::RefCell::new(TimingTrace::new("admission")); }
'''
p.write_text(s, encoding='utf-8', newline='\n')
