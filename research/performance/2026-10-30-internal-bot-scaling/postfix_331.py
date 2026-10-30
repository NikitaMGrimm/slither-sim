"""Wait for baseline attribution, then measure the pinned external #331 head."""
import json
import os
from pathlib import Path
import re
import select
import shutil
import subprocess
import sys
import tarfile
import tomllib
import urllib.request
from run import digest, emit

HEAD='a6511aadf029adfc1f34b9cdb360a326e2ba5204'


def wait_for(pid,expected):
    try:
        assert expected in (Path('/proc')/str(pid)/'cmdline').read_bytes()
        descriptor=os.pidfd_open(pid);wait=select.poll();wait.register(descriptor,select.POLLIN)
        wait.poll();os.close(descriptor)
    except (FileNotFoundError,ProcessLookupError):pass


def execute(root,study):
    root=Path(root).resolve();study=Path(study).resolve()
    controller_sha=digest(Path(__file__))
    os.sched_setaffinity(0,{3})
    (root/'postfix-331.pid').write_text(str(os.getpid())+'\n')
    emit('postfix-waiting-for-baseline')
    wait_for(int((root/'after-primary.pid').read_text()),b'after_primary.py')
    assert digest(Path(__file__))==controller_sha,'Restart a waiting controller if its source changes'
    status=json.loads((root/'post-count-status.json').read_text())
    assert status and all(row['exit']==0 for row in status),'Baseline stages must complete successfully first'
    diagnostic=json.loads((root/'diagnostic/selected.json').read_text())
    assert len(diagnostic['windows'])==3
    from reduce import mode_summary, phase_summary, verify_master
    reducer_sha=digest(Path(__file__).with_name('reduce.py'))
    master=verify_master(root)
    output=root/'postfix-331';output.mkdir(exist_ok=False)
    archive=output/'source.tar.gz'
    url=f'https://codeload.github.com/NikitaMGrimm/slither-sim/tar.gz/{HEAD}'
    emit('postfix-source-download')
    with urllib.request.urlopen(url,timeout=120) as response,archive.open('xb') as stream:
        shutil.copyfileobj(response,stream)
    container=output/'source';container.mkdir()
    with tarfile.open(archive,'r:gz') as bundle:bundle.extractall(container,filter='data')
    roots=list(container.iterdir());assert len(roots)==1 and roots[0].is_dir()
    source=roots[0]
    assert not any((source/'.cargo'/name).exists() for name in ['config','config.toml']), 'Inspect any new Cargo source-root configuration'
    old_names={p.relative_to(root/'src').as_posix() for p in (root/'src').rglob('*') if p.is_file()}
    new_names={p.relative_to(source/'src').as_posix() for p in (source/'src').rglob('*') if p.is_file()}
    assert old_names==new_names
    different=[name for name in sorted(old_names) if digest(root/'src'/name)!=digest(source/'src'/name)]
    assert different==['food_grid.rs'],different
    old=(root/'src/food_grid.rs').read_text();new=(source/'src/food_grid.rs').read_text()
    def production(text):
        separator='#[cfg(test)]\nmod tests {'
        assert text.count(separator)==1
        return '\n'.join(line for line in text.split(separator)[0].splitlines() if not line.lstrip().startswith('///'))
    assert production(new).replace('const MAX_CELLS: usize = 1 << 22;','const MAX_CELLS: usize = 1 << 20;')==production(old)
    old_manifest=tomllib.loads((root/'Cargo.toml').read_text())
    new_manifest=tomllib.loads((source/'Cargo.toml').read_text())
    for key in ['dependencies','dev-dependencies','profile']:
        assert old_manifest.get(key)==new_manifest.get(key),key
    assert old_manifest['workspace']['package']['version']==new_manifest['workspace']['package']['version']
    env=dict(os.environ,CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'))
    for key in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_PROFILE_RELEASE_DEBUG']:env.pop(key,None)
    for key,tool in [('RUSTC','rustc'),('RUSTDOC','rustdoc')]:
        env[key]=subprocess.check_output(['rustup','which',tool],text=True).strip()
    assert subprocess.check_output([env['RUSTC'],'-Vv'],text=True)==master['rustc']
    trees=[]
    for label,path in [('main',root),('candidate',source)]:
        command=['cargo','tree','--locked','--no-default-features','--features','bot,parallel','-p','slither-sim',
                 '--edges','normal,build,dev','--prefix','none','--format','{p}']
        result=subprocess.run(command,cwd=path,env=env,text=True,capture_output=True)
        (output/f'{label}-dependency-tree.log').write_text(result.stderr)
        result.check_returncode();value=result.stdout
        (output/f'{label}-dependency-tree-private.txt').write_text(value)
        trees.append('\n'.join(sorted(set(re.sub(r' \(.*\)','',line) for line in value.splitlines() if line.strip()))))
    assert trees[0]==trees[1],'Selected dependency package versions differ'
    (output/'selected-dependencies.txt').write_text(trees[0]+'\n')
    main_storm=next(row for row in diagnostic['windows'] if row['label']=='botstorm-12000')
    storm_needed=main_storm['pickupGuards']['allTicks']['boxCells']['max']>(1<<20)
    examples=[('internal_scaling','internal_scaling_331')]
    if storm_needed:examples.append(('internal_botstorm','internal_botstorm_331'))
    for original,name in examples:
        shutil.copy2(root/'examples'/(original+'.rs'),source/'examples'/(name+'.rs'))
    manifest=''.join(p.relative_to(source).as_posix()+'\0'+digest(p)+'\n'
        for p in sorted((source/'src').rglob('*')) if p.is_file())
    manifest+=''.join('examples/'+name+'.rs\0'+digest(source/'examples'/(name+'.rs'))+'\n' for _,name in examples)
    (output/'execution-source-manifest.txt').write_text(manifest)
    command=['cargo','build','--release','--locked','--no-default-features','--features','bot,parallel','-p','slither-sim']
    for _,name in examples:command+=['--example',name]
    emit('postfix-build-start')
    with (output/'build.log').open('wb') as log:
        subprocess.run(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
    binaries={}
    for original,name in examples:
        binary=root/'target/release/examples'/name
        saved=root/'bin'/name;shutil.copy2(binary,saved);binaries[original]=saved
    pins={'head':HEAD,'sourceArchiveUrl':url,'sourceArchiveSha256':digest(archive),
        'masterPinsSha256':digest(root/'measurement-pins.json'),'controllerSha256':controller_sha,
        'reducerSha256':reducer_sha,
        'planSha256':digest(study/'POSTFIX_PLAN.md'),
        'executionSourceManifestSha256':digest(output/'execution-source-manifest.txt'),
        'selectedDependenciesSha256':digest(output/'selected-dependencies.txt'),
        'cargoLockSha256':digest(source/'Cargo.lock'),'buildLogSha256':digest(output/'build.log'),
        'command':command,'profile':'repository default portable release, debug=0, no Rust flags',
        'runtimeSourceChange':'MAX_CELLS 2^20 to 2^22 only; comments/tests excluded from runtime comparison',
        'workspaceDifference':'client workspace import; selected simulator dependency package versions verified equal',
        'binaries':{key:digest(value) for key,value in binaries.items()}}
    (output/'pins.json').write_text(json.dumps(pins,indent=2)+'\n')
    selected={'pins':pins,'mode':'world-only replay; no observation/A4 work in added row',
              'order':'after main rows and attribution; descriptive, not alternating','rows':[],
              'botstormEligible':storm_needed,'botstormBoxCellsMax':main_storm['pickupGuards']['allTicks']['boxCells']['max']}
    cases=[('10000',root/'results/10000','internal_scaling')]
    if storm_needed:cases.append(('botstorm-12000',root/'botstorm-results/12000','internal_botstorm'))
    for label,directory,stem in cases:
        fixture=directory/'fixture';result_dir=output/label;result_dir.mkdir()
        if stem=='internal_scaling':
            reference=json.loads((directory/'verify-serial.json').read_text())['finalStateSha256']
            base_rows=[json.loads((directory/f'pair-{i}-simulation.json').read_text()) for i in range(5)]
        else:
            reference=json.loads((directory/'qualification.json').read_text())['finalStateSha256']
            base_rows=[json.loads((directory/f'window-{i}.json').read_text()) for i in range(5)]
        def run(binary,kind,path):
            args=[str(binary),'measure',str(fixture),*(['simulation'] if stem=='internal_scaling' else []),'512',kind,str(path)]
            subprocess.run(args,check=True)
            row=json.loads(path.read_text());assert row['finalStateSha256']==reference
            return row
        emit('postfix-main-phase-start',row=label)
        baseline_profile=run(root/'bin'/stem,'profile',result_dir/'main-profile.json')
        rows=[]
        for window in range(-1,5):
            emit('postfix-measurement-start',row=label,window=window)
            row=run(binaries[stem],'plain',result_dir/f'window-{window}.json')
            if window>=0:rows.append(row)
            emit('postfix-measurement-end',row=label,window=window)
        profile=run(binaries[stem],'profile',result_dir/'candidate-profile.json')
        # mode_summary expects A4's complete envelope; these are the A4 replay
        # rows by default. Storm uses its own reducer if its conditional arm runs.
        if stem!='internal_scaling':
            for row in base_rows+rows:
                row['mainCpuSeconds']=row['processCpuSeconds'];row['worldOnlyMemoryKiB']=row['initial']['memoryKiB']
        summary=mode_summary(rows)
        selected['rows'].append({'label':label,'referenceFinalStateSha256':reference,
            'allFinalStatesIdentical':True,
            'baselineReference':'selected-results.json:10000/simulation' if stem=='internal_scaling' else 'botstorm-selected.json:12000',
            'candidate':{key:summary[key] for key in ['windows','ticks','worldCpuMs','worldWallMs','admissionCpuMs',
                'wholeWindowTicksPerSecond','worldPlusAdmissionCostTicksPerSecond',
                'firstRestoredTickWorldCpuMs','worldCpuMsExcludingFirstTick','perWindow']},
            'baselinePhaseDiagnostic':phase_summary(baseline_profile,base_rows),
            'candidatePhaseDiagnostic':phase_summary(profile,rows)})
    (output/'selected-results.json').write_text(json.dumps(selected,indent=2)+'\n')
    emit('postfix-complete',rows=[row['label'] for row in selected['rows']],head=HEAD)


if __name__=='__main__':execute(*sys.argv[1:])
