"""Prepare a source-only core-phase and optional bounds-index diagnostic."""
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
p.add_argument('--nested', action='store_true')
a = p.parse_args()
root = a.root.resolve()
s = root / 'source'
s.mkdir(parents=True, exist_ok=False)
archive = root / 'source.tar'
subprocess.run(['git', 'archive', '-o', str(archive), a.ref], check=True)
with tarfile.open(archive) as f:
    f.extractall(s, filter='data')
counter = subprocess.check_output(['git', 'show', '40ce9fac41db8b3043b827056f21ebefe27a706a:research/performance/2026-10-27-food-compaction/counter.rs'], text=True)
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
        raise SystemExit('Ambiguous profile hook: ' + old)
    q.write_text(v.replace(old, new))

q = s / 'src/simulator.rs'
for stage, old in [
    (0, '        let mut clock = if PROFILE { Some(Instant::now()) } else { None };'),
    (1, '        self.food_cache.begin_mirrored(&self.food);'),
    (2, '        // Stable-slot indexing avoids'),
    (3, '        // The offline Android client resolves collisions'),
    (4, '        candidates.clear();\n        dead.clear();'),
    (5, '        if PROFILE {\n            profile.collision_seconds'),
    (6, '        // Without food there is nothing to index or eat.'),
    (7, '        self.advance_prey(previous, prey_buffers);'),
    (9, '        self.food_cache.end();\n        if PROFILE {'),
]:
    replace(q, old, f'        crate::core_profile::mark({stage});\n' + old)
replace(q, '        self.advance_prey(previous, prey_buffers);', '        self.advance_prey(previous, prey_buffers);\n        crate::core_profile::mark(8);')
replace(q, '        self.buffers = buffers;', '        self.buffers = buffers;\n        crate::core_profile::mark(10);')
changed = ['src/lib.rs', 'src/simulator.rs', 'src/core_profile.rs', 'src/client/simulation.rs', 'src/client/simulation/study_protocols.rs', 'src/client/simulation/study_protocols/tick_counter.rs', 'examples/study_protocols.rs']
if a.nested:
    q = s / 'src/bounds_index.rs'
    for stage, old in [
        (0, '    pub(crate) fn new(bounds: &[Bounds]) -> Self {'),
        (1, '    pub(crate) fn candidates(&self, query: Bounds, reach: f64, out: &mut Vec<u32>) -> bool {'),
    ]:
        replace(q, old, old + f'\n        let _scope = crate::core_profile::scope({stage});')
    changed.append('src/bounds_index.rs')
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
(root / 'source-pins.json').write_text(json.dumps({'ref': a.ref, 'nested': a.nested, 'archiveSha256': sha(archive), 'overlay': {name: sha(s / name) for name in changed}}, indent=2) + '\n')
print(root, flush=True)
