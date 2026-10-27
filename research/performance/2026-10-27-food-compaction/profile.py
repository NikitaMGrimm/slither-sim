"""Prepare an offline diagnostic overlay; never changes production sources."""
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
p.add_argument('--exact', action='store_true')
a = p.parse_args()
root = a.root.resolve()
s = root / 'source'
s.mkdir(parents=True, exist_ok=False)
archive = root / 'source.tar'
subprocess.run(['git', 'archive', '-o', str(archive), a.ref], check=True)
with tarfile.open(archive) as f:
    f.extractall(s, filter='data')

counter = Path(__file__).with_name('counter.rs').read_text()
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
physical = ('            compact_food(&mut self.food,'
            if '            compact_food(&mut self.food,' in q.read_text()
            else '            let mut i = 0;\n            self.food.retain')
for stage, old in [
    (0, '        // Without food there is nothing to index or eat.'),
    (1, '        // Index the few eater circles,'),
    (2, '        // Eating changes only mass,'),
    (3, '        for &(findex, i) in hits.iter() {'),
    (4, '        if !hits.is_empty() {'),
    (5, '            self.food_cache.remove('),
    (6, physical),
    (7, '        self.advance_prey(previous, prey_buffers);'),
    (8, '        let dt = self.config.dt;\n        let target = if self.config.dynamic_radius'),
    (9, '        self.food_cache.end();\n        if PROFILE {'),
]:
    replace(q, old, f'        crate::core_profile::mark({stage});\n' + old)
replace(q, '    hits.sort_unstable_by_key(|hit| std::cmp::Reverse(hit.0));', '    let _sort = crate::core_profile::scope(10);\n    hits.sort_unstable_by_key(|hit| std::cmp::Reverse(hit.0));')
if a.exact:
    replace(q, '    let mut eater = None;\n    let mut best', '    drop(crate::core_profile::scope(14));\n    let _exact = crate::core_profile::scope(8);\n    let mut eater = None;\n    let mut best')

q = s / 'src/food_grid.rs'
for stage, old in [
    (0, '    pub(crate) fn push(&mut self, pellet: &Food) {'),
    (1, '    fn push(&mut self, entry: Entry, food: usize) {'),
    (2, '    pub(crate) fn remove(&mut self, food_len: usize, removed: impl Iterator<Item = usize>) {'),
    (4, '    fn remove(&mut self, food: usize) {'),
    (5, '    pub(crate) fn has_room(&mut self, food: &[Food], pellet: &Food, size: f64, max: usize) -> bool {'),
    (7, '    pub(crate) fn pellets(&mut self, food: &[Food]) -> Option<&PelletIndex> {'),
    (12, '    pub(crate) fn ambient(&mut self, food: &[Food]) -> usize {'),
]:
    replace(q, old, old + f'\n        let _scope = crate::core_profile::scope({stage});')
replace(q, '        let same = self.entries.len() == food.len()', '        let _sync = crate::core_profile::scope(6);\n        let same = self.entries.len() == food.len()')
replace(q, '            for (i, &slot) in index.food_slots.iter().enumerate() {', '            let _refresh = crate::core_profile::scope(3);\n            for (i, &slot) in index.food_slots.iter().enumerate() {')
replace(q, '    ) -> bool {\n        for &i in &self.touched {', '    ) -> bool {\n        let _grid = crate::core_profile::scope(13);\n        for &i in &self.touched {')
changed = ['src/lib.rs', 'src/simulator.rs', 'src/food_grid.rs', 'src/core_profile.rs', 'src/client/simulation.rs', 'src/client/simulation/study_protocols.rs', 'src/client/simulation/study_protocols/tick_counter.rs', 'examples/study_protocols.rs']
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
(root / 'source-pins.json').write_text(json.dumps({'ref': a.ref, 'archiveSha256': sha(archive), 'overlay': {name: sha(s / name) for name in changed}}, indent=2) + '\n')
print(root, flush=True)
