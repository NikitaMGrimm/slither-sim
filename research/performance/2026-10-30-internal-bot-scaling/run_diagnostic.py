"""Post-count CPU samples for observation attribution; no production edits."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from reduce import population_directory


def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def execute(root):
    root=Path(root).resolve()
    events=[json.loads(line) for line in (root/'run.log').read_text().splitlines() if line.startswith('{')]
    assert events[-1]['event']=='complete','Never build/profile during primary counting'
    master=json.loads((root/'measurement-pins.json').read_text())
    original=root/'target/release/examples/internal_scaling'
    assert digest(original)==master['binarySha256']
    saved=root/'bin';saved.mkdir(exist_ok=True)
    destination=saved/'internal_scaling'
    if destination.exists():assert digest(destination)==master['binarySha256']
    else:shutil.copy2(original,destination)
    source=root/'examples/internal_scaling.rs'
    assert digest(source)==master['harnessSha256']
    debug_source=root/'examples/internal_scaling_debug.rs'
    shutil.copyfile(source,debug_source)
    output=root/'diagnostic';output.mkdir(exist_ok=False)
    env=dict(os.environ,CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',
             CARGO_TARGET_DIR=str(root/'target'),CARGO_PROFILE_RELEASE_DEBUG='2')
    for key in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(key,None)
    for key,tool in [('RUSTC','rustc'),('RUSTDOC','rustdoc')]:
        env[key]=subprocess.check_output(['rustup','which',tool],text=True).strip()
    command=['cargo','build','--release','--locked','--no-default-features','--features','bot,parallel','--example','internal_scaling_debug']
    with (output/'build.log').open('wb') as out:
        subprocess.run(command,cwd=root,env=env,stdout=out,stderr=subprocess.STDOUT,check=True)
    binary=root/'target/release/examples/internal_scaling_debug'
    shutil.copy2(binary,saved/'internal_scaling_debug')
    pins={'masterPinsSha256':digest(root/'measurement-pins.json'),
          'harnessSha256':digest(debug_source),'runnerSha256':digest(Path(__file__)),
          'binarySha256':digest(binary),'buildLogSha256':digest(output/'build.log'),
          'command':command,'profile':'portable default codegen with debug=2, no Rust flags; diagnostic only',
          'sampling':'cpu-clock:u at 499 Hz, DWARF 8192-byte stacks'}
    (output/'pins.json').write_text(json.dumps(pins,indent=2)+'\n')
    os.sched_setaffinity(0,{3})
    summaries=[]
    for n in [1000,10000]:
        directory=population_directory(root,n)
        if n==10000 and directory.name=='10000' and (root/'results/10000-continuation/fixture/world.json').exists():
            directory=root/'results/10000-continuation'
        fixture=directory/'fixture'
        ordinary=output/f'{n}-ordinary.json';profiled=output/f'{n}-sampled.json'
        args=[str(binary),'measure',str(fixture),'serial','512','plain']
        subprocess.run([*args,str(ordinary)],check=True)
        with (output/f'{n}-perf.log').open('wb') as out:
            subprocess.run(['perf','record','-o',str(output/f'{n}.data'),
                            '-e','cpu-clock:u','-F','499','--call-graph','dwarf,8192','--',
                            *args,str(profiled)],stdout=out,stderr=out,check=True)
        with (output/f'{n}-stacks.txt').open('wb') as out:
            subprocess.run(['perf','script','-i',str(output/f'{n}.data'),
                            '--inline','--full-source-path','-F','+srcline'],stdout=out,check=True)
        with (output/f'{n}-report.txt').open('wb') as out:
            subprocess.run(['perf','report','-i',str(output/f'{n}.data'),
                            '--stdio','--no-children','-g','none','--percent-limit','0.5'],stdout=out,check=True)
        a=json.loads(ordinary.read_text());b=json.loads(profiled.read_text())
        assert a['finalStateSha256']==b['finalStateSha256']
        reference=directory/'verify-serial.json'
        if reference.exists():assert a['finalStateSha256']==json.loads(reference.read_text())['finalStateSha256']
        qualification=json.loads((directory/'warm-qualification.json').read_text())
        summaries.append({'population':n,'stabilized':qualification['stabilized'],
                          'ordinaryProcessCpuSeconds':a['processCpuSeconds'],
                          'sampledProcessCpuSeconds':b['processCpuSeconds'],
                          'cpuRatio':b['processCpuSeconds']/a['processCpuSeconds'],
                          'wallRatio':b['wallSeconds']/a['wallSeconds'],
                          'finalStateSha256':a['finalStateSha256']})
    (output/'overhead.json').write_text(json.dumps(summaries,indent=2)+'\n')
    assert digest(binary)==pins['binarySha256']
    print(json.dumps(summaries,indent=2),flush=True)


if __name__=='__main__':execute(sys.argv[1])
