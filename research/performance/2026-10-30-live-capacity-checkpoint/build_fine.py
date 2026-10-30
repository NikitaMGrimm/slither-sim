"""Optional current-main nested diagnostic; run only after primary collection.

Reuses prepared #256 hooks; modifies archived copies only. No primary binary edits.
All new nested timers are disabled until CAPACITY_FINE is set.
"""
from pathlib import Path
import sys,os,json,subprocess,shutil,hashlib
r=Path(__file__).resolve().parent;root=r/'diagnostic-candidate';assert not root.exists()
os.link(r/'candidate.tar',r/'diagnostic-candidate.tar')
subprocess.run([sys.executable,str(r/'build.py'),str(r/'shared248'),'diagnostic-candidate','--prepare-only'],check=True,stdout=subprocess.DEVNULL)
source=root/'source'
def change(name,old,new):
 p=source/name;s=p.read_text();assert s.count(old)==1,(name,old,s.count(old));p.write_text(s.replace(old,new),newline='\n')
def scope(name,anchor,stage):
 p=source/name;s=p.read_text();at=s.index('{',s.index(anchor));p.write_text(s[:at+1]+f'\nlet _fine_clock=crate::study_observe::CpuGuard::new({stage});'+s[at+1:],newline='\n')
p=source/'src/study_observe.rs';s=p.read_text().replace('[u64;16]','[u64;32]').replace('[0;16]','[0;32]')
s=s.replace('static DETAIL:AtomicBool','static FINE:AtomicBool=AtomicBool::new(false);\nstatic DETAIL:AtomicBool')
s=s.replace('if tick%100==0{DETAIL.store','if tick%100==0{FINE.store(std::env::var_os("CAPACITY_FINE").is_some_and(|p|std::path::Path::new(&p).exists()),Ordering::Relaxed);DETAIL.store')
s=s.replace('stage<10||DIAGNOSTIC.load(Ordering::Relaxed)','stage<10||(stage>=16&&FINE.load(Ordering::Relaxed))||DIAGNOSTIC.load(Ordering::Relaxed)')
p.write_text(s,newline='\n')
f='src/client/simulation/broadcast.rs'
change(f,'        let food_changes = self.sync_food_sectors()?;','        let prep_clock=crate::study_observe::CpuGuard::new(16);\n        let food_changes = self.sync_food_sectors()?;')
change(f,'        let mut board = None;','        drop(prep_clock);\n        let mut board = None;')
change(f,'        let vision = world.config().vision;','        let snake_clock=crate::study_observe::CpuGuard::new(17);\n        let vision = world.config().vision;')
change(f,'        let food_start = packets.len();','        drop(snake_clock);\n        let food_clock=crate::study_observe::CpuGuard::new(18);\n        let food_start = packets.len();')
change(f,'        let mut visible = BTreeMap::new();','        drop(food_clock);\n        let _prey_clock=crate::study_observe::CpuGuard::new(19);\n        let mut visible = BTreeMap::new();')
change(f,'        self.tick += 1;','        let subscription_clock=crate::study_observe::CpuGuard::new(23);\n        self.tick += 1;')
change(f,'        self.broadcast_with(early, hand_over, false)?;','        drop(subscription_clock);\n        self.broadcast_with(early, hand_over, false)?;')
for anchor,stage in [('    pub(super) fn minimap(',20),('    fn hud(',21),('    pub(super) fn advance_with(',24)]:scope(f,anchor,stage)
scope('src/client/simulation/connection.rs','    pub(super) fn emit_presented(',22)
scope('src/client/simulation/food.rs',"    fn sync<'a>(",27)
change('src/client/simulation/food.rs','    let held = std::mem::take(&mut known.entries);','    let _membership_clock=crate::study_observe::CpuGuard::new(28);\n    let held = std::mem::take(&mut known.entries);')
for anchor,stage in [('    fn apply_commands(',25),('    fn hand_over(',26)]:scope('src/client/server.rs',anchor,stage)
if '--prepare-only' in sys.argv:print('FINE SOURCE PREPARED');raise SystemExit(0)
target=Path('/tmp/slither-live-checkpoint-target');assert not target.exists()
env=dict(os.environ,CARGO_BUILD_JOBS='1',CARGO_TARGET_DIR=str(target),CARGO_PROFILE_RELEASE_DEBUG='1',RUSTFLAGS='-C target-cpu=native',CARGO_NET_OFFLINE='true');env.pop('CARGO_ENCODED_RUSTFLAGS',None)
env['RUSTC']=subprocess.check_output(['rustup','which','rustc'],text=True).strip();os.sched_setaffinity(0,{0})
with (root/'build.log').open('x') as log:subprocess.run(['cargo','build','--release','--locked','--offline','-j','1','--bin','slither-sim'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
(root/'bin').mkdir();shutil.copy2(target/'release/slither-sim',root/'bin/host')
sha=lambda p:hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
(root/'pins.json').write_text(json.dumps({'sourceCommit':json.loads((r/'candidate/pins.json').read_text())['commit'],'binaries':{'host':sha(root/'bin/host')},'builderSha256':sha(r/'build_fine.py'),'timingSha256':sha(r/'timing.rs')},indent=2)+'\n')
assert target.resolve()==target and not target.is_symlink();shutil.rmtree(target)
print('FINE HOST BUILT; TARGET REMOVED',flush=True)
