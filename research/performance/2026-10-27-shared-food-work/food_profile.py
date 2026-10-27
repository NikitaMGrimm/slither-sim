"""Prepare/run whole-tick brackets, splitting the old unattributed bucket."""
import argparse
from pathlib import Path
import re
import subprocess
import sys
if not __debug__:
    raise SystemExit('Run without optimization')
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('ref');p.add_argument('output',type=Path);p.add_argument('--control',type=Path,required=True)
p.add_argument('--prepare-only',action='store_true');p.add_argument('--build-only',action='store_true');p.add_argument('--cpu',type=int,default=3);a=p.parse_args()
root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
archive=root/'source.tar'
subprocess.run(['git','archive','-o',str(archive),a.ref],check=True)
prefix='research/performance/2026-10-27-host-food-ranges/'
driver=subprocess.check_output(['git','show',f'{a.ref}:{prefix}profile.py'],text=True)
counter=subprocess.check_output(['git','show',f'{a.ref}:{prefix}profile.rs'],text=True)
names=['world_step','shared_food','reports','bounds','grid','prey_bounds','candidates','snake_updates','recipient_food','recipient_prey','framing','legacy','board','map','hud','broadcast',
       'ingest','actions','food_events','settle','advance_interest','processing_overrides','client_completion','prey_ids','prey_clocks','broadcast_interest','updates','updates_prefix','recipient_envelope','reveal_rebuild','broadcast_clock_setup','background_decisions','food_setup','food_subscriptions','food_sector_loop','food_build','food_tail']
counter=counter.replace('const N:usize=16;',f'const N:usize={len(names)};')
counter=re.sub(r'const NAMES:\[&str;N\]=\[.*?\];',f'const NAMES:[&str;N]=[{",".join(chr(34)+n+chr(34) for n in names)}];',counter)
counter=counter.replace('phases:[u64;N],calls:[u64;N]}','#[serde(serialize_with="serialize_counts")]phases:[u64;N],#[serde(serialize_with="serialize_counts")]calls:[u64;N]}')
counter+='\nfn serialize_counts<S:serde::Serializer>(v:&[u64;N],s:S)->Result<S::Ok,S::Error>{serde::Serialize::serialize(v.as_slice(),s)}\n'
counter=counter.replace('"phases":NAMES,','"phases":NAMES.as_slice(),')
driver=driver.replace("shutil.copy2(folder/'profile.rs',source/'src/recipient_profile.rs')","(source/'src/recipient_profile.rs').write_text("+repr(counter)+")")
# #215 replaced borrowed legacy output with owned packet conversion.
old="edit('src/client/encode13.rs','        let mut out = Vec::with_capacity(packets.len());','        let _study_legacy=crate::recipient_profile::Scope::new(11);\\n        let mut out = Vec::with_capacity(packets.len());')"
# Keep the old hook only when present; current conversion no longer has it.
driver=driver.replace(old,"# legacy conversion is nested in framing; no extra hook")

before = "edit(b,line,'        drop(study_bounds);\\n        let study_grid=crate::recipient_profile::Scope::new(4);\\n'+line+'\\n        drop(study_grid);')"
after = """if line in (source/b).read_text():
 edit(b,line,'        drop(study_bounds);\\n        let study_grid=crate::recipient_profile::Scope::new(4);\\n'+line+'\\n        drop(study_grid);')
else:
 edit(b,'        let grid = if let Some(mut grid) = self.coverage_grid.take() {','        drop(study_bounds);\\n        let study_grid=crate::recipient_profile::Scope::new(4);\\n        let grid = if let Some(mut grid) = self.coverage_grid.take() {')
 line='        let prey_bounds = PreyBounds::new(self.world.prey(), self.world.config().vision);'
 edit(b,line,'        drop(study_grid);\\n'+line)
"""
assert driver.count(before)==1
driver=driver.replace(before,after)

hooks=r'''
edit(b,'        let end = (self.tick + 1) as f64 * self.world.config().dt;','        let study_input=crate::recipient_profile::Scope::new(16);\n        let end = (self.tick + 1) as f64 * self.world.config().dt;')
edit(b,'        let actions: Vec<_> = self','        drop(study_input);\n        let study_actions=crate::recipient_profile::Scope::new(17);\n        let actions: Vec<_> = self')
edit(b,'        let radius = self.world.radius();','        drop(study_actions);\n        let radius = self.world.radius();')
edit(b,'        // Mirror update_radius\'s food removal before births and pickups,','        let study_food_events=crate::recipient_profile::Scope::new(18);\n        // Mirror update_radius\'s food removal before births and pickups,')
edit(b,'        self.settle_lives();','        drop(study_food_events);\n        let study_settle=crate::recipient_profile::Scope::new(19);\n        self.settle_lives();\n        drop(study_settle);')
edit(b,'        self.tick += 1;','        let study_interest=crate::recipient_profile::Scope::new(20);\n        self.tick += 1;')
edit(b,'        for slot in &mut self.slots {','        drop(study_interest);\n        let study_processing=crate::recipient_profile::Scope::new(21);\n        for slot in &mut self.slots {')
edit(b,'        self.broadcast_with(early, hand_over)?;','        drop(study_processing);\n        self.broadcast_with(early, hand_over)?;\n        let _study_completion=crate::recipient_profile::Scope::new(22);')
scope(b,'    fn assign_prey_wire(&mut self) -> Result<(), String> {',23)
scope(b,'    fn update_prey_clocks(&mut self) -> Result<(), String> {',24)
edit(b,'        // Dead clients still render the world during the stock death fade;','        let study_broadcast_interest=crate::recipient_profile::Scope::new(25);\n        // Dead clients still render the world during the stock death fade;')
edit(b,'        let food_changes = self.sync_food_sectors()?;','        drop(study_broadcast_interest);\n        let food_changes = self.sync_food_sectors()?;')
edit(b,'        let world = &self.world;','        let _study_updates=crate::recipient_profile::Scope::new(26);\n        let study_prefix=crate::recipient_profile::Scope::new(27);\n        let world = &self.world;')
edit(b,'        let study_candidates=crate::recipient_profile::Scope::new(6);','        drop(study_prefix);\n        let study_candidates=crate::recipient_profile::Scope::new(6);')
edit(b,'            if slot == early {','            let _study_envelope=crate::recipient_profile::Scope::new(28);\n            if slot == early {')
edit(b,'        self.legacy_maps.clear();','        let study_reveal=crate::recipient_profile::Scope::new(29);\n        self.legacy_maps.clear();')
edit(b,'        self.update_reports()?;','        drop(study_reveal);\n        self.update_reports()?;\n        let study_clocks=crate::recipient_profile::Scope::new(30);')
edit(b,'        let study_broadcast_interest=crate::recipient_profile::Scope::new(25);','        drop(study_clocks);\n        let study_broadcast_interest=crate::recipient_profile::Scope::new(25);')
scope('src/client/simulation.rs','    fn drive_background(&mut self) -> Result<(), String> {',31)
'''

hooks+=r"""
def production_edit(name,before,after):
 p=source/name;body,separator,tests=p.read_text().partition('\n#[cfg(test)]\nmod tests {')
 assert body.count(before)==1,(name,before,body.count(before))
 p.write_text(body.replace(before,after)+separator+tests)
edit(f,'        let eaten = eaten_sectors(&self.food_eaters);','        let study_food_setup=crate::recipient_profile::Scope::new(32);\n        let eaten = eaten_sectors(&self.food_eaters);')
edit(f,'        let retired = self.food_subscriptions.sync(self.slots.iter().map(|slot| {','        drop(study_food_setup);\n        let study_food_subscriptions=crate::recipient_profile::Scope::new(33);\n        let retired = self.food_subscriptions.sync(self.slots.iter().map(|slot| {')
production_edit(f,'        // Listings are independent and indexed by sector. Recipient packet','        drop(study_food_subscriptions);\n        let study_food_loop=crate::recipient_profile::Scope::new(34);\n        // Listings are independent and indexed by sector. Recipient packet')
edit(f,'                let run = self.food_index.listing(index);','                let _study_food_build=crate::recipient_profile::Scope::new(35);\n                let run = self.food_index.listing(index);')
production_edit(f,'        // Unlisted sectors are built afresh when a connection subscribes.','        drop(study_food_loop);\n        let _study_food_tail=crate::recipient_profile::Scope::new(36);\n        // Unlisted sectors are built afresh when a connection subscribes.')
"""

idx=driver.index('\nenv=dict(os.environ')
driver=driver[:idx]+'\n'+hooks+driver[idx:]
driver=driver.replace('range(16)',f'range({len(names)})')
driver=driver.replace("CARGO_TARGET_DIR=str(out/'target')","CARGO_TARGET_DIR=os.environ.get('TICK_TARGET',str(out/'target'))")
driver=driver.replace("out/'target/release/examples/study_protocols'","Path(env['CARGO_TARGET_DIR'])/'release/examples/study_protocols'")
(root/'driver.py').write_text(driver)
if a.prepare_only:driver=driver[:driver.index('\nenv=dict(os.environ')]
elif a.build_only:driver=driver[:driver.index('\nselected={};ticks=None')]
sys.argv=[sys.argv[0],str(archive),str(root),'--cpu',str(a.cpu),'--control',str(a.control.resolve())]
exec(compile(driver,str(root/'driver.py'),'exec'))
