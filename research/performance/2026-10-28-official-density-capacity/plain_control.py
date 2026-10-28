"""Supplementary pristine-CLI cold-start check after both primary cases.

plain_control.py BENCH_ROOT
Not a settled-length hold or a matched estimate of instrumentation overhead.
"""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import run as collector

def main(root):
    os.sched_setaffinity(0,{1});deadline=time.monotonic()+27000
    for name in ['target-2281','reference-500']:
        while True:
            path=root/name/'summary.json'
            if path.exists():
                try:meta=json.loads(path.read_text())
                except json.JSONDecodeError:time.sleep(.05);continue
                if meta['status']=='complete':break
                if meta['status']=='failed':raise RuntimeError('Primary failed; do not replace it with the cold check')
            if time.monotonic()>deadline:raise TimeoutError('Primary collection deadline')
            time.sleep(5)
    # A complete metadata write precedes the primary collector's teardown.
    until=time.monotonic()+20
    while collector.proc(meta['serverPid']) is not None:
        if time.monotonic()>until:raise RuntimeError('Primary server has not exited')
        time.sleep(.1)
    out=root/'plain-control';out.mkdir(exist_ok=False)
    config=json.loads((root/'target-2281/config.json').read_text())
    (out/'config.json').write_text(json.dumps(config)+'\n')
    binaries=root/'bin/plain';server=client=None
    with (out/'host.log').open('w') as host_log,(out/'client.log').open('w') as client_log:
        try:
            server=subprocess.Popen(['taskset','-c','0',str(binaries/'slither-sim'),'host','--config',str(out/'config.json'),'--stats','5'],stdout=host_log,stderr=subprocess.STDOUT,start_new_session=True)
            time.sleep(2)
            client=subprocess.Popen(['taskset','-c','1',str(binaries/'portable_bot'),'--url','ws://'+config['bind']+'/slither','--steps','1000000000','--lives','1000000000'],stdout=client_log,stderr=subprocess.STDOUT,start_new_session=True)
            time.sleep(28)
            result={'status':'running','warmupSeconds':30,'holdSeconds':300,
                    'serverStart':collector.snapshot(server.pid),'clientStart':collector.snapshot(client.pid),
                    'processStart':collector.processes(out,'start'),'stats':[]}
            end=time.monotonic()+300;offset=(out/'host.log').stat().st_size
            while time.monotonic()<end:
                assert server.poll() is None and client.poll() is None
                assert all(row['cpus']==[0] for row in collector.snapshot(server.pid)['threads'])
                with (out/'host.log').open() as log:
                    log.seek(offset)
                    while True:
                        position=log.tell();line=log.readline()
                        if not line.endswith('\n'):offset=position;break
                        offset=log.tell()
                        if line.startswith('stats:'):result['stats'].append({'readUnix':time.time(),'line':line.strip()})
                time.sleep(.1)
            result.update(status='complete',serverEnd=collector.snapshot(server.pid),clientEnd=collector.snapshot(client.pid),
                          processEnd=collector.processes(out,'end'))
            (out/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
            print('PRISTINE_COLD_CONTROL_COMPLETE',flush=True)
        finally:
            for child in [client,server]:
                if child is not None and child.poll() is None:
                    os.killpg(child.pid,signal.SIGTERM)
                    try:child.wait(timeout=10)
                    except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()

if __name__=='__main__':main(Path(sys.argv[1]).resolve())
