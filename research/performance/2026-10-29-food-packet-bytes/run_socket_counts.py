"""Finite own-child ptrace diagnostic with eight real loopback recipients."""
from pathlib import Path
import csv,hashlib,json,math,os,signal,subprocess,time
r=Path(__file__).resolve().parent
out=r/'eight-stock';out.mkdir()
client=Path('/var/tmp/slither-capacity-checkpoint-20261028/common-client/bin/clients')
assert client.is_file()
config={'bind':'127.0.0.1:18768','seed':7,'players':{'max':8},'bots':{'count':0,'threads':1},'simulation':{'world':{'preset':'official'},'delivery':'official'}}
(out/'config.json').write_text(json.dumps(config)+'\n');(out/'tick').write_text('0\n')
inventory=lambda label:subprocess.run(['python3',str(r/'inventory.py'),str(out),label],check=True)
server=clients=None
os.sched_setaffinity(0,{3})
inventory('before')
with (out/'host.log').open('x') as hl,(out/'client.log').open('x') as cl:
    try:
        server=subprocess.Popen([str(r/'counter'),str(out/'syscalls.csv'),'taskset','-c','0',str(r/'host'),'host','--config',str(out/'config.json'),'--stats','5'],stdout=hl,stderr=subprocess.STDOUT,start_new_session=True)
        time.sleep(1)
        assert server.poll() is None
        clients=subprocess.Popen(['taskset','-c','1-3',str(client),str(out),'18768','8'],stdout=cl,stderr=subprocess.STDOUT,start_new_session=True)
        deadline=time.monotonic()+45
        while True:
            assert server.poll() is None,'host/counter failed'
            assert clients.poll() is None,'clients failed'
            rows=list(csv.DictReader((out/'syscalls.csv').open()))
            if rows and int(rows[-1]['tick'])>=1000:break
            assert time.monotonic()<deadline,'bounded diagnostic timeout'
            time.sleep(.2)
    finally:
        (out/'STOP').touch()
        if clients is not None:
            try:clients.wait(timeout=10)
            except subprocess.TimeoutExpired:os.killpg(clients.pid,signal.SIGTERM);clients.wait(timeout=5)
        if server is not None:
            # Signal the traced host, not its tracer, so the tracer can reap it.
            children=(Path('/proc')/str(server.pid)/'task'/str(server.pid)/'children').read_text().split()
            for child in children:
                try:os.kill(int(child),signal.SIGTERM)
                except ProcessLookupError:pass
            server.wait(timeout=10)
inventory('after')
assert server.returncode==0 and clients.returncode==0
assert 'panicked at' not in (out/'host.log').read_text()
reports=[json.loads(line[7:]) for line in (out/'client.log').read_text().splitlines() if line.startswith('CLIENT ')]
latest={x['worker']:x for x in reports}
assert set(latest)=={1,2,3} and sum(x['playing'] for x in latest.values())==8
assert all(x['admissionRefusals']==0 for x in reports)
rows=[{k:int(v) for k,v in row.items()} for row in csv.DictReader((out/'syscalls.csv').open())]
rows=[row for row in rows if 301<=row['tick']<=1000]
assert len(rows)==700 and [x['tick'] for x in rows]==list(range(301,1001))
assert all(x['write']==x['writev']==x['sendmsg']==x['sends_outside_tcp_write']==x['other_calls']==0 for x in rows)
result={'recipients':8,'protocol':19,'tickRange':[301,1000],'limits':'Short world, ptrace and markers perturb scheduling; counts only, no CPU/tail/capacity claim. Completed call brackets validate the one-sendto mapping independently of tick-boundary crossings.','phases':{}}
for phase,selected in [('all',rows),('maintenance',[x for x in rows if math.floor(x['tick']*.007/.504)>math.floor((x['tick']-1)*.007/.504)]),('ordinary',[x for x in rows if math.floor(x['tick']*.007/.504)==math.floor((x['tick']-1)*.007/.504)])]:
    totals={k:sum(x[k] for x in selected) for k in selected[0] if k!='tick'}
    result['phases'][phase]={'ticks':len(selected),'totals':totals,'perRecipientTick':{k:v/8/len(selected) for k,v in totals.items()}}
result['clientSha256']=hashlib.file_digest(client.open('rb'),'sha256').hexdigest()
result['lastClientReports']=latest
(out/'selected.json').write_text(json.dumps(result,indent=2)+'\n')
print('SYSCALL DIAGNOSTIC COMPLETE',json.dumps(result),flush=True)
