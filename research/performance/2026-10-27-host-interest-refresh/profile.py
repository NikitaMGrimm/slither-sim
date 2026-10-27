"""Split tick overhead in either frozen arm; Linux x86-64 user PMU."""
from pathlib import Path
import sys,tarfile,re,json
if not __debug__:raise SystemExit('run without -O')
archive=Path(sys.argv[1])
with tarfile.open(archive) as t:
    wrapper=t.extractfile('research/performance/2026-10-27-host-coverage-cells/profile.py').read().decode()
    broadcast=t.extractfile('src/client/simulation/broadcast.rs').read().decode()
ns={'__file__':__file__,'__name__':'__main__'}
exec(compile(wrapper[:wrapper.rindex('exec(compile(driver')],__file__,'exec'),ns)
driver,counter=ns['driver'],ns['counter'];old=counter
old_signature='    fn broadcast_with(&mut self, early: usize, hand_over: &mut EarlyFrames) -> Result<(), String> {'
signature=re.search(r'    fn broadcast_with\([\s\S]*?\) -> Result<\(\), String> \{',broadcast).group()
assert driver.count(repr(old_signature))==1
driver=driver.replace(repr(old_signature),repr(signature))
names=['world_step','shared_food','reports','bounds','grid','prey_bounds','candidates','snake_updates','recipient_food','recipient_prey','framing','legacy','board','map','hud','broadcast','grid_cleanup','ingest','actions','food_events','settle','interest_tick','processing','client_until','interest_broadcast','prey_wire','prey_clocks','recipient_loop','updates','hand_over','packet_drop']
counter=counter.replace('const N:usize=17;','const N:usize=31;');counter=re.sub(r'const NAMES:\[&str;N\]=\[.*?\];','const NAMES:[&str;N]='+json.dumps(names,separators=(',',':'))+';',counter)
before="(source/'src/recipient_profile.rs').write_text("+repr(old)+")";assert driver.count(before)==1;driver=driver.replace(before,"(source/'src/recipient_profile.rs').write_text("+repr(counter)+")").replace('range(17)','range(31)')
hooks='''
edit(b,'        let end = (self.tick + 1) as f64 * self.world.config().dt;','        let study_ingest=crate::recipient_profile::Scope::new(17);\\n        let end = (self.tick + 1) as f64 * self.world.config().dt;')
edit(b,'        let actions: Vec<_> = self','        drop(study_ingest);\\n        let study_actions=crate::recipient_profile::Scope::new(18);\\n        let actions: Vec<_> = self')
edit(b,'        let radius = self.world.radius();','        drop(study_actions);\\n        let radius = self.world.radius();')
edit(b,'        drop(study_physics);','        drop(study_physics);\\n        let study_events=crate::recipient_profile::Scope::new(19);')
edit(b,'        self.settle_lives();','        drop(study_events);\\n        let study_settle=crate::recipient_profile::Scope::new(20);\\n        self.settle_lives();\\n        drop(study_settle);\\n        let study_interest=crate::recipient_profile::Scope::new(21);')
edit(b,'        for slot in &mut self.slots {','        drop(study_interest);\\n        let study_processing=crate::recipient_profile::Scope::new(22);\\n        for slot in &mut self.slots {')
edit(b,'        self.broadcast_with(early, hand_over)?;','        drop(study_processing);\\n        self.broadcast_with(early, hand_over)?;')
edit(b,'        for slot in 0..self.slots.len() {\\n            self.client_until(slot, now)?;','        let study_clients=crate::recipient_profile::Scope::new(23);\\n        for slot in 0..self.slots.len() {\\n            self.client_until(slot, now)?;')
edit(b,'        // Dead clients still render the world during the stock death fade;','        let study_interest=crate::recipient_profile::Scope::new(24);\\n        // Dead clients still render the world during the stock death fade;')
edit(b,'        let food_changes = self.sync_food_sectors()?;','        drop(study_interest);\\n        let food_changes = self.sync_food_sectors()?;')
scope(b,'    fn assign_prey_wire(&mut self) -> Result<(), String> {',25)
scope(b,'    fn update_prey_clocks(&mut self) -> Result<(), String> {',26)
edit(b,'        for slot in 0..self.slots.len() {\\n            if slot == early {','        for slot in 0..self.slots.len() {\\n            let _study_recipient=crate::recipient_profile::Scope::new(27);\\n            if slot == early {')
edit(b,'    ) -> Result<PacketViews, String> {','    ) -> Result<PacketViews, String> {\\n        let _study_updates=crate::recipient_profile::Scope::new(28);')
scope(b,'    fn hand_over_early(&mut self, early: usize, hand_over: &mut EarlyFrames) -> Result<(), String> {',29)
edit(b,'            } else if !packets.is_empty() {\\n                self.emit(slot, now, &packets)?;\\n            }','            } else if !packets.is_empty() {\\n                self.emit(slot, now, &packets)?;\\n            }\\n            let study_drop=crate::recipient_profile::Scope::new(30);\\n            drop(packets);\\n            drop(study_drop);')
'''
if '        advance_interest: bool,' in broadcast:
    hooks=hooks.replace('self.broadcast_with(early, hand_over)?','self.broadcast_with(early, hand_over, false)?')
    hooks=hooks.replace('// Dead clients still render the world during the stock death fade;','// Normal ticks already advance every subscription before entering this')
driver=driver.replace('\nenv=dict(os.environ',hooks+'\nenv=dict(os.environ')
exec(compile(driver,str(archive)+'-residual','exec'),ns)
