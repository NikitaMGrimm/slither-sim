"""Battledome measurement-only deltas on #258's archived-source overlay."""
from pathlib import Path
import runpy,json,hashlib
r=Path(__file__).resolve().parent
runpy.run_path(str(r/'prepare258.py'))
source=r/'server/source'
def swap(name,old,new):
 p=source/name;s=p.read_text();assert s.count(old)==1,(name,old,s.count(old));p.write_text(s.replace(old,new),newline='\n')
# Keep the original aligned clocks; retain phases/events for each individual tick.
swap('src/study_observe.rs','stages:[u64;16],rows:Vec<[u64;3]>','stages:[u64;32],rows:Vec<serde_json::Value>,events:[u64;8]')
swap('src/study_observe.rs','s.rows.push([sim-s.last_sim,io-s.last_io,wall.elapsed().as_nanos() as u64]);','let phases=s.stages;let events=s.events;s.rows.push(serde_json::json!([sim-s.last_sim,io-s.last_io,wall.elapsed().as_nanos() as u64,phases,events]));s.stages=[0;32];s.events=[0;8];')
swap('src/study_observe.rs','s.rows.clear();s.stages=[0;16];','s.rows.clear();s.stages=[0;32];')
swap('src/study_observe.rs','s.stages=[0;16];','s.stages=[0;32];')
swap('src/study_observe.rs','stage<10||DIAGNOSTIC','stage<10||stage>=16||DIAGNOSTIC')
swap('src/study_observe.rs','events:[u64;8]','events:[u64;8],pending:Vec<(&\'static str,serde_json::Value)>')
swap('src/study_observe.rs','if !active(){CLOCK.with_borrow_mut(|s|{s.last_sim=0;s.last_io=0;});return;}','if !active(){CLOCK.with_borrow_mut(|s|{s.last_sim=0;s.last_io=0;for (tag,value) in s.pending.drain(..){eprintln!("{tag} {value}");}});return;}')
swap('src/study_observe.rs','eprintln!("CAPACITY {}",serde_json::json!({','s.pending.push(("CAPACITY",serde_json::json!({')
swap('src/study_observe.rs','"stages":s.stages}));s.rows.clear();','"stages":s.stages})));s.rows.clear();')
swap('src/study_observe.rs','eprintln!("CENSUS {}", serde_json::json!({','report("CENSUS", serde_json::json!({')
swap('src/study_observe.rs','for (tag,value) in s.pending.drain(..){eprintln!("{tag} {value}");}','for (tag,value) in s.pending.drain(..){let raw=serde_json::to_string(&value).unwrap();eprintln!("{tag} {raw}");}')
with (source/'src/study_observe.rs').open('a') as f:f.write('''
pub fn report(tag:&'static str,value:serde_json::Value){if active(){CLOCK.with_borrow_mut(|s|s.pending.push((tag,value)));}else{let raw=serde_json::to_string(&value).unwrap();eprintln!("{tag} {raw}");}}
pub fn event(index:usize,n:u64){if active(){CLOCK.with_borrow_mut(|s|s.events[index]+=n);}}
thread_local!{static BOOST:RefCell<[u64;3]>=const{RefCell::new([0;3])};}
pub fn boost_totals(world:&crate::Simulator)->[u64;3]{BOOST.with_borrow_mut(|s|{
 s[0]+=world.snakes().iter().filter(|x|x.alive).count() as u64;
 s[1]+=(0..world.snakes().len()).filter(|&i|world.snakes()[i].alive&&world.is_boosting(i)).count() as u64;
 s[2]+=world.food_spawn_events().unwrap_or(&[]).iter().filter(|e|e.food.origin==Some(crate::model::Origin::Boost)).count() as u64;*s
})}
''')
swap('src/study_observe.rs','    let elapsed = start.elapsed().as_nanos() as u64;','    let boost=boost_totals(world);let elapsed = start.elapsed().as_nanos() as u64;')
swap('src/study_observe.rs','"tick":tick,"deferred":deferred,','"boostTotals":boost,"boostFood":world.food().iter().filter(|f|f.origin==Some(crate::model::Origin::Boost)).count(),"tick":tick,"deferred":deferred,')
swap('src/client/simulation.rs','let _clock=crate::study_observe::CpuGuard::new(7);','let _clock=crate::study_observe::CpuGuard::new(7);crate::study_observe::event(3,1);')
swap('src/client/simulation.rs','                return Ok((body, heading));','                crate::study_observe::event(4,1);return Ok((body, heading));') if False else None
# Distinguish strict acceptance, directional fallback and failed searches.
p=source/'src/client/simulation.rs';s=p.read_text();start=s.index('    fn spawn_pose_waited(');end=s.index('    /// Drive non-controlled',start);chunk=s[start:end]
assert chunk.count('return Ok((body, heading));')==2
chunk=chunk.replace('return Ok((body, heading));','crate::study_observe::event(4,1);return Ok((body, heading));',1)
pos=chunk.index('        if circular {\n            if let Some')
chunk=chunk[:pos]+chunk[pos:].replace('return Ok((body, heading));','crate::study_observe::event(5,1);return Ok((body, heading));',1).replace('return Err(spawn::WAITING.into());','crate::study_observe::event(6,1);return Err(spawn::WAITING.into());')
s=s[:start]+chunk+s[end:];p.write_text(s,newline='\n')
swap('src/client/simulation/broadcast.rs','            self.food_epoch = epoch;','            crate::study_observe::event(0,1);self.food_epoch = epoch;')
swap('src/client/simulation/broadcast.rs','        let food_changes = self.sync_food_sectors()?;','        crate::study_observe::event(1,minimap_due as u64);crate::study_observe::event(2,leaderboard_due as u64);\n        let food_changes = {let _clock=crate::study_observe::CpuGuard::new(16);self.sync_food_sectors()?};\n        let study_coverage=crate::study_observe::CpuGuard::new(17);')
swap('src/client/simulation/broadcast.rs','        let mut board = None;','        drop(study_coverage);let study_recipients=crate::study_observe::CpuGuard::new(18);\n        let mut board = None;')
swap('src/client/simulation/broadcast.rs','                board = Some(Board::new(&self.world, &self.profiles)?);','                let _clock=crate::study_observe::CpuGuard::new(20);board = Some(Board::new(&self.world, &self.profiles)?);')
swap('src/client/simulation/broadcast.rs','                prefix.push(packet);','                prefix.push(packet);') if False else None
swap('src/client/simulation/broadcast.rs','            if minimap_due && let Some(packet) = self.minimap(slot, &mut map)? {','            let study_map=crate::study_observe::CpuGuard::new(19);\n            if minimap_due && let Some(packet) = self.minimap(slot, &mut map)? {')
swap('src/client/simulation/broadcast.rs','            // Most broadcasts have neither pongs nor a map.','            drop(study_map);\n            // Most broadcasts have neither pongs nor a map.')
swap('src/client/simulation/broadcast.rs','        self.coverage_grid = Some(grid);','        drop(study_recipients);self.coverage_grid = Some(grid);')
swap('src/client/simulation/food.rs','        let now = self.time();\n        let pit_changed = match','        let now = self.time();\n        let study_markers=crate::study_observe::CpuGuard::new(21);let pit_changed = match')
swap('src/client/simulation/food.rs','        let retired = self.food_subscriptions.sync','        drop(study_markers);crate::study_observe::event(7,pit_changed as u64);let retired = self.food_subscriptions.sync')
# Extend the #258 client, without changing policy inputs, decisions or packet decoding.
name='examples/aws_clients.rs'
swap(name,'pending:Option<f64>}','pending:Option<f64>,login_at:f64,ever:bool,admitted:bool}')
swap(name,'blocked:false,pending:None}','blocked:false,pending:None,login_at:at,ever:false,admitted:false}')
swap(name,'*s=Seat::connect(s.id,epoch,port)?;joins+=1;','let ever=s.ever;*s=Seat::connect(s.id,epoch,port)?;s.ever=ever;joins+=1;')
swap(name,'  let churn=tick.min(179999)*rotating as u64/20000;','  let _=(tick,residents,rotating);let churn=0;')
swap(name,'let mut previous_churn=0u64;','let mut previous_churn=0u64;let mut gaps=std::collections::BTreeMap::<u64,u64>::new();let mut joins_ms=Vec::<[u64;3]>::new();let mut respawns_ms=Vec::<[u64;3]>::new();let mut boosted=0u64;let mut action_count=0u64;let mut life_start=std::collections::BTreeMap::<usize,f64>::new();let mut lifetimes=Vec::<u64>::new();')
swap(name,'let at=epoch.elapsed().as_secs_f64();s.state.receive_frame(&data,at)?;s.last_read=at;', '''let at=epoch.elapsed().as_secs_f64();let was=s.state.status()==ClientStatus::Playing;s.state.receive_frame(&data,at)?;
     if was&&s.state.status()==ClientStatus::Playing{*gaps.entry(((at-s.last_read).max(0.)*1e6) as u64).or_default()+=1;}
     if !s.admitted&&s.state.status()==ClientStatus::Playing{let row=[s.id as u64,((at-s.login_at)*1e6) as u64,(at*1e6) as u64];if s.ever{respawns_ms.push(row);}else{joins_ms.push(row);}s.ever=true;s.admitted=true;life_start.insert(s.id,at);}s.last_read=at;''')
swap(name,'s.boost=Some(action.boost);decisions+=1;','s.boost=Some(action.boost);decisions+=1;action_count+=1;boosted+=action.boost as u64;')
swap(name,'if retire{forced+=1;}else{deaths+=1;}','if retire{forced+=1;}else{deaths+=1;if let Some(born)=life_start.remove(&s.id){lifetimes.push(((at-born)*1e6) as u64);}}')
swap(name,'"unix":std::time::SystemTime::now()', '"gapsMicros":gaps.iter().map(|(k,v)|[*k,*v]).collect::<Vec<_>>(),"initialWaits":joins_ms,"respawnWaits":respawns_ms,"lifetimesMicros":lifetimes,"boostActions":boosted,"actions":action_count,"pendingWaitMicros":seats.iter().filter(|s|s.pending.is_none()&&!s.admitted).map(|s|((epoch.elapsed().as_secs_f64()-s.login_at)*1e6) as u64).collect::<Vec<_>>(),"unix":std::time::SystemTime::now()')
swap(name,'max_pass=0;lates.clear();','max_pass=0;lates.clear();gaps.clear();joins_ms.clear();respawns_ms.clear();lifetimes.clear();boosted=0;action_count=0;')
swap(name,'login_at:f64,ever:bool,admitted:bool}','login_at:f64,ever:bool,admitted:bool,server_ms:u64,excess_min:f64}')
swap(name,'login_at:at,ever:false,admitted:false}','login_at:at,ever:false,admitted:false,server_ms:0,excess_min:f64::INFINITY}')
swap(name,'let mut lifetimes=Vec::<u64>::new();','let mut lifetimes=Vec::<u64>::new();let mut residuals=std::collections::BTreeMap::<i64,u64>::new();let mut excess=std::collections::BTreeMap::<u64,u64>::new();')
swap(name,'let was=s.state.status()==ClientStatus::Playing;s.state.receive_frame(&data,at)?;','let was=s.state.status()==ClientStatus::Playing;s.state.receive_frame(&data,at)?;let header_ms=u16::from_be_bytes([data[0],data[1]]) as u64;s.server_ms+=header_ms;let residual=at-s.server_ms as f64/1000.;s.excess_min=s.excess_min.min(residual);')
swap(name,'if was&&s.state.status()==ClientStatus::Playing{*gaps.entry','if was&&s.state.status()==ClientStatus::Playing{*residuals.entry(((at-s.last_read)*1e6) as i64-(header_ms*1000) as i64).or_default()+=1;*excess.entry(((residual-s.excess_min).max(0.)*1e6) as u64).or_default()+=1;*gaps.entry')
swap(name,'"gapsMicros":gaps.iter()','"gapResidualMicros":residuals.iter().map(|(k,v)|[*k,*v as i64]).collect::<Vec<_>>(),"frameExcessMicros":excess.iter().map(|(k,v)|[*k,*v]).collect::<Vec<_>>(),"gapsMicros":gaps.iter()')
swap(name,'boosted=0;action_count=0;','boosted=0;action_count=0;residuals.clear();excess.clear();')
swap(name,'let mut observed_target=count;','let mut observed_target=count;let mut observed_reset=String::new();')
swap(name,'  if target!=observed_target{','  let reset=std::fs::read_to_string(root.join("RESET")).unwrap_or_default();if reset!=observed_reset{for seat in &mut seats{seat.excess_min=f64::INFINITY;}observed_reset=reset;}\n  if target!=observed_target{')
p=r/'prepared.json';pins=json.loads(p.read_text());pins['runtime']='165a5bb764aba6cbf39f42503e090f9b5e3c4d22';pins['clientSourceSha256']=hashlib.file_digest((source/name).open('rb'),'sha256').hexdigest();pins['sourceOverlays']={str(p.relative_to(source)):hashlib.file_digest(p.open('rb'),'sha256').hexdigest() for p in (source/'src').rglob('*.rs')};p.write_text(json.dumps(pins,indent=2)+'\n')
print('BATTLEDOME EXPERIENCE OVERLAYS PREPARED')
