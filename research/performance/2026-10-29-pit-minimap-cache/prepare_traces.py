"""Trace-only snapshot overlay. No counters, timers or performance claims.

Usage: python prepare_traces.py REPO OUTPUT ARM REF
Run outside the checkout; archive/copy source and build with one Cargo job.
Reuse the earlier host fixture, adding only two Battledome trace cases.
"""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile

repo, root = map(lambda p: Path(p).resolve(), sys.argv[1:3])
arm, ref = sys.argv[3:]
source = root / arm / 'source'
source.mkdir(parents=True)
archive = root / f'{arm}.tar'
subprocess.run(['git', 'archive', ref, '-o', str(archive)], cwd=repo, check=True)
with tarfile.open(archive) as stream:
    stream.extractall(source, filter='data')
fixture = (repo / 'research/performance/2026-10-27-host-snake-candidates/protocol_fixture.rs').read_text()


def replace(old, new):
    global fixture
    assert fixture.count(old) == 1, old
    fixture = fixture.replace(old, new)


replace('        "official500-legacy" => (500, 500),', '''        "official500-legacy" => (500, 500),
        "official2281" => (2281, 0),
        "battledome8" => (8, 0),
        "battledome8-legacy" => (8, 8),''')
replace('    let config = SimulationConfig {', '''    let mut world = crate::Config::official(count);
    if case.starts_with("battledome") {
        world = crate::Config::battledome();
        world.radius_updates = vec![
            crate::RadiusUpdate { time: 0.014, radius: 7168. },
            crate::RadiusUpdate { time: 0.532, radius: 6000. },
            crate::RadiusUpdate { time: 3.01, radius: 3584. },
        ];
    }
    let config = SimulationConfig {''')
replace('        world: crate::Config::official(count),', '        world,')
# The old fixture's Linux PMU is intentionally absent from this portable parity
# overlay. Methods remain type-compatible; no measurement environment is read.
folder = source / 'src/client/simulation/study_protocols'
folder.mkdir()
(folder / 'tick_counter.rs').write_text('''pub(super) struct Counter;
impl Counter {
 pub(super) fn from_env()->Option<Self>{None}
 pub(super) fn read(&self)->[u64;3]{unreachable!("trace-only counter")}
 pub(super) fn record(&mut self,_:usize,_:[u64;3],_:[u64;3],_:usize){unreachable!("trace-only counter")}
}
''', newline='\n')
(source / 'src/client/simulation/study_protocols.rs').write_text(fixture, newline='\n')
p = source / 'src/client/simulation.rs'
p.write_text(p.read_text() + '\npub mod study_protocols;\n', newline='\n')
(source / 'examples/study_protocols.rs').write_text('''fn main(){
let a:Vec<_>=std::env::args().collect();
slither_sim::client::simulation::study_protocols::run(&a[1],true);}
''', newline='\n')
if arm == 'mutation':
    p = source / 'src/client/minimap.rs'
    text = p.read_text()
    old = 'self.apply(&mut map, pit / world.radius() * side as f64 / 2.);'
    assert text.count(old) == 1
    p.write_text(text.replace(old, 'self.apply(&mut map, pit / world.radius() * side as f64 / 2. * 0.98);'), newline='\n')
env = dict(os.environ, CARGO_BUILD_JOBS='1', RUST_TEST_THREADS='1',
           CARGO_TARGET_DIR=os.environ.get('CARGO_TARGET_DIR', str(root / 'target')), RAYON_NUM_THREADS='1')
command = ['cargo', 'build', '--locked', '--offline', '-j', '1',
           '--example', 'study_protocols', '--example', 'perf_baseline']
with (root / arm / 'build.log').open('x') as log:
    subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
bins = root / arm / 'bin'
bins.mkdir()
suffix = '.exe' if os.name == 'nt' else ''
for binary in ['study_protocols', 'perf_baseline']:
    shutil.copy2(Path(env['CARGO_TARGET_DIR']) / 'debug/examples' / (binary + suffix), bins / (binary + suffix))


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


(root / arm / 'pins.json').write_text(json.dumps({
    'traceOnly': True, 'ref': ref, 'commit': subprocess.check_output(
        ['git', 'rev-parse', ref], cwd=repo, text=True).strip(),
    'archiveSha256': sha(archive), 'command': command,
    'driverSha256': sha(Path(__file__)),
    'sources': {str(p.relative_to(source)): sha(p) for p in source.rglob('*.rs')},
    'binaries': {p.name: sha(p) for p in bins.iterdir()},
}, indent=2) + '\n', newline='\n')
print('BUILT trace-only', arm, flush=True)
