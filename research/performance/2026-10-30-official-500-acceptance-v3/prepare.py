"""Reuse executed v2; change only generator per-worker CPU accounting."""
from pathlib import Path
import hashlib,json
r=Path(__file__).resolve().parent
exec(compile((r/'sharedv2/prepare.py').read_text(),str(r/'linked-v2-prepare.py'),'exec'),{'__file__':str(r/'linked-v2-prepare.py')})
root=r/'server/source';client=root/'examples/aws_clients.rs';s=client.read_text()
def swap(old,new):
    global s
    assert s.count(old)==1,(old,s.count(old))
    s=s.replace(old,new)
swap('mod background;','mod background;\nmod worker_cpu;')
swap('let mut prior_cpu=cpu();','let mut prior_cpu;')
swap(' let (completed_tx,completed_rx)=', ''' let worker_cpu=worker_cpu::WorkerCpu::new(worker_cpu::Clock::current(),connector.cpu_clock(),publisher.cpu_clock());
 let mut prior_parts=worker_cpu.snapshot();prior_cpu=prior_parts.total();
 let (completed_tx,completed_rx)=''')
swap('let now=cpu();let seconds=', 'let parts=worker_cpu.snapshot();let now=parts.total();let seconds=')
swap('"cpuFraction":(now-prior_cpu) as f64/1e9/seconds,', '''"cpuFraction":(now-prior_cpu) as f64/1e9/seconds,"decisionCpuFraction":(parts.decision-prior_parts.decision) as f64/1e9/seconds,"connectorCpuFraction":(parts.connector-prior_parts.connector) as f64/1e9/seconds,"publisherCpuFraction":(parts.publisher-prior_parts.publisher) as f64/1e9/seconds,''')
swap('prior_cpu=now;prior_wall=', 'prior_cpu=now;prior_parts=parts;prior_wall=')
client.write_text(s,newline='\n')
background=root/'examples/background.rs';text=background.read_text()
assert text.count('    queued: Arc<AtomicUsize>,')==1 and text.count('        std::thread::Builder::new()')==1
text=text.replace('    queued: Arc<AtomicUsize>,','    queued: Arc<AtomicUsize>,\n    thread: std::thread::JoinHandle<()>,')
text=text.replace('        std::thread::Builder::new()','        let thread=std::thread::Builder::new()')
assert text.count('Self { sender, queued }')==1
text=text.replace('Self { sender, queued }','Self { sender, queued, thread }')
text+='\nimpl Background { pub fn cpu_clock(&self)->crate::worker_cpu::Clock { crate::worker_cpu::Clock::for_handle(&self.thread) } }\n'
background.write_text(text,newline='\n')
(root/'examples/worker_cpu.rs').write_bytes((r/'worker_cpu.rs').read_bytes())
pins=json.loads((r/'prepared.json').read_text());pins['clientSourceSha256']=hashlib.sha256(client.read_bytes()).hexdigest()
h=hashlib.sha256()
for f in sorted(root.rglob('*.rs')):h.update(f.relative_to(root).as_posix().encode()+b'\0'+f.read_bytes()+b'\0')
pins['instrumentedRustAggregateSha256']=h.hexdigest()
(r/'prepared.json').write_text(json.dumps(pins,indent=2)+'\n')
