"""Bounded 50-client loopback smoke test of the exact study harness."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import subprocess
import time

def main(binary, output):
    output.mkdir(parents=True, exist_ok=False)
    suffix = '.exe' if os.name == 'nt' else ''
    config = output/'config.json'
    config.write_text(json.dumps({'bind':'127.0.0.1:8766','seed':7,'maxClients':50,'bots':0,'botThreads':0})+'\n')
    epoch = time.time()+3
    children, logs = [], []
    def start(command, env=None):
        log = open(output/f'child-{len(children)}.log','w'); logs.append(log)
        child = subprocess.Popen(command,stdin=subprocess.DEVNULL,stdout=log,stderr=log,env=env)
        children.append(child)
        if os.name == 'nt':
            if not ctypes.windll.kernel32.SetProcessAffinityMask(int(child._handle),15):
                raise RuntimeError('Could not limit smoke to four logical CPUs')
        else:
            os.sched_setaffinity(child.pid,sorted(os.sched_getaffinity(0))[:4])
    try:
        start([str(binary/('host_probe'+suffix)),str(config),'3','8',str(output/'host.json'),str(epoch)])
        time.sleep(1)
        for i in range(2):
            start([str(binary/('host_capacity_load'+suffix)),'25','8766','8','3',str(output/f'load-{i}.json'),str(i*25)],dict(os.environ,STUDY_EPOCH_UNIX_S=str(epoch)))
        for child in children:
            if child.wait(timeout=40): raise RuntimeError('Smoke child failed')
        for i in range(2):
            data=json.loads((output/f'load-{i}.json').read_text())
            assert data['clients']['timeWeightedMean']['playing']==25
            assert not data['errors']
            assert data['traffic']['frames']>1000
        rows=json.loads((output/'host.json').read_text())['rows']
        assert next(r for r in rows if r['boundary']=='end')['deferred']==0
        print('PASS: 50/50 playing, every frame decoded, zero errors, zero deferred ticks')
    finally:
        for child in children:
            if child.poll() is None: child.terminate(); child.wait(timeout=10)
        for log in logs: log.close()

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary',type=Path); parser.add_argument('output',type=Path)
    args=parser.parse_args(); main(args.binary.resolve(),args.output.resolve())
