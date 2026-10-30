"""Read-only acceptance deltas on the linked #258/#256 portable harness."""
from pathlib import Path
import hashlib,json,runpy,tarfile
r=Path(__file__).resolve().parent
runpy.run_path(str(r/'prepare258.py'))
source=r/'server/source';client=source/'examples/aws_clients.rs'
text=client.read_text()
def swap(old,new):
 global text
 assert text.count(old)==1,(old,text.count(old));text=text.replace(old,new)
swap('pending:Option<f64>}', 'pending:Option<f64>,server_ms:u64,excess_min:f64}')
swap('blocked:false,pending:None}', 'blocked:false,pending:None,server_ms:0,excess_min:f64::INFINITY}')
swap('let mut observed_target=count;', '''let mut observed_target=count;let mut observed_reset=String::new();let mut metric_start=0.;
 let mut gaps=std::collections::BTreeMap::<u64,u64>::new();let mut residuals=std::collections::BTreeMap::<i64,u64>::new();let mut excess=std::collections::BTreeMap::<u64,u64>::new();''')
swap('  if target!=observed_target{', '''  let reset=std::fs::read_to_string(root.join("RESET")).unwrap_or_default();
  if reset!=observed_reset{
   let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
   if observed_reset=="count"{eprintln!("CLIENT_WINDOW {}",serde_json::json!({"worker":core,"startUnix":metric_start,"endUnix":now,"gapsMicros":gaps.iter().map(|(k,v)|[*k,*v]).collect::<Vec<_>>(),"gapResidualMicros":residuals.iter().map(|(k,v)|[*k,*v as i64]).collect::<Vec<_>>(),"frameExcessMicros":excess.iter().map(|(k,v)|[*k,*v]).collect::<Vec<_>>()}));}
   gaps.clear();residuals.clear();excess.clear();for seat in &mut seats{seat.excess_min=f64::INFINITY;}metric_start=now;observed_reset=reset;
  }
  if target!=observed_target{''')
swap('let at=epoch.elapsed().as_secs_f64();s.state.receive_frame(&data,at)?;s.last_read=at;', '''let at=epoch.elapsed().as_secs_f64();let was=s.state.status()==ClientStatus::Playing;s.state.receive_frame(&data,at)?;
     let header_ms=u16::from_be_bytes([data[0],data[1]]) as u64;s.server_ms+=header_ms;let residual=at-s.server_ms as f64/1000.;s.excess_min=s.excess_min.min(residual);
     if observed_reset=="count"&&was&&s.state.status()==ClientStatus::Playing{*gaps.entry(((at-s.last_read).max(0.)*1e6) as u64).or_default()+=1;*residuals.entry(((at-s.last_read)*1e6) as i64-(header_ms*1000) as i64).or_default()+=1;*excess.entry(((residual-s.excess_min).max(0.)*1e6) as u64).or_default()+=1;}s.last_read=at;''')
client.write_text(text,newline='\n')
with tarfile.open(r/'main.tar') as archive:runtime=archive.pax_headers['comment']
assert runtime==json.loads((r/'input.json').read_text())['runtime']
p=r/'prepared.json';pins=json.loads(p.read_text());pins['runtime']=runtime
pins['clientSourceSha256']=hashlib.sha256(client.read_bytes()).hexdigest()
h=hashlib.sha256()
for f in sorted(source.rglob('*.rs')):h.update(f.relative_to(source).as_posix().encode()+b'\0'+f.read_bytes()+b'\0')
pins['instrumentedRustAggregateSha256']=h.hexdigest();pins.pop('sourceOverlays',None)
p.write_text(json.dumps(pins,indent=2)+'\n');print('ACCEPTANCE OVERLAYS PREPARED',flush=True)
