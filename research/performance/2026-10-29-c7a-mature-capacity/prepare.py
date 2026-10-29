"""Reuse #256 overlays and driver; add only remote/dynamic-count research hooks."""
from pathlib import Path
import sys,shutil,io,contextlib,json,hashlib
r=Path(__file__).resolve().parent;prior=r/'shared256';donor=r/'shared248'
for n in ['client.rs','timing.rs']:shutil.copy2(prior/n,r/n)
shutil.copy2(r/'main.tar',r/'server.tar')
original_argv=sys.argv[:];sys.argv=['build.py',str(donor),'server','--prepare-only']
try:
 with contextlib.redirect_stdout(io.StringIO()):
  try:exec(compile((prior/'build.py').read_text(),str(prior/'build.py'),'exec'),{'__file__':str(r/'bridge.py'),'__name__':'__main__'})
  except SystemExit as e:assert e.code==0
finally:sys.argv=original_argv
source=r/'server/source'
def swap(file,old,new):
 p=source/file;s=p.read_text();assert s.count(old)==1,(file,old,s.count(old));p.write_text(s.replace(old,new),encoding='utf-8',newline='\n')
# One additional peer classification per open session per one-second queue census.
swap('src/client/server.rs','let mut kernel=0;for session','let mut kernel=0;let mut external=0u64;for session')
swap('src/client/server.rs','queue+=open.queued_bytes as u64;','if matches!(open.phase,Phase::Playing(_)) && open.socket.get_ref().inner.peer_addr().is_ok_and(|a|match a.ip(){std::net::IpAddr::V4(ip)=>!ip.is_private()&&!ip.is_loopback(),std::net::IpAddr::V6(_)=>true}){external+=1;}queue+=open.queued_bytes as u64;')
swap('src/client/server.rs','crate::study_observe::socket_report(queue,kernel,players);','crate::study_observe::set_external(external);crate::study_observe::socket_report(queue,kernel,players);')
swap('src/study_observe.rs','"players":players,"rejects":','"players":players,"externalPlayers":EXTERNAL.load(Ordering::Relaxed),"rejects":')
with (source/'src/study_observe.rs').open('a') as f:f.write('\nstatic EXTERNAL:AtomicU64=AtomicU64::new(0);\npub fn set_external(n:u64){EXTERNAL.store(n,Ordering::Relaxed);}\n')
s=(prior/'client.rs').read_text()
def replace(old,new):
 global s
 assert s.count(old)==1,(old,s.count(old));s=s.replace(old,new)
replace('use std::{net::TcpStream,','use std::{sync::{Mutex,OnceLock},net::TcpStream,')
replace('struct Seat{','const SHARDS:usize=6;\nstatic SERVER:OnceLock<std::net::SocketAddr>=OnceLock::new();\nstatic CONNECT:Mutex<()>=Mutex::new(());\nstruct Seat{')
replace('  let tcp=TcpStream::connect((std::net::Ipv4Addr::LOCALHOST,port))','  let _admission=CONNECT.lock().unwrap();let _=port;\n  let tcp=TcpStream::connect(*SERVER.get().unwrap())')
replace('format!("ws://127.0.0.1:{port}/slither")','format!("ws://{}/slither",SERVER.get().unwrap())')
replace('  Ok(Self{id,socket,state,boost:None,due:at+0.042*(id%16) as f64/16.,last_read:at,frames:0,bytes:0,blocked:false,pending:None})','  let seat=Self{id,socket,state,boost:None,due:at+0.042*(id%16) as f64/16.,last_read:at,frames:0,bytes:0,blocked:false,pending:None};thread::sleep(Duration::from_millis(3));Ok(seat)')
replace(' pin(core);let mut policy=',' pin(core-1);let mut policy=')
replace('(core-1..count).step_by(3)','(core-1..count).step_by(SHARDS)')
replace(' while !root.join("STOP").exists(){',' let mut observed_target=count;\n while !root.join("STOP").exists(){')
replace('  let pass=Instant::now();let thin=', '''  let pass=Instant::now();
  let target=std::fs::read_to_string(root.join("target")).map_err(|e|e.to_string())?.trim().parse::<usize>().map_err(|e|e.to_string())?;
  if target!=observed_target{let mut remove=Vec::new();
  for (i,seat) in seats.iter_mut().enumerate(){if seat.id>=target{if seat.pending.is_none(){let(f,b)=seat.close_clean(epoch)?;frames+=f;bytes+=b;}remove.push(i);}}
  for i in remove.into_iter().rev(){seats.remove(i);}
  for id in (core-1..target).step_by(SHARDS){if !seats.iter().any(|s|s.id==id){seats.push(Seat::connect(id,epoch,port)?);joins+=1;}}observed_target=target;}
  let thin=''')
replace('  let churn=(tick.min(179999))/50;','  let residents=count/5;let rotating=count-residents;\n  let churn=tick.min(179999)*rotating as u64/20000;')
replace('s.id>=100 && (previous_churn+1..=churn).any(|n|100+((n-1)%400) as usize==s.id)','s.id>=residents && s.id<count && (previous_churn+1..=churn).any(|n|residents+((n-1)%rotating as u64) as usize==s.id)')
replace('"seats":seats.len(),','"seats":seats.len(),"target":target,"connected":seats.iter().filter(|s|s.pending.is_none()).count(),')
replace('let epoch=Instant::now();pin(3);let workers:Vec<_>=(1..=3)', 'let ip=a[4].parse::<std::net::Ipv4Addr>().unwrap();assert!(ip.is_private());SERVER.set(std::net::SocketAddr::new(ip.into(),port)).unwrap();let epoch=Instant::now();pin(7);let workers:Vec<_>=(1..=SHARDS)')
(source/'examples/aws_clients.rs').write_text(s,encoding='utf-8',newline='\n')
sha=lambda p:hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
(r/'prepared.json').write_text(json.dumps({'runtime':'692cd561714b8af3ea95adb3adcbd807b3a48734','archiveSha256':sha(r/'main.tar'),'priorHarnessSha256':sha(r/'shared256/build.py'),'sourceOverlays':{str(p.relative_to(source)):sha(p) for p in (source/'src').rglob('*.rs')},'clientSourceSha256':sha(source/'examples/aws_clients.rs'),'cpuTarget':'x86-64'},indent=2)+'\n')
print('PORTABLE OVERLAYS PREPARED')
