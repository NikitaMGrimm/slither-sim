"""Identical additive diagnostic probes; never applied to production sources."""
from pathlib import Path
import inspect
if not __debug__: raise SystemExit('run without -O')
def replace(p,a,b):
 s=p.read_text();
 if s.count(a)!=1:raise RuntimeError(f'anchor mismatch {p.name}: {a!r}')
 p.write_text(s.replace(a,b),newline='\n')
def instrument(source):
 source=Path(source);p=source/'src/client/simulation/remote.rs'
 replace(p,'impl RawSimulator {','''impl RawSimulator {
    pub(crate) fn study_deaths(&self) -> usize { self.backend.world.deaths().len() }
    pub(crate) fn study_world(&self) -> serde_json::Value {
        let w = &self.backend.world;
        let live: Vec<_> = w.snakes().iter().filter(|s| s.alive).collect();
        serde_json::json!({"alive": live.len(), "segments": live.iter().map(|s| s.segments).sum::<usize>(),
            "anchors": live.iter().map(|s| s.body.len()).sum::<usize>(),
            "mass": live.iter().map(|s| s.length).sum::<f64>(), "food": w.food().len(),
            "radius": w.radius(), "births": self.backend.slots.iter().map(|s| s.life).sum::<u64>()})
    }
''')
 p=source/'src/client/server.rs'
 replace(p,'        let mut study_previous = None;','''        let mut study_previous = None;
        let mut study_world = Vec::with_capacity(256);
        let mut study_next = 0.;
        let mut study_deaths = 0usize;''')
 replace(p,'                study_ticks.push((study_at, study_tick.elapsed().as_nanos() as u64, study_gap));','''                study_ticks.push((study_at, study_tick.elapsed().as_nanos() as u64, study_gap));
                study_deaths += self.world.study_deaths();
                let study_elapsed = study_origin.elapsed().as_secs_f64();
                if study_elapsed >= study_next {
                    let mut row = self.world.study_world();
                    let scheduled = (Instant::now().duration_since(self.shared.clock.epoch).as_secs_f64() / dt) as u64;
                    row["unix"] = (study_unix + study_elapsed).into();
                    row["ticks"] = self.ticks.into(); row["deferred"] = self.deferred.into();
                    row["backlog"] = scheduled.saturating_sub(self.ticks + self.deferred).into();
                    row["deaths"] = study_deaths.into();
                    study_world.push(row); study_next = study_elapsed + 1.;
                }''')
 replace(p,'        let file = std::fs::File::create(std::env::var("STUDY_TICKS_JSON").unwrap())','''        std::fs::write(std::env::var("STUDY_WORLD_JSON").unwrap(), serde_json::to_vec(&study_world).unwrap()).map_err(|e| e.to_string())?;
        let file = std::fs::File::create(std::env::var("STUDY_TICKS_JSON").unwrap())''')
 p=source/'src/main.rs'
 replace(p,'if boundary < 2 && (now >= next || now >= if boundary == 0 { start } else { end }) {','if now >= next || (boundary < 2 && now >= if boundary == 0 { start } else { end }) {')
 replace(p,'if now >= end + 2. {','if now >= end + 32. {')
 p=source/'scripts/run_trial.py'
 replace(p,"env['STUDY_INPUT_PREFIX']=str((out/'input').resolve())","env['STUDY_INPUT_PREFIX']=str((out/'input').resolve())\n            env['STUDY_WORLD_JSON']=str((out/'world.json').resolve())")
 # Retain primary window clipping in ctx.in_window and weighted occupancy.
 p=source/'examples/host_capacity_load.rs'
 replace(p,'            if now >= end {','            if now >= end + 30. {')
 replace(p,'            if now >= start {','            if now >= start && now < end {')

def inventory():
    result={}
    for p in Path('/proc').glob('[0-9]*'):
        try:
            s=(p/'stat').read_text().rsplit(')',1)[1].split()
            result[p.name]={'comm':(p/'comm').read_text().strip(),'startTicks':int(s[19]),
                'userSeconds':int(s[11])/HZ,'systemSeconds':int(s[12])/HZ,
                'lastCpu':int(s[36]),'rssPages':int(s[21])}
        except (FileNotFoundError,ProcessLookupError,PermissionError):pass
    return result

def worker_overlay(original,dest,prior):
 prior.worker_overlay(original,dest)
 p=Path(dest)
 replace(p,'def snapshot(pids):',inspect.getsource(inventory)+'\n\ndef snapshot(pids):')
 replace(p,"'diagnostic': clock_sample(),","'diagnostic': clock_sample(),\n            'allProcesses': inventory() if COLLECT_HOST else None,\n            'allCpuCounters':[s for s in Path('/proc/stat').read_text().splitlines() if s.startswith('cpu')],")
 replace(p,'def main(args):',"def main(args):\n    global COLLECT_HOST\n    COLLECT_HOST = args.role == 'host'")
