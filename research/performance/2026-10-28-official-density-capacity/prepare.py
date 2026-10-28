"""Build one bounded target from a pinned archive; overlays affect only copies.

prepare.py ARCHIVE NEW_OUTPUT --target NEW_TARGET --cpu 0
The target must be task-owned. Delete it after copying and hashing binaries.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

HERE = Path(__file__).resolve().parent

def sha(path):
    return hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()

def swap(path, old, new):
    text = path.read_text()
    assert text.count(old) == 1, (path.name, old, text.count(old))
    path.write_text(text.replace(old, new), newline='\n')

def profile(source):
    donor = source/'research/performance/2026-10-27-population-scaling/counter.rs'
    counter = donor.read_text().replace('c.active=active;', 'c.active=active && enabled();') + '''
pub fn enabled() -> bool {
    std::env::var_os("CAPACITY_PROFILE").is_some_and(|path| std::path::Path::new(&path).exists())
}
pub struct Guard(usize, Option<[u64;3]>);
impl Guard {pub fn new(stage:usize)->Self {Self(stage,start())}}
impl Drop for Guard {fn drop(&mut self) {add(self.0,self.1);}}
'''
    (source/'src/study_counter.rs').write_text(counter)
    with (source/'src/lib.rs').open('a') as f:
        f.write('\nmod study_counter;\n')
    path = source/'src/simulator.rs'
    swap(path, '        let mut clock = if PROFILE', '        crate::study_counter::begin();\n        let mut clock = if PROFILE')
    anchors = [(0,'        // Stable-slot indexing'),
               (1,'        candidates.clear();\n        dead.clear();'),
               (2,'        self.time += c.dt;'),
               (3,'        // Without food there is nothing'),
               (4,'        self.advance_prey(previous, heads, eaters, prey_buffers);'),
               (5,'        let dt = self.config.dt;\n        let target'),
               (6,'        for (snake, kiosk) in self.snakes.iter_mut().zip(&self.kiosks)')]
    for stage, anchor in anchors:
        swap(path,anchor,f'        crate::study_counter::mark({stage});\n'+anchor)
    # RAII scopes include every return on admission and broadcast paths.
    for name, anchor, stage in [
        ('src/client/simulation.rs','    fn spawn_pose_waited(',7),
        ('src/client/simulation/broadcast.rs','    fn broadcast_with',8)]:
        path=source/name
        text=path.read_text()
        start=text.index(anchor)
        brace=text.index('{',start)
        path.write_text(text[:brace+1]+f'\n        let _phase = crate::study_counter::Guard::new({stage});'+text[brace+1:])
    path=source/'src/client/server.rs'
    swap(path,'        let dt = self.shared.clock.dt;\n        while !self.shared.stopped()',
         '        crate::study_counter::reset(true);\n        let dt = self.shared.clock.dt;\n        while !self.shared.stopped()')
    swap(path,'            for _ in 0..due {','            for _ in 0..due {\n                let study_pmu = crate::study_counter::start();')
    swap(path,'                self.hand_over()?;', '''                self.hand_over()?;
                crate::study_counter::add(9, study_pmu);
                if self.ticks % 100 == 0 {
                    eprintln!("SIM_PROFILE {}", crate::study_counter::result());
                    crate::study_counter::reset(true);
                }''')
    swap(path,'        let start = self.shared.clock.wall(Instant::now());',
         '        crate::study_counter::reset(true);\n        let study_started = Instant::now();\n        let start = self.shared.clock.wall(Instant::now());')
    swap(path,'\n            self.shared.clock.sleep_until(next, MAX_IDLE);', '''
            if study_started.elapsed().as_secs() > 0 {
                thread_local! {static LAST: std::cell::Cell<u64> = const {std::cell::Cell::new(0)};}
                let second = study_started.elapsed().as_secs();
                if LAST.with(|last| last.replace(second)) != second {
                    eprintln!("BOT_PROFILE {}", crate::study_counter::result());
                    crate::study_counter::reset(true);
                }
            }
            self.shared.clock.sleep_until(next, MAX_IDLE);''')
    swap(path,'                if self.state.receive_frame(&bytes, at).is_err() {',
         '                let study_pmu = crate::study_counter::start();\n                let study_result = self.state.receive_frame(&bytes, at);\n                crate::study_counter::add(10, study_pmu);\n                if study_result.is_err() {')
    swap(path,'        let frame = self.state.frame(at)?;',
         '        let study_pmu = crate::study_counter::start();\n        let frame = self.state.frame(at)?;\n        crate::study_counter::add(11, study_pmu);\n        let study_pmu = crate::study_counter::start();')
    swap(path,'        if let Some(action) = policy.decide(&frame) {',
         '        let study_action = policy.decide(&frame);\n        crate::study_counter::add(12, study_pmu);\n        if let Some(action) = study_action {')

def main():
    if not __debug__:raise SystemExit('Run without Python optimization')
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('archive',type=Path);p.add_argument('output',type=Path)
    p.add_argument('--target',required=True,type=Path);p.add_argument('--cpu',type=int,default=0)
    a=p.parse_args();os.sched_setaffinity(0,{a.cpu})
    assert not a.target.exists(), 'target must be a fresh task-owned directory'
    root=a.output.resolve();root.mkdir(exist_ok=False)
    source=root/'source';source.mkdir()
    with tarfile.open(a.archive) as tar:
        tar.extractall(source,filter='data')
        revision=tar.pax_headers.get('comment')
    originals={str(path.relative_to(source)):sha(path) for path in (source/'src').rglob('*.rs')}
    env=dict(os.environ,CARGO_BUILD_JOBS='1',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',
             CARGO_TARGET_DIR=str(a.target.resolve()),CARGO_PROFILE_RELEASE_DEBUG='1',
             RUSTFLAGS='-C target-cpu=native',CARGO_ENCODED_RUSTFLAGS='')
    env.pop('CARGO_ENCODED_RUSTFLAGS')
    for key,tool in [('RUSTC','rustc'),('RUSTDOC','rustdoc')]:
        env[key]=subprocess.check_output(['rustup','which',tool],text=True).strip()
    pins={'sourceCommit':revision,'archiveSha256':sha(a.archive),'buildCommand':'cargo build --release --locked --offline -j 1 --bin slither-sim; portable_bot built once from pristine source',
          'rustflags':env['RUSTFLAGS'],'rustc':subprocess.check_output([env['RUSTC'],'-Vv'],text=True),'arms':{}}
    for arm in ['plain','observe','profile']:
        if arm=='observe':
            shutil.copyfile(HERE/'observe.rs',source/'src/study_observe.rs')
            with (source/'src/lib.rs').open('a') as f:f.write('\nmod study_observe;\n')
            swap(source/'src/client/server.rs','            for _ in 0..due {','            for _ in 0..due {\n                let study_start = Instant::now();')
            swap(source/'src/client/server.rs','                self.hand_over()?;',
                 '                self.hand_over()?;\n                crate::study_observe::tick(study_start, self.ticks, self.deferred, &self.world.backend.world);')
            # Access is read-only and limited to this archived-copy sibling module.
            swap(source/'src/client/simulation.rs','    world: Simulator,','    pub(crate) world: Simulator,')
            swap(source/'src/client/simulation/remote.rs','    pub(super) backend: SimulatorBackend,','    pub(crate) backend: SimulatorBackend,')
        if arm=='profile':profile(source)
        with (root/f'build-{arm}.log').open('w') as log:
            command=['cargo','build','--release','--locked','--offline','-j','1','--bin','slither-sim']
            if arm=='plain':command+=['--example','portable_bot']
            subprocess.run(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
        dest=root/'bin'/arm;dest.mkdir(parents=True)
        for name,rel in [('slither-sim','slither-sim'),('portable_bot','examples/portable_bot')]:
            shutil.copyfile(a.target/'release'/rel,dest/name);(dest/name).chmod(0o755)
        pins['arms'][arm]={'binarySha256':sha(dest/'slither-sim'),'clientSha256':sha(dest/'portable_bot'),
            'overlays':{str(path.relative_to(source)):sha(path) for path in (source/'src').rglob('*.rs') if sha(path)!=originals.get(str(path.relative_to(source)))}}
        (root/'pins.json').write_text(json.dumps(pins,indent=2)+'\n')
        print('BUILT',arm,flush=True)
    assert a.target.resolve().name=='official-density-target', 'refuse unexpected cleanup target'
    shutil.rmtree(a.target.resolve())
    print('TARGET_REMOVED',flush=True)

if __name__=='__main__':main()
