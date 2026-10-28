//! Offline research fixture: full deterministic host since tick zero.
use super::{RawSimulator, SimulationConfig, PlayerAppearance};
use crate::{Action, Simulator, bot::{Forager, Timing}};
use std::{fs, io::{self, Write, BufRead}, path::Path};
const SEATS:usize=500;
unsafe extern "C" {fn fork()->i32;fn waitpid(pid:i32,status:*mut i32,opts:i32)->i32;fn clock_gettime(clock:i32,out:*mut Ts)->i32;}
#[repr(C)]struct Ts{sec:i64,ns:i64}
fn cpu()->f64{let mut t=Ts{sec:0,ns:0};assert_eq!(unsafe{clock_gettime(2,&mut t)},0);t.sec as f64+t.ns as f64*1e-9}
fn canonical(w:&Simulator)->Vec<u8>{let mut v=serde_json::to_value(w.checkpoint().unwrap()).unwrap();v.as_object_mut().unwrap().remove("producer");serde_json::to_vec(&v).unwrap()}
fn census(w:&Simulator)->serde_json::Value{
 let mut n:Vec<_>=w.snakes().iter().filter(|s|s.alive).map(|s|s.segments).collect();n.sort_unstable();
 let q=|p:f64|n[((n.len()-1) as f64*p).ceil() as usize];
 serde_json::json!({"living":n.len(),"mean":n.iter().sum::<usize>() as f64/n.len() as f64,"p90":q(0.9),"p99":q(0.99),"max":n.last(),"anchors":w.snakes().iter().filter(|s|s.alive).map(|s|s.body.len()).sum::<usize>(),"food":w.food().len(),"prey":w.prey().len()})
}
fn screened(v:&serde_json::Value)->bool{v["living"].as_u64().unwrap()>=490 && (55. ..=90.).contains(&v["mean"].as_f64().unwrap()) && (150..=230).contains(&v["p90"].as_u64().unwrap()) && (220..=350).contains(&v["p99"].as_u64().unwrap())}
fn record(out:&mut dyn Write,tag:u64,b:&[u8]){out.write_all(&tag.to_le_bytes()).unwrap();out.write_all(&(b.len() as u64).to_le_bytes()).unwrap();out.write_all(b).unwrap();}
fn profile(slot:usize)->PlayerAppearance{PlayerAppearance{nickname:format!("Local{slot}").into_bytes(),skin:(slot%9) as u8,..Default::default()}}
struct Fixture{sim:RawSimulator,policy:Forager,actions:Vec<Option<Action>>,legacy:bool,frames:u64,bytes:u64,rejoins:u64,refused:u64,minimum:usize}
impl Fixture{
 fn new(legacy:bool)->Self{
  let config=SimulationConfig{agents:SEATS,world:crate::Config::official(SEATS),delivery:super::DeliveryProfile::Official,..Default::default()};
  let mut sim=RawSimulator::new(7,config,SEATS).unwrap();sim.backend.world.grow_for_seats(SEATS,true);
  for slot in 0..SEATS{assert!(sim.connect_waited(slot,profile(slot),2.).unwrap());if legacy{sim.protocol13(slot);}}
  let actions=sim.backend.world.snakes().iter().map(|s|Some(Action{heading:s.heading,boost:false})).collect();
  Self{sim,policy:Forager::default(),actions,legacy,frames:0,bytes:0,rejoins:0,refused:0,minimum:SEATS}
 }
 fn tick(&mut self,measured:bool,mut out:Option<&mut dyn Write>){
  let tick=self.sim.backend.tick as usize;
  if tick>0 && tick<180_000 && tick%50==0{let slot=100+(tick/50-1)%400;self.sim.disconnect(slot).unwrap();self.actions[slot]=None;}
  for slot in 0..SEATS{if !self.sim.backend.world.snakes()[slot].alive{
   if let Ok((body,heading))=self.sim.backend.spawn_pose(slot){
    self.sim.backend.world.respawn_body(slot,body,heading).unwrap();self.sim.backend.slots[slot].living=None;
    self.sim.backend.slots[slot].life+=1;self.sim.backend.slots[slot].control=Action{heading,boost:false};
    self.sim.backend.connect(slot).unwrap();self.sim.backend.slots[slot].connection.decorations=true;
    if self.legacy{self.sim.protocol13(slot);}self.actions[slot]=Some(Action{heading,boost:false});self.rejoins+=1;
   }else{self.refused+=1;}
  }}
  if tick%6==0{self.actions=self.sim.backend.world.observe_all().iter().map(|o|o.as_ref().map(|o|self.policy.decide(o,&Timing::default(),&[]))).collect();}
  for (slot,action) in self.actions.iter().enumerate(){if let Some(action)=action{self.sim.backend.slots[slot].control=*action;}}
  if let Some(out)=out.as_deref_mut(){
   let actions:Vec<_>=self.actions.iter().map(|a|a.as_ref().map(|a|[a.heading.to_bits(),u64::from(a.boost)])).collect();record(out,1,&serde_json::to_vec(&(tick,actions)).unwrap());
  }
  let guard=measured.then(||crate::study_counter::enter(2));self.sim.advance_tick_with(0,&mut |_|Ok(())).unwrap();drop(guard);
  self.minimum=self.minimum.min(self.sim.backend.world.living());
  for slot in 0..SEATS{for(at,frame) in self.sim.take(slot).unwrap(){self.frames+=1;self.bytes+=frame.len() as u64;
   if let Some(out)=out.as_deref_mut(){let h=[tick as u64,slot as u64,at.to_bits()];let mut bytes=[0u8;24];for(i,v)in h.iter().enumerate(){bytes[i*8..(i+1)*8].copy_from_slice(&v.to_le_bytes());}record(out,2,&bytes);record(out,3,&frame);}
  }}
 }
 fn summary(&self)->serde_json::Value{serde_json::json!({"tick":self.sim.backend.tick,"census":census(&self.sim.backend.world),"frames":self.frames,"bytes":self.bytes,"rejoins":self.rejoins,"refused":self.refused,"minimum":self.minimum,"legacy":self.legacy,"connections":self.sim.backend.connections,"spawnRandom":format!("{:?}",self.sim.backend.spawn_random),"latencyRandom":format!("{:?}",self.sim.backend.random)})}
 fn window(&mut self,ticks:usize,path:&Path,trace:bool){
  assert_eq!(fs::read_dir("/proc/self/task").unwrap().count(),1);
  let mut file=trace.then(||io::BufWriter::new(fs::File::create(path.with_extension("trace")).unwrap()));
  if let Some(out)=&mut file{record(out,0,&canonical(&self.sim.backend.world));record(out,4,&serde_json::to_vec(&self.summary()).unwrap());}
  crate::study_counter::enable();
  for _ in 0..ticks{self.tick(true,file.as_mut().map(|f|f as &mut dyn Write));}
  crate::study_counter::disable();
  if let Some(out)=&mut file{record(out,0,&canonical(&self.sim.backend.world));record(out,4,&serde_json::to_vec(&self.summary()).unwrap());out.flush().unwrap();}
  fs::write(path,serde_json::to_vec(&serde_json::json!({"summary":self.summary(),"counts":crate::study_counter::result()})).unwrap()).unwrap();
  crate::study_counter::finish();
 }
}
pub fn run(legacy:bool,warm:usize,ticks:usize,trace:bool){
 assert_eq!(fs::read_dir("/proc/self/task").unwrap().count(),1);
 let mut f=Fixture::new(legacy);
 // Explicit controller amendment, matching the preceding core construction.
 let fresh=RawSimulator::new(7,f.sim.backend.config.clone(),SEATS).unwrap().backend.spawn_random;
 let start=cpu();let mut output=io::BufWriter::with_capacity(1<<20,io::stdout());
 if trace{record(&mut output,0,&canonical(&f.sim.backend.world));}
 while (f.sim.backend.tick as usize)<warm{
  if f.sim.backend.tick==180_000{f.sim.backend.spawn_random=fresh.clone();}
  f.tick(false,trace.then_some(&mut output as &mut dyn Write));
  if f.sim.backend.tick%1000==0{eprintln!("{}",serde_json::json!({"tick":f.sim.backend.tick,"cpuSeconds":cpu()-start,"census":census(&f.sim.backend.world)}));}
  assert!(cpu()-start<7200.,"bounded per-warm CPU budget exhausted");
 }
 if warm>=193_000{assert!(screened(&census(&f.sim.backend.world)),"mature length screen failed");}
 if trace{record(&mut output,0,&canonical(&f.sim.backend.world));record(&mut output,4,&serde_json::to_vec(&f.summary()).unwrap());}
 output.flush().unwrap();eprintln!("{}",serde_json::json!({"ready":true,"cpuSeconds":cpu()-start,"summary":f.summary(),"threads":fs::read_dir("/proc/self/task").unwrap().count()}));
 drop(output);
 for line in io::stdin().lock().lines(){
  let line=line.unwrap();let words:Vec<_>=line.split_whitespace().collect();if words[0]=="quit"{break;}
  assert_eq!(fs::read_dir("/proc/self/task").unwrap().count(),1);
  let path=Path::new(words[1]);assert!(!path.exists());let trace=words.get(2)==Some(&"trace");
  if words[0]=="parent"{f.window(ticks,path,trace);eprintln!("DONE {}",path.display());break;}
  assert_eq!(words[0],"child");let pid=unsafe{fork()};assert!(pid>=0);
  if pid==0{f.window(ticks,path,trace);std::process::exit(0);}
  let mut status=0;assert_eq!(unsafe{waitpid(pid,&mut status,0)},pid);assert_eq!(status,0);eprintln!("DONE {}",path.display());
 }
}
