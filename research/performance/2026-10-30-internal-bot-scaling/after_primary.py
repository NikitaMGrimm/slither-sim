"""Wait without polling, then run separately planned post-count work.

Linux pidfd wait keeps this controller asleep throughout primary timing.
Retain stdout/stderr independently of an SSH connection.
"""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time
from run import digest, emit
from reduce import population_directory


def execute(root,study):
    root=Path(root).resolve();study=Path(study).resolve()
    os.sched_setaffinity(0,{3})
    (root/'after-primary.pid').write_text(str(os.getpid())+'\n')
    candidates=[]
    for path in Path('/proc').glob('[0-9]*/cmdline'):
        try:
            args=path.read_bytes().split(b'\0')
            if any(arg.rsplit(b'/',1)[-1] in [b'run.py',b'restart_primary.py'] for arg in args) and str(root).encode() in args:
                candidates.append(int(path.parent.name))
        except (FileNotFoundError,PermissionError,ProcessLookupError):pass
    if candidates:
        assert len(candidates)==1
        try:
            descriptor=os.pidfd_open(candidates[0])
            wait=select.poll();wait.register(descriptor,select.POLLIN)
            emit('waiting-for-primary')
            wait.poll();os.close(descriptor)
        except ProcessLookupError:pass
    time.sleep(.2)
    events=[json.loads(line) for line in (root/'run.log').read_text().splitlines() if line.startswith('{')]
    assert events[-1]['event']=='complete','Primary failed; preserve it before starting further work'
    results=[]
    def run(label,arguments):
        emit('post-count-stage-start',stage=label)
        with (root/f'{label}.log').open('wb') as log:
            value=subprocess.run(arguments,cwd=root,stdout=log,stderr=subprocess.STDOUT)
        results.append({'stage':label,'exit':value.returncode})
        emit('post-count-stage-end',stage=label,exit=value.returncode)
        (root/'post-count-status.json').write_text(json.dumps(results,indent=2)+'\n')
        return value.returncode==0
    qualification=json.loads((root/'results/10000/warm-qualification.json').read_text())
    if not qualification['stabilized']:
        run('guarded-counts',[sys.executable,str(study/'count_censored.py'),str(root),str(study)])
    if run('botstorm-build',[sys.executable,str(study/'build_botstorm.py'),str(root),str(study)]):
        binary=root/'bin/internal_botstorm'
        smoke=root/'storm-smoke';smoke.mkdir();(smoke/'STOP_WARM').write_text('unselected smoke\n')
        smoke_ok=run('botstorm-smoke-warm',[str(binary),'warm',str(smoke),'64'])
        smoke_rows=[]
        if smoke_ok:
            for mode in ['plain','profile']:
                path=root/f'botstorm-smoke-{mode}.json'
                smoke_ok=run('botstorm-smoke-'+mode,[str(binary),'measure',str(smoke),'24',mode,str(path)]) and smoke_ok
                if path.exists():smoke_rows.append(json.loads(path.read_text()))
        if smoke_ok:
            assert len(smoke_rows)==2 and smoke_rows[0]['finalStateSha256']==smoke_rows[1]['finalStateSha256']
            (root/'botstorm-smoke-pass.json').write_text(json.dumps({'passed':True,'population':64,'ticks':24,
               'plainProfileFinalStatesEqual':True,'binarySha256':digest(binary)},indent=2)+'\n')
            run('botstorm-run',[sys.executable,str(study/'run_botstorm.py'),str(root)])
        for n in [1000,2500,5000,10000]:
            directory=population_directory(root,n)
            run(f'view-census-{n}',[str(binary),'views',str(directory/'fixture'),str(root/f'view-census-{n}.json')])
        run('grid-census-botstorm-12000',[str(binary),'grid',str(root/'botstorm-results/12000/fixture'),str(root/'grid-census-botstorm-12000.json')])
    if run('cpu-diagnostic',[sys.executable,str(study/'run_diagnostic.py'),str(root)]):
        run('cpu-classification',[sys.executable,str(study/'classify_diagnostic.py'),str(root)])
    emit('post-count-complete',stages=results)


if __name__=='__main__':execute(*sys.argv[1:])
