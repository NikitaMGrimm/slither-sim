"""Prepare the fresh world-step diagnostic; no production checkout edits."""
import argparse,hashlib,json,shutil,subprocess,tarfile
from pathlib import Path
if not __debug__:raise SystemExit('Run without Python optimization')
p=argparse.ArgumentParser();p.add_argument('ref');p.add_argument('root',type=Path);p.add_argument('--nested',action='store_true');a=p.parse_args()
root=a.root.resolve();source=root/'source';source.mkdir(parents=True,exist_ok=False)
archive=root/'source.tar';subprocess.run(['git','archive','-o',str(archive),a.ref],check=True)
with tarfile.open(archive) as f:f.extractall(source,filter='data')
counter=subprocess.check_output(['git','show','40ce9fac41db8b3043b827056f21ebefe27a706a:research/performance/2026-10-27-food-compaction/counter.rs'],text=True)
counter=counter.replace('[u64;16]','[u64;32]').replace('[0;16]','[0;32]')
counter=counter.replace('fd:c_int,tick:usize,','fd:c_int,active:bool,tick:usize,').replace('Counter{fd,tick:0,','Counter{fd,active:false,tick:0,')
counter=counter.replace('pub fn mark(stage:u64){C.with_borrow_mut(|c|{','pub fn mark(stage:u64){C.with_borrow_mut(|c|{if stage==0||stage==11{c.active=true;}')
counter=counter.replace('c.last=v;});}', 'c.last=v;if stage==10{c.active=false;}});}')
counter=counter.replace('let end=c.read();assert!', 'if !c.active{return;}let end=c.read();assert!')
(source/'src/core_profile.rs').write_text(counter)
for name,extra in [('src/lib.rs','\npub mod core_profile;\n'),('src/client/simulation.rs','\npub mod study_protocols;\n')]:
 q=source/name;q.write_text(q.read_text()+extra)
fixture=source/'research/performance/2026-10-27-host-snake-candidates'
s=(fixture/'protocol_fixture.rs').read_text().replace('        sim.advance_tick_with(0,','        crate::core_profile::begin(tick);\n        sim.advance_tick_with(0,').replace('        population_sum += population;','        crate::core_profile::end(population);\n        population_sum += population;')
(source/'src/client/simulation/study_protocols.rs').write_text(s)
child=source/'src/client/simulation/study_protocols';child.mkdir();shutil.copy2(fixture/'tick_counter.rs',child/'tick_counter.rs')
(source/'examples/study_protocols.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_protocols::run(&a[1],a.get(2).is_some_and(|s|s=="--trace"));}\n')
def edit(name,old,new):
 q=source/name;s=q.read_text()
 if s.count(old)!=1:raise SystemExit(f'Ambiguous hook {name}: {old}: {s.count(old)}')
 q.write_text(s.replace(old,new))
def function(name,signature,stage):
 q=source/name;s=q.read_text()
 if s.count(signature)!=1:raise SystemExit('Ambiguous function '+signature)
 i=s.index('{',s.index(signature))+1
 q.write_text(s[:i]+f'\n        let _core_scope=crate::core_profile::scope({stage});'+s[i:])
f='src/simulator.rs'
prey_call='        self.advance_prey(previous, prey_buffers);'
if prey_call not in (source/f).read_text():prey_call='        self.advance_prey(previous, heads, eaters, prey_buffers);'
edit(f,'    pub fn step(&mut self, actions: &[Option<Action>]) -> Result<(), CoreStepError> {','    pub fn step(&mut self, actions: &[Option<Action>]) -> Result<(), CoreStepError> {\n        crate::core_profile::mark(11);')
for stage,old in [(0,'        let mut clock = if PROFILE { Some(Instant::now()) } else { None };'),(1,'        self.food_cache.begin_mirrored(&self.food);'),(2,'        // Stable-slot indexing avoids'),(3,'        // The offline Android client resolves collisions'),(4,'        candidates.clear();\n        dead.clear();'),(5,'        if PROFILE {\n            profile.collision_seconds'),(6,'        // Without food there is nothing to index or eat.'),(7,prey_call),(9,'        self.food_cache.end();\n        if PROFILE {')]:
 edit(f,old,f'        crate::core_profile::mark({stage});\n'+old)
edit(f,prey_call,prey_call+'\n        crate::core_profile::mark(8);')
edit(f,'        self.buffers = buffers;','        self.buffers = buffers;\n        crate::core_profile::mark(10);')
if a.nested:
 for name,signature,stage in [
 ('src/bounds_index.rs','    pub(crate) fn new(bounds: &[Bounds]) -> Self {',0),
 ('src/physics.rs','pub(crate) fn strikes_scaled(',2),
 ('src/physics.rs','pub fn advance(\n',3),
 ('src/physics.rs','pub fn append_anchor(',4),
 ('src/physics.rs','    pub fn step(&mut self, boosting:',5),
 ('src/kiosk.rs','    pub(crate) fn step(\n',6),
 ('src/food_grid.rs','    pub(crate) fn remove(&mut self, food_len:',7),
 ('src/food_grid.rs','    pub(crate) fn build(\n',8),
 ('src/simulator.rs','fn indexed_hits<const PROFILE:',9),
 ('src/food_grid.rs','    pub(crate) fn has_room(',10),
 ('src/food_grid.rs','    pub(crate) fn push(&mut self, pellet:',11),
 ('src/simulator.rs','    fn advance_prey(',12),
 ('src/simulator.rs','    fn decide_prey(',15),
 ('src/physics.rs','pub fn pickup_circle(',16),
 ('src/prey.rs','    pub fn advance_server_tick(',17),
 ]:function(name,signature,stage)
 for signature,stage in [('pub(crate) fn advance_directed(',3),('pub(crate) fn pickup_circle_directed(',16)]:
  if signature in (source/'src/physics.rs').read_text():function('src/physics.rs',signature,stage)

 old='            let indexed = index.as_ref().is_some_and(|(index, longest)| {'
 edit(f,old,'            let core_query=crate::core_profile::scope(1);\n'+old)
 old='            let test = |j: usize, profile: &mut StepProfile| {'
 edit(f,old,'            drop(core_query);\n'+old)
 # Existing score calls are inside movement and pickups, not borrowed by broadcast.
 function('src/simulator.rs','    fn update_mass(',13)
 function('src/simulator.rs','    pub fn validate_actions(',14)
 edit('src/food_grid.rs','            // Stable compaction leaves the prefix before the first removal in','            let _position_scope=crate::core_profile::scope(19);\n            // Stable compaction leaves the prefix before the first removal in')
changed=['src/lib.rs','src/client/simulation.rs','src/client/simulation/study_protocols.rs','src/client/simulation/study_protocols/tick_counter.rs','examples/study_protocols.rs','src/core_profile.rs','src/simulator.rs','src/physics.rs','src/bounds_index.rs','src/food_grid.rs','src/kiosk.rs','src/prey.rs']
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
(root/'source-pins.json').write_text(json.dumps({'ref':a.ref,'nested':a.nested,'archiveSha256':sha(archive),'overlay':{n:sha(source/n) for n in changed}},indent=2)+'\n')
print('PREPARED',a.ref,'nested',a.nested)
