"""Post-count user-CPU samples and slow-tick clocks; no production edits."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from diagnostic_source import instrument
from run import digest


def execute(root):
    root=Path(root).resolve()
    events=[json.loads(line) for line in (root/'run.log').read_text().splitlines() if line.startswith('{')]
    assert events[-1]['event']=='complete','Never build/profile during primary counting'
    master=json.loads((root/'measurement-pins.json').read_text())
    storm=json.loads((root/'botstorm-build.json').read_text())
    output=root/'diagnostic';output.mkdir(exist_ok=False)
    saved=root/'bin';saved.mkdir(exist_ok=True)
    manifest=[]
    for stem,pins,key in [('internal_scaling',master,'harnessSha256'),
                          ('internal_botstorm',storm,'generatedHarnessSha256')]:
        binary=root/'target/release/examples'/stem
        assert digest(binary)==pins['binarySha256']
        destination=saved/stem
        if destination.exists():assert digest(destination)==pins['binarySha256']
        else:shutil.copy2(binary,destination)
        source=root/'examples'/(stem+'.rs')
        assert digest(source)==pins[key]
        generated=root/'examples'/(stem+'_debug.rs')
        generated.write_text(instrument(source.read_text()))
        manifest.append(generated.relative_to(root).as_posix()+'\0'+digest(generated)+'\n')
    (output/'generated-source-manifest.txt').write_text(''.join(manifest))
    env=dict(os.environ,CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',
             CARGO_TARGET_DIR=str(root/'target'),CARGO_PROFILE_RELEASE_DEBUG='2',DEBUGINFOD_URLS='')
    for key in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(key,None)
    for key,tool in [('RUSTC','rustc'),('RUSTDOC','rustdoc')]:
        env[key]=subprocess.check_output(['rustup','which',tool],text=True).strip()
    command=['cargo','build','--release','--locked','--no-default-features','--features','bot,parallel',
             '--example','internal_scaling_debug','--example','internal_botstorm_debug']
    with (output/'build.log').open('wb') as out:
        subprocess.run(command,cwd=root,env=env,stdout=out,stderr=subprocess.STDOUT,check=True)
    binaries={}
    for stem in ['internal_scaling','internal_botstorm']:
        binary=root/'target/release/examples'/(stem+'_debug')
        shutil.copy2(binary,saved/(stem+'_debug'));binaries[stem]=binary
    pins={'masterPinsSha256':digest(root/'measurement-pins.json'),
          'botstormBuildSha256':digest(root/'botstorm-build.json'),
          'generatedSourceManifestSha256':digest(output/'generated-source-manifest.txt'),
          'runnerSha256':digest(Path(__file__)),
          'instrumenterSha256':digest(Path(__file__).with_name('diagnostic_source.py')),
          'a4BinarySha256':digest(binaries['internal_scaling']),
          'botstormBinarySha256':digest(binaries['internal_botstorm']),
          'buildLogSha256':digest(output/'build.log'),'command':command,
          'profile':'portable default codegen, debug=2, timestamps and event-derived pickup guards; no Rust flags',
          'sampling':'cpu-clock:u, 499 Hz, DWARF 4096-byte stacks, CLOCK_MONOTONIC',
          'clockScope':'admission/world brackets; guard arithmetic after world end; event recording is inside step; primary binaries unchanged'}
    (output/'pins.json').write_text(json.dumps(pins,indent=2)+'\n')
    os.sched_setaffinity(0,{3})
    summaries=[]
    for label,n,stem,directory in [
        ('1000',1000,'internal_scaling',root/'results/1000'),
        ('10000',10000,'internal_scaling',root/'results/10000'),
        ('botstorm-12000',12000,'internal_botstorm',root/'botstorm-results/12000')]:
        binary=binaries[stem];fixture=directory/'fixture'
        ordinary=output/f'{label}-ordinary.json';sampled=output/f'{label}-sampled.json'
        def args(kind,path):
            return [str(binary),'measure',str(fixture),*(['serial'] if stem=='internal_scaling' else []),'512',kind,str(path)]
        subprocess.run(args('plain',ordinary),env=env,check=True)
        with (output/f'{label}-perf.log').open('wb') as out:
            subprocess.run(['perf','record','--no-buildid-cache','--clockid','mono','-o',str(output/f'{label}.data'),
                '-e','cpu-clock:u','-F','499','--call-graph','dwarf,4096','--',*args('plain',sampled)],
                env=env,stdout=out,stderr=out,check=True)
        with (output/f'{label}-stacks.txt').open('wb') as out:
            subprocess.run(['perf','script','-i',str(output/f'{label}.data'),
                '--ns','--inline','--full-source-path','-F','+srcline,+period'],env=env,stdout=out,check=True)
        with (output/f'{label}-report.txt').open('wb') as out:
            subprocess.run(['perf','report','-i',str(output/f'{label}.data'),
                '--stdio','--no-children','-g','none','--percent-limit','0.5'],env=env,stdout=out,check=True)
        a=json.loads(ordinary.read_text());b=json.loads(sampled.read_text())
        assert a['finalStateSha256']==b['finalStateSha256']
        phase_path=output/f'{label}-phases.json'
        subprocess.run(args('profile',phase_path),env=env,check=True)
        assert json.loads(phase_path.read_text())['finalStateSha256']==a['finalStateSha256']
        guards=lambda row:[s['pickupGuard'] for s in row['samples']]
        assert guards(a)==guards(b)==guards(json.loads(phase_path.read_text()))
        if stem=='internal_scaling':
            reference=json.loads((directory/'verify-serial.json').read_text())['finalStateSha256']
            qualification=json.loads((directory/'warm-qualification.json').read_text())
            rows=[json.loads((directory/f'pair-{i}-serial.json').read_text()) for i in range(5)]
        else:
            qualification=json.loads((directory/'qualification.json').read_text())
            reference=qualification['finalStateSha256']
            rows=[json.loads((directory/f'window-{i}.json').read_text()) for i in range(5)]
        assert a['finalStateSha256']==reference
        mean_world=lambda row:sum(s['worldCpu'] for s in row['samples'])/row['ticks']
        summaries.append({'label':label,'population':n,'fixtureAttempt':directory.name,
            'stabilized':qualification['stabilized'],
            'timestampDebugToPrimaryWorldCpuRatios':[mean_world(a)/mean_world(row) for row in rows],
            'timestampDebugToPrimaryWholeCpuRatios':[a['processCpuSeconds']/row['processCpuSeconds'] for row in rows],
            'ordinaryProcessCpuSeconds':a['processCpuSeconds'],'sampledProcessCpuSeconds':b['processCpuSeconds'],
            'cpuRatio':b['processCpuSeconds']/a['processCpuSeconds'],'wallRatio':b['wallSeconds']/a['wallSeconds'],
            'finalStateSha256':a['finalStateSha256']})
    (output/'overhead.json').write_text(json.dumps(summaries,indent=2)+'\n')
    assert digest(binaries['internal_scaling'])==pins['a4BinarySha256']
    assert digest(binaries['internal_botstorm'])==pins['botstormBinarySha256']
    print(json.dumps(summaries,indent=2),flush=True)


if __name__=='__main__':execute(sys.argv[1])
