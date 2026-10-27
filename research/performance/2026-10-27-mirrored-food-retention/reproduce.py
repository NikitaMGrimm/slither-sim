"""Reproduce standard/shrinking trace, instruction and tick-distribution evidence.

python3 reproduce.py {build,trace,count,ticks} REPOSITORY NEW_OUTPUT --perf PERF
Linux x86-64, Python 3.12+, Rust 1.99.0, patch and user PMU access required.
Optional RETENTION_TARGET reuses task-owned native-{base,candidate}/target caches.
"""
import argparse
import os
from pathlib import Path
import subprocess
import sys
if not __debug__:
    raise SystemExit('Run without optimization')
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('mode',choices=['build','trace','count','ticks']);p.add_argument('repository',type=Path);p.add_argument('output',type=Path)
p.add_argument('--base',default='339041fec0ebde58e5da0b544a6ab2cc6c720850');p.add_argument('--candidate',default='3e22127756da910488ac3f4dbb998806d87a749d')
p.add_argument('--cpu',type=int,default=3);p.add_argument('--compression-cpu',type=int,default=2);p.add_argument('--perf');a=p.parse_args()
repo,out=a.repository.resolve(),a.output.resolve();study=Path(__file__).resolve().parent
os.sched_setaffinity(0,{a.cpu})
if a.mode=='build':
    subprocess.run([sys.executable,str(study/'build.py'),str(repo),str(out),a.base,a.candidate],env=dict(os.environ,RETENTION_CPU=str(a.cpu)),check=True)
else:
    if a.mode in ['trace','count']:
        command=[sys.executable,str(out.with_name(out.name+'-fixture')/'reproduce.py'),a.mode,str(repo),str(out),'--cpu',str(a.cpu)]
        if a.mode=='count':
            if not a.perf:raise SystemExit('count requires --perf')
            command+=['--perf',a.perf]
        subprocess.run(command,check=True)
    command=[sys.executable,str(study/'shrink.py'),a.mode,str(out),'--cpu',str(a.cpu),'--compression-cpu',str(a.compression_cpu)]
    if a.perf:command+=['--perf',a.perf]
    subprocess.run(command,check=True)
