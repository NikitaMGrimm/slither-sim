"""Real loopback host, all server threads on one CPU; client on another.

run.py BIN_ROOT NEW_OUTPUT --bots 2280 --server-cpu 0 --client-cpu 1
No builds or outside network traffic. Raw process command lines stay local-only.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time

def proc(pid,tgid=None):
    root=Path('/proc')/str(pid) if tgid is None else Path('/proc')/str(tgid)/'task'/str(pid)
    try:
        stat=(root/'stat').read_text().rsplit(')',1)[1].split()
        status=dict(line.split(':',1) for line in (root/'status').read_text().splitlines() if ':' in line)
        hz=os.sysconf('SC_CLK_TCK')
        return {'pid':pid,'start':stat[19],'user':int(stat[11])/hz,'system':int(stat[12])/hz,
                'comm':(root/'comm').read_text().strip(),'cpus':sorted(os.sched_getaffinity(pid)),
                'rssKiB':int(status.get('VmRSS','0 kB').split()[0]),
                'peakKiB':int(status.get('VmHWM','0 kB').split()[0])}
    except (OSError,ProcessLookupError):return None

def snapshot(pid):
    return {'unix':time.time(),'process':proc(pid),
            'systemCpu':Path('/proc/stat').read_text().splitlines()[0],
            'threads':[row for task in Path(f'/proc/{pid}/task').glob('*') if (row:=proc(int(task.name),pid)) is not None]}

class Attached:
    """Attach a corrected collector without restarting the running game."""
    def __init__(self,pid):self.pid=pid;self.returncode=None
    def poll(self):
        try:os.kill(self.pid,0)
        except ProcessLookupError:self.returncode=0
        return self.returncode
    def wait(self,timeout):
        deadline=time.monotonic()+timeout
        while self.poll() is None:
            if time.monotonic()>deadline:raise subprocess.TimeoutExpired('attached process',timeout)
            time.sleep(.05)

def processes(out,label):
    command=['ps','-eo','pid,ppid,lstart,pcpu,pmem,time,args','--sort=-pcpu','-ww']
    raw=subprocess.check_output(command)
    path=out/f'processes-{label}.txt';path.write_bytes(raw)
    # /proc CPU counters give interval activity, rather than lifetime ps averages.
    rows=[row for task in Path('/proc').glob('[0-9]*') if (row:=proc(int(task.name))) is not None]
    return {'sha256':hashlib.sha256(raw).hexdigest(),'rows':rows}

def main():
    if not __debug__:raise SystemExit('Run without Python optimization')
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('binaries',type=Path);p.add_argument('output',type=Path)
    p.add_argument('--bots',type=int,required=True)
    p.add_argument('--server-cpu',type=int,default=0);p.add_argument('--client-cpu',type=int,default=1)
    p.add_argument('--hold',type=float,default=300);p.add_argument('--holds',type=int,default=3)
    p.add_argument('--max-warm-seconds',type=float,default=5400)
    p.add_argument('--min-ticks',type=int,default=30000)
    p.add_argument('--profile-seconds',type=float,default=60)
    p.add_argument('--arm',choices=['plain','observe','profile'],default='profile')
    p.add_argument('--warm-cpus',help='Optional comma-separated warmup CPUs; holds always use server-cpu')
    p.add_argument('--attach',action='store_true',help='Resume collection during warmup; retain host/client processes and logs')
    a=p.parse_args();os.sched_setaffinity(0,{a.client_cpu})
    assert a.server_cpu!=a.client_cpu, 'client must use a different CPU'
    warm_cpus=sorted({int(cpu) for cpu in a.warm_cpus.split(',')}) if a.warm_cpus else [a.server_cpu]
    assert a.server_cpu in warm_cpus and a.client_cpu not in warm_cpus
    out=a.output.resolve()
    if not a.attach:out.mkdir(exist_ok=False)
    loopback=__import__('socket').gethostbyname('localhost')
    config={'bind':f'{loopback}:18766','seed':7,'players':{'max':1},'bots':{'count':a.bots,'threads':2},
            'simulation':{'world':{'preset':'official'},'delivery':'official'}}
    (out/'config.json').write_text(json.dumps(config,indent=2)+'\n')
    marker=out/'PROFILE';env=dict(os.environ,RAYON_NUM_THREADS='1',CAPACITY_PROFILE=str(marker))
    binaries=a.binaries.resolve()
    meta={'bots':a.bots,'seats':a.bots+1,'serverCpu':a.server_cpu,'clientCpu':a.client_cpu,
          'arm':a.arm,'binarySha256':hashlib.file_digest((binaries/a.arm/'slither-sim').open('rb'),'sha256').hexdigest(),
          'warmBlocks':[],'holds':[],'status':'running'}
    if a.attach:
        previous=json.loads((out/'summary.json').read_text())
        assert previous['status']=='running' and not previous['holds'] and 'screen' not in previous
        assert previous['binarySha256']==meta['binarySha256'] and previous['bots']==a.bots
        meta.update(serverPid=previous['serverPid'],clientPid=previous['clientPid'],collectorAttached=True)
    meta['warmCpus']=warm_cpus
    def save(): (out/'summary.json').write_text(json.dumps(meta,indent=2)+'\n')
    save()
    server=client=None
    mode='a' if a.attach else 'w'
    with (out/'host.log').open(mode) as log,(out/'client.log').open(mode) as client_log,(out/'telemetry.jsonl').open(mode) as telemetry:
        if a.attach:
            server=Attached(meta['serverPid'])
            client=Attached(meta['clientPid'])
        else:
            server=subprocess.Popen(['taskset','-c',str(a.server_cpu),str(binaries/a.arm/'slither-sim'),
                 'host','--config',str(out/'config.json'),'--stats','5'],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        meta['serverPid']=server.pid;save()
        def pin(cpus,label):
            start=time.time()
            receipt=subprocess.run(['taskset','-a','-p','-c',','.join(map(str,cpus)),str(server.pid)],
                                   capture_output=True,text=True,check=True)
            observed=snapshot(server.pid)
            assert all(row['cpus']==cpus for row in observed['threads'])
            (out/f'affinity-{label}.txt').write_text(receipt.stdout+receipt.stderr)
            meta[label]={'startUnix':start,'endUnix':time.time(),'server':observed,
                         'processes':processes(out,'affinity-'+label)}
            save()
        pin(warm_cpus,'warmPin')
        begin=time.monotonic();last_sample=0;censuses=[];offset=0;hold=None;profile_end=None
        repinned=False;hold_not_before=0
        if a.attach:
            first=json.loads((out/'telemetry.jsonl').open().readline())
            begin-=time.time()-first['server']['unix']
        try:
            while True:
                now=time.monotonic()
                if server.poll() is not None:raise RuntimeError(f'host exited {server.returncode}')
                if client is None and now-begin>2:
                    client=subprocess.Popen(['taskset','-c',str(a.client_cpu),str(binaries/'plain/portable_bot'),
                         '--url',f'ws://{loopback}:18766/slither','--steps','1000000000','--lives','1000000000'],
                         env=env,stdout=client_log,stderr=subprocess.STDOUT,start_new_session=True)
                    meta['clientPid']=client.pid;save()
                if client is not None and client.poll() is not None:raise RuntimeError(f'client exited {client.returncode}')
                if now-last_sample>=1:
                    sample={'server':snapshot(server.pid),'client':snapshot(client.pid) if client else None}
                    expected=[a.server_cpu] if repinned else warm_cpus
                    assert all(row['cpus']==expected for row in sample['server']['threads'])
                    telemetry.write(json.dumps(sample)+'\n');telemetry.flush();last_sample=now
                # Read only complete lines while the host appends.
                with (out/'host.log').open() as reader:
                    reader.seek(offset)
                    while True:
                        position=reader.tell();line=reader.readline()
                        if not line.endswith('\n'):offset=position;break
                        offset=reader.tell()
                        if line.startswith('CENSUS '):
                            row=json.loads(line[7:]);censuses.append(row)
                            if row['tick']%5000==0:
                                block={k:v for k,v in row.items() if k!='tickNanos'}
                                meta['warmBlocks'].append(block);save()
                                print('BLOCK',a.bots,block,flush=True)
                if hold is None and not meta['holds']:
                    blocks=meta['warmBlocks']
                    settled=False
                    if len(blocks)>=3 and blocks[-1]['tick']>=a.min_ticks:
                        tail=blocks[-3:]
                        ratios={key:max(b[key] for b in tail)/max(1e-30,min(b[key] for b in tail)) for key in ['meanSegments','p90Segments']}
                        settled=all(v<=1.05 for v in ratios.values())
                    if settled:
                        meta['screen']={'settled':True,'ratios':ratios,'lastTick':blocks[-1]['tick'],'wallSeconds':now-begin}
                    elif now-begin>=a.max_warm_seconds:
                        meta['screen']={'settled':False,'lastTick':censuses[-1]['tick'] if censuses else 0,'wallSeconds':now-begin}
                    else:
                        time.sleep(.1);continue
                if not repinned:
                    pin([a.server_cpu],'holdPin')
                    repinned=True
                    # Give queues/caches one minute on the measured CPU before holds.
                    hold_not_before=now+60
                if now<hold_not_before:
                    time.sleep(.1);continue
                if hold is None and len(meta['holds'])<a.holds:
                    label=str(len(meta['holds']))
                    hold={'start':snapshot(server.pid),'clientStart':snapshot(client.pid),
                          'processStart':processes(out,label+'-start'),'censusIndex':len(censuses)}
                    hold_end=now+a.hold
                    print('HOLD_START',a.bots,label,flush=True)
                if hold is not None and now>=hold_end:
                    label=str(len(meta['holds']))
                    hold.update(end=snapshot(server.pid),clientEnd=snapshot(client.pid),
                                processEnd=processes(out,label+'-end'),censusEndIndex=len(censuses))
                    meta['holds'].append(hold);hold=None;save()
                    print('HOLD_END',a.bots,label,flush=True)
                if len(meta['holds'])==a.holds and hold is None:
                    if profile_end is None:
                        marker.touch();profile_end=now+a.profile_seconds
                        meta['profileStart']=snapshot(server.pid);save();print('PROFILE_START',flush=True)
                    if now>=profile_end:
                        meta['profileEnd']=snapshot(server.pid);break
                time.sleep(.05)
            meta['status']='complete'
        except BaseException as error:
            meta['status']='failed';meta['error']=repr(error)
            raise
        finally:
            meta['lastServer']=snapshot(server.pid);save()
            for child in [client,server]:
                if child is not None and child.poll() is None:
                    os.killpg(child.pid,signal.SIGTERM)
                    try:child.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid,signal.SIGKILL)
                        if not isinstance(child,Attached):child.wait()

if __name__=='__main__':main()
