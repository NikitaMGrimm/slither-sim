"""Validate an explicit core candidate archive in an isolated source tree.

python3 check.py REF NEW_OUTPUT [--cpu 2] [--target TASK_OWNED_POSITIVE_TARGET]
Never reuse an active target or a mutation target. A supplied positive target
is rebuilt from freshly touched sources, avoiding older archive timestamps.
"""
import argparse
import os
from pathlib import Path
import subprocess
import tarfile
if not __debug__:
    raise SystemExit('Run without Python optimization')
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('ref');p.add_argument('output',type=Path);p.add_argument('--cpu',type=int,default=2);p.add_argument('--target',type=Path)
a=p.parse_args();os.sched_setaffinity(0,{a.cpu})
root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
ref=subprocess.check_output(['git','rev-parse',a.ref],text=True).strip()
archive=root/'source.tar';subprocess.run(['git','archive','-o',str(archive),ref],check=True)
source=root/'source';source.mkdir()
with tarfile.open(archive) as f:f.extractall(source,filter='data')
for path in source.rglob('*'):
    if path.is_file():os.utime(path,None)
env=dict(os.environ,CARGO_BUILD_JOBS='1',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTUP_TOOLCHAIN='1.99.0',CARGO_TARGET_DIR=str(a.target.resolve() if a.target else root/'target'))
for name,command in [('focused',['cargo','test','--locked','--lib','food']),('pickup',['cargo','test','--locked','--test','circle_pickup','--test','pickup_events']),('rust',['python3','tools/check.py','rust'])]:
    with (root/(name+'.log')).open('w') as log:subprocess.run(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
print('CORE CHECKS PASS',ref,flush=True)
