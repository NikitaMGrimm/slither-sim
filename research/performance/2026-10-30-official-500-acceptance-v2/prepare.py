"""Apply v1's frozen instrumentation, then isolate generator blocking work."""
from pathlib import Path
r = Path(__file__).resolve().parent
exec(compile((r / 'sharedv1/prepare.py').read_text(), str(r / 'linked-v1-prepare.py'), 'exec'), {'__file__': str(r / 'linked-v1-prepare.py')})
p = r / 'server/source/examples/aws_clients.rs'
s = p.read_text()
def swap(old, new):
    global s
    assert s.count(old) == 1, (old, s.count(old))
    s = s.replace(old, new)
swap('struct Seat{', 'mod background;\nuse background::Background;\nenum Completion{Connected(usize,Result<Seat,String>,u128),Closed(usize,Result<(u64,u64),String>,u128)}\nstruct Seat{')
swap('socket:WebSocket<TcpStream>', 'socket:Option<WebSocket<TcpStream>>')
swap('let seat=Self{id,socket,state,', 'let seat=Self{id,socket:Some(socket),state,')
swap('impl Seat{', '''impl Seat{
 fn pending(id:usize)->Self{Self{id,socket:None,state:ClientState::new(),boost:None,due:f64::INFINITY,last_read:0.,frames:0,bytes:0,blocked:true,pending:Some(f64::INFINITY),server_ms:0,excess_min:f64::INFINITY}}
''')
# Only live seats have a socket; pending slots are excluded by the original poll/loop guards.
swap('s.socket.get_ref()', 's.socket.as_ref().unwrap().get_ref()')
for old in ['self.socket.close(', 'self.socket.read(', 'self.socket.write(', 's.socket.read(', 's.socket.flush(']:
    swap(old, old.replace('.socket.', '.socket.as_mut().unwrap().'))
swap('let mut observed_target=count;', '''let mut observed_target=count;
 let connector=Background::new(&format!("connect-{core}"));let publisher=Background::new(&format!("publish-{core}"));
 let (completed_tx,completed_rx)=std::sync::mpsc::channel::<Completion>();''')
# This acceptance has a fixed target, never a ladder. Drain completions before polling.
first=s.index('  if target!=observed_target{')
last=s.index('  let thin=', first)
s=s[:first]+'''  assert_eq!(target,observed_target,"acceptance population target must stay fixed");
  while let Ok(done)=completed_rx.try_recv(){match done{
   Completion::Connected(id,result,nanos)=>{let seat=seats.iter_mut().find(|s|s.id==id).unwrap();*seat=result?;joins+=1;
    let event=serde_json::json!({"worker":core,"seat":id,"kind":"connect","elapsedNanos":nanos,"unix":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()});publisher.submit(move||eprintln!("CLIENT_CONNECTION {}",event))?;},
   Completion::Closed(id,result,nanos)=>{let(f,b)=result?;frames+=f;bytes+=b;let seat=seats.iter_mut().find(|s|s.id==id).unwrap();seat.pending=Some(epoch.elapsed().as_secs_f64()+0.028);
    let event=serde_json::json!({"worker":core,"seat":id,"kind":"close","elapsedNanos":nanos,"unix":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()});publisher.submit(move||eprintln!("CLIENT_CONNECTION {}",event))?;}
  }}
'''+s[last:]
swap('if let Some(at)=s.pending{if epoch.elapsed().as_secs_f64()>=at{*s=Seat::connect(s.id,epoch,port)?;joins+=1;}continue;}', '''if let Some(at)=s.pending{if epoch.elapsed().as_secs_f64()>=at{let id=s.id;s.pending=Some(f64::INFINITY);let done=completed_tx.clone();connector.submit(move||{let began=Instant::now();let result=Seat::connect(id,epoch,port);done.send(Completion::Connected(id,result,began.elapsed().as_nanos())).unwrap();})?;}continue;}''')
swap('let(f,b)=s.close_clean(epoch)?;frames+=f;bytes+=b;s.state.disconnect();s.pending=Some(epoch.elapsed().as_secs_f64()+0.028);continue;', '''let id=s.id;let mut closing=std::mem::replace(s,Seat::pending(id));let done=completed_tx.clone();connector.submit(move||{let began=Instant::now();let result=closing.close_clean(epoch);done.send(Completion::Closed(id,result,began.elapsed().as_nanos())).unwrap();})?;continue;''')
# Move whole maps, not serialized copies, to the publisher. No large drop/format in the loop.
old='if observed_reset=="count"{eprintln!("CLIENT_WINDOW {}",serde_json::json!({"worker":core,"startUnix":metric_start,"endUnix":now,"gapsMicros":gaps.iter().map(|(k,v)|[*k,*v]).collect::<Vec<_>>(),"gapResidualMicros":residuals.iter().map(|(k,v)|[*k,*v as i64]).collect::<Vec<_>>(),"frameExcessMicros":excess.iter().map(|(k,v)|[*k,*v]).collect::<Vec<_>>()}));}'
new='''if observed_reset=="count"{let g=std::mem::take(&mut gaps);let r=std::mem::take(&mut residuals);let e=std::mem::take(&mut excess);let begin=metric_start;publisher.submit(move||eprintln!("CLIENT_WINDOW {}",serde_json::json!({"worker":core,"startUnix":begin,"endUnix":now,"gapsMicros":g.iter().map(|(k,v)|[*k,*v]).collect::<Vec<_>>(),"gapResidualMicros":r.iter().map(|(k,v)|[*k,*v as i64]).collect::<Vec<_>>(),"frameExcessMicros":e.iter().map(|(k,v)|[*k,*v]).collect::<Vec<_>>()})))?;}'''
swap(old,new)
first=s.index('   eprintln!("CLIENT {}",')
last=s.index('\n',first)
line=s[first:last]
assert line.endswith('));')
s=s[:first]+line.replace('eprintln!("CLIENT {}",','let event=').removesuffix(');')+';publisher.submit(move||eprintln!("CLIENT {}",event))?;'+s[last:]
p.write_text(s, newline='\n')
(p.parent/'background.rs').write_bytes((r/'background.rs').read_bytes())
# Bind the final source, not only the v1 intermediate.
import hashlib, json
pins=json.loads((r/'prepared.json').read_text())
pins['clientSourceSha256']=hashlib.sha256(p.read_bytes()).hexdigest()
h=hashlib.sha256()
for f in sorted((r/'server/source').rglob('*.rs')):
    h.update(f.relative_to(r/'server/source').as_posix().encode()+b'\0'+f.read_bytes()+b'\0')
pins['instrumentedRustAggregateSha256']=h.hexdigest()
(r/'prepared.json').write_text(json.dumps(pins,indent=2)+'\n')
