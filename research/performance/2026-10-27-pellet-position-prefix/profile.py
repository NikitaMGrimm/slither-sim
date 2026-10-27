"""Prepare a source-only diagnostic overlay for slot-position maintenance."""
import argparse
import hashlib
import json
import shutil
import subprocess
import tarfile
from pathlib import Path

if not __debug__:
    raise SystemExit('Run without Python optimization')
p = argparse.ArgumentParser()
p.add_argument('ref')
p.add_argument('root', type=Path)
a = p.parse_args()
root = a.root.resolve()
s = root / 'source'
s.mkdir(parents=True, exist_ok=False)
archive = root / 'source.tar'
subprocess.run(['git', 'archive', '-o', str(archive), a.ref], check=True)
with tarfile.open(archive) as f:
    f.extractall(s, filter='data')

counter = subprocess.check_output(['git', 'show', '40ce9fac41db8b3043b827056f21ebefe27a706a:research/performance/2026-10-27-food-compaction/counter.rs'], text=True)
counter = counter.replace('calls:[u64;16],rows:', 'calls:[u64;16],metrics:[u64;4],rows:')
counter = counter.replace('calls:[0;16],rows:', 'calls:[0;16],metrics:[0;4],rows:')
counter = counter.replace('c.calls=[0;16];', 'c.calls=[0;16];c.metrics=[0;4];')
counter = counter.replace('"calls":c.calls', '"calls":c.calls,"metrics":c.metrics')
counter += '\npub fn metric(stage:usize,value:usize){C.with_borrow_mut(|c|c.metrics[stage]+=value as u64);}\n'
(s / 'src/core_profile.rs').write_text(counter)
for name, extra in [('src/lib.rs', '\npub mod core_profile;\n'), ('src/client/simulation.rs', '\npub mod study_protocols;\n')]:
    q = s / name
    q.write_text(q.read_text() + extra)
fixture = s / 'research/performance/2026-10-27-host-snake-candidates'
v = (fixture / 'protocol_fixture.rs').read_text()
v = v.replace('        sim.advance_tick_with(0,', '        crate::core_profile::begin(tick);\n        sim.advance_tick_with(0,')
v = v.replace('        population_sum += population;', '        crate::core_profile::end(population);\n        population_sum += population;')
(s / 'src/client/simulation/study_protocols.rs').write_text(v)
child = s / 'src/client/simulation/study_protocols'
child.mkdir()
shutil.copy2(fixture / 'tick_counter.rs', child / 'tick_counter.rs')
(s / 'examples/study_protocols.rs').write_text('fn main(){slither_sim::client::simulation::study_protocols::run("official500",false);}\n')

def replace(q, old, new):
    v = q.read_text()
    if v.count(old) != 1:
        raise SystemExit('Ambiguous hook: ' + old)
    q.write_text(v.replace(old, new))

q = s / 'src/simulator.rs'
replace(q, '        let mut clock = if PROFILE { Some(Instant::now()) } else { None };', '        let _physics = crate::core_profile::scope(2);\n        let mut clock = if PROFILE { Some(Instant::now()) } else { None };')
replace(q, '    // Unique cells and one handle per pellet give at most one hit per pellet.', '    let _lookup = crate::core_profile::scope(4);\n    // Unique cells and one handle per pellet give at most one hit per pellet.')
q = s / 'src/food_grid.rs'
old = '    pub(crate) fn remove(&mut self, food_len: usize, removed: impl Iterator<Item = usize>) {'
replace(q, old, old + '\n        let _remove = crate::core_profile::scope(0);\n        let mut removed = removed.peekable();\n        crate::core_profile::metric(0, food_len);\n        crate::core_profile::metric(1, removed.peek().copied().unwrap_or(food_len));')
for stage, old in [
    (3, '    fn push(&mut self, entry: Entry, food: usize) {'),
    (5, '    pub(crate) fn pellets(&mut self, food: &[Food]) -> Option<&PelletIndex> {'),
]:
    replace(q, old, old + f'\n        let _scope = crate::core_profile::scope({stage});')
replace(q, '        self.entries.truncate(write + self.entries.len() - read);', '        self.entries.truncate(write + self.entries.len() - read);\n        crate::core_profile::metric(2, self.entries.len());')
v = q.read_text()
assignment = 'index.slots[slot as usize].food = i as u32;'
if assignment in v:
    if v.count(assignment) != 1:
        raise SystemExit('Ambiguous refresh loop')
    start = v.rfind('            for ', 0, v.index(assignment))
    if start < 0:
        raise SystemExit('Missing refresh loop')
    v = v[:start] + '            let _refresh = crate::core_profile::scope(1);\n' + v[start:]
    q.write_text(v)
changed = ['src/lib.rs', 'src/simulator.rs', 'src/food_grid.rs', 'src/core_profile.rs', 'src/client/simulation.rs', 'src/client/simulation/study_protocols.rs', 'src/client/simulation/study_protocols/tick_counter.rs', 'examples/study_protocols.rs']
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
(root / 'source-pins.json').write_text(json.dumps({'ref': a.ref, 'archiveSha256': sha(archive), 'overlay': {name: sha(s / name) for name in changed}}, indent=2) + '\n')
print(root, flush=True)
