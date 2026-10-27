"""Apply #178's harness unchanged, then add buffered per-tick diagnostics."""
from pathlib import Path
import subprocess


def instrument(source, original, harness):
    source, original, harness = map(Path, (source, original, harness))
    (source/'examples/host_capacity_load.rs').write_bytes((original/'host_capacity_load.rs').read_bytes())
    (source/'scripts').mkdir(exist_ok=True)
    (source/'scripts/run_trial.py').write_bytes((original/'run_trial.py').read_bytes())
    subprocess.run(['git','apply',str(harness.resolve())],cwd=source,check=True)
    path = source/'src/client/server.rs'
    text = path.read_text(encoding='utf-8')
    start = text.index('impl Simulation {\n    fn run(&mut self) -> Result<(), String> {')
    end = text.index('\n    fn due_ticks(',start)
    body = text[start:end]
    body = body.replace('        let dt = self.shared.clock.dt;', '''        let dt = self.shared.clock.dt;
        // Study-only: no I/O in the measured tick, same instrumentation in each arm.
        let study_origin = Instant::now();
        let study_unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
        let mut study_ticks: Vec<(u64, u64, u64)> = Vec::with_capacity(65536);
        let mut study_previous = None;''',1)
    body = body.replace('            for _ in 0..due {', '''            for _ in 0..due {
                let study_tick = Instant::now();
                let study_at = study_tick.duration_since(study_origin).as_nanos() as u64;
                let study_gap = study_previous.map_or(0, |p: Instant| study_tick.duration_since(p).as_nanos() as u64);
                study_previous = Some(study_tick);''',1)
    body = body.replace('                self.hand_over()?;', '''                self.hand_over()?;
                study_ticks.push((study_at, study_tick.elapsed().as_nanos() as u64, study_gap));''',1)
    body = body.replace('        Ok(())', '''        let file = std::fs::File::create(std::env::var("STUDY_TICKS_JSON").unwrap()).map_err(|e| e.to_string())?;
        let out = std::io::BufWriter::new(file);
        serde_json::to_writer(out, &(study_unix, study_ticks)).map_err(|e| e.to_string())?;
        Ok(())''',1)
    assert body.count('study_ticks.push')==1 and body.count('STUDY_TICKS_JSON')==1
    path.write_text(text[:start]+body+text[end:],encoding='utf-8',newline='\n')
    worker = source/'scripts/run_trial.py'
    text = worker.read_text(encoding='utf-8').replace(
        "STUDY_HOLD=str(args.hold), STUDY_HOST_JSON=", "STUDY_HOLD=str(args.hold), STUDY_TICKS_JSON=str((out/'ticks.json').resolve()), STUDY_HOST_JSON=")
    worker.write_text(text,encoding='utf-8',newline='\n')


if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('source',type=Path)
    a=p.parse_args()
    repo=Path(__file__).resolve().parents[3]
    instrument(a.source,repo/'research/performance/2026-10-26-host-capacity',
               repo/'research/performance/2026-10-26-epyc-host/harness.patch')
