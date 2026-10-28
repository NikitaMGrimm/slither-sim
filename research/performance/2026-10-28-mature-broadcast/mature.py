"""Retained experimental mature-core recipe, NOT a supported host benchmark.

The churn warmup intentionally fails its length screen and retains its input
for extend.py. The host trace then fails the missing food-birth provenance
guard. No mature pairs, profiles or mutation control completed; those branches
are retained experimental code, not evidence. See REPORT.md before running.
"""
import argparse
import gzip
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess

study=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('broadcast',study.parent/'2026-10-28-broadcast-distance/reproduce.py')
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('mode',choices=['build','warm','trace','pairs','profile','mutation'])
p.add_argument('root',type=Path);p.add_argument('--arm',default='base');p.add_argument('--profile',action='store_true')
p.add_argument('--cpu',type=int,default=0)
a=p.parse_args();root=a.root.resolve();os.sched_setaffinity(0,{a.cpu})
env=dict(os.environ,RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1')
def binary(arm):return root/arm/'bin/study_mature'
def command(arm,legacy=False,trace=False):
    return [str(binary(arm)),'run',str(root/'warm/checkpoint.json'),str(root/'warm/meta.json'),str(int(legacy)),str(int(trace))]
def verify(arm):
    assert base.sha(binary(arm))==json.loads((root/arm/'pins.json').read_text())['binarySha256']
    if (root/'checkpoint-sha256.txt').exists():
        assert base.sha(root/'warm/checkpoint.json')==(root/'checkpoint-sha256.txt').read_text().strip()
def run(cmd,label,extra=None):
    arm=Path(cmd[0]).parent.parent.name;verify(arm)
    with (root/(label+'.stdout')).open('xb') as out,(root/(label+'.stderr')).open('xb') as err:
        subprocess.run(cmd,env=dict(env,**(extra or {})),stdout=out,stderr=err,check=True)
    verify(arm)
def validate(path,summary):
    frames=total=states=0;last=None
    with gzip.open(path,'rb') as f:
        while header:=f.read(16):
            assert len(header)==16;tag,n=struct.unpack('<QQ',header);data=f.read(n);assert len(data)==n
            if tag in [0,3,4]:
                value=json.loads(data);assert value['state'] and value['config'];states+=1
            elif tag==1:
                tick,slot,_=struct.unpack('<QQQ',data);assert tick<480 and slot<500
            elif tag==2:frames+=1;total+=n
            elif tag==5:assert json.loads(data)==summary
            else:raise AssertionError(tag)
            last=tag
    assert last==5 and states==3 and frames==summary['frames'] and total==summary['bytes'] and summary['ticks']==360
if a.mode=='build':
    name,source,pin=base.prepare(root,a.arm,a.profile)
    (source/'src/client/simulation/study_mature.rs').write_bytes((study/'mature.rs').read_bytes())
    f=source/'src/client/simulation.rs';f.write_text(f.read_text()+'\npub mod study_mature;\n')
    counter=source/'src/study_counter.rs'
    counter.write_text(counter.read_text()+'''\npub fn clear(){
C.with_borrow_mut(|c|{c.sums=[0;16];c.calls=[0;16];});
let p=unsafe{dlsym(std::ptr::null_mut(),c"study_clear".as_ptr().cast())};
if !p.is_null(){unsafe{std::mem::transmute::<*mut c_void,unsafe extern "C" fn()>(p)()};}
}\n''')
    if a.profile:
        sim=source/'src/simulator.rs'
        base.swap(sim,'fn compact_food(food: &mut Vec<Food>, removed: impl Iterator<Item = usize>) {',
                  'fn compact_food(food: &mut Vec<Food>, removed: impl Iterator<Item = usize>) {\n let _study_compact=crate::study_counter::enter(10);')
        base.swap(sim,'        // Stable-slot indexing avoids searching the living list for every pair.',
                  '        let study_collision_build=crate::study_counter::enter(11);\n        // Stable-slot indexing avoids searching the living list for every pair.')
        base.swap(sim,'        candidates.clear();\n        dead.clear();',
                  '        drop(study_collision_build);\n        let study_collision_query=crate::study_counter::enter(12);\n        candidates.clear();\n        dead.clear();')
        base.swap(sim,'        if PROFILE {\n            profile.collision_seconds',
                  '        drop(study_collision_query);\n        if PROFILE {\n            profile.collision_seconds')
    (source/'examples/study_mature.rs').write_text('''fn main(){let a:Vec<_>=std::env::args().collect();
if a[1]=="warm" {slither_sim::client::simulation::study_mature::warm(std::path::Path::new(&a[2]));}
else {let g=slither_sim::study_counter::enter(1);slither_sim::client::simulation::study_mature::run(std::path::Path::new(&a[2]),std::path::Path::new(&a[3]),a[4]=="1",a[5]=="1");drop(g);slither_sim::study_counter::finish();}}\n''')
    compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
    buildenv=dict(env,CARGO_TARGET_DIR=str(root/'target'),CARGO_BUILD_JOBS='1',CARGO_PROFILE_RELEASE_DEBUG='1',RUSTC=compiler)
    with (root/name/'build.log').open('x') as log:
        subprocess.run(['cargo','build','--release','--locked','--offline','--example','study_mature'],cwd=source,env=buildenv,stdout=log,stderr=subprocess.STDOUT,check=True)
    (root/name/'bin').mkdir();shutil.copy2(root/'target/release/examples/study_mature',binary(name))
    archive_arm='candidate' if a.arm=='mutation' else a.arm
    (root/name/'pins.json').write_text(json.dumps({'archiveCommit':pin,'archiveSha256':base.sha(root/(archive_arm+'.tar')),
        'binarySha256':base.sha(binary(name)),'sources':{str(f.relative_to(source)):base.sha(f) for f in source.rglob('*.rs')},
        'driverSha256':base.sha(Path(__file__)),'fixtureSha256':base.sha(study/'mature.rs')},indent=2)+'\n')
    print('BUILT',name,flush=True)
elif a.mode=='warm':
    run([str(binary('base')),'warm',str(root/'warm')],'warm')
    (root/'checkpoint-sha256.txt').write_text(base.sha(root/'warm/checkpoint.json')+'\n')
elif a.mode in ['trace','mutation']:
    rows=[]
    for legacy in [False,True]:
        label='mature13' if legacy else 'mature19';row={'case':label}
        for arm in (['mutation'] if a.mode=='mutation' else ['base','candidate']):
            verify(arm)
            path=root/(label+'-'+arm+'.gz');digest=__import__('hashlib').sha256();n=0
            with gzip.open(path,'xb',compresslevel=1) as out,(root/(label+'-'+arm+'.stderr')).open('xb') as err:
                proc=subprocess.Popen(command(arm,legacy,True),env=env,stdout=subprocess.PIPE,stderr=err)
                for block in iter(lambda:proc.stdout.read(1<<20),b''):out.write(block);digest.update(block);n+=len(block)
                assert proc.wait()==0
            summary=json.loads((root/(label+'-'+arm+'.stderr')).read_text().splitlines()[-1]);validate(path,summary);verify(arm)
            row[arm]={'sha256':digest.hexdigest(),'bytes':n}
        if a.mode=='trace':
            assert row['base']==row['candidate'] and base.compare(root/(label+'-base.gz'),root/(label+'-candidate.gz'))
        else:
            assert not base.compare(root/(label+'-candidate.gz'),root/(label+'-mutation.gz'))
            row['detected']=True
        rows.append(row);(root/('mutation.json' if a.mode=='mutation' else 'parity.json')).write_text(json.dumps(rows,indent=2)+'\n');print(a.mode,label,flush=True)
elif a.mode=='pairs':
    assert len(json.loads((root/'parity.json').read_text()))==2
    assert len(json.loads((root/'mutation.json').read_text()))==2
    rows=[]
    for legacy in [False,True]:
        for i in range(-1,5):
            row={'legacy':legacy,'pair':i,'warmup':i<0}
            for arm in (['base','candidate'] if i<0 or i%2==0 else ['candidate','base']):
                label=f'pair-{int(legacy)}-{i}-{arm}';base.inventory(root,label+'-before')
                run(command(arm,legacy),label,{'STUDY_COUNTS':str(root/(label+'-counts.json'))})
                base.inventory(root,label+'-after');row[arm]=json.loads((root/(label+'-counts.json')).read_text())
            assert (root/f'pair-{int(legacy)}-{i}-base.stderr').read_bytes()==(root/f'pair-{int(legacy)}-{i}-candidate.stderr').read_bytes()
            row['ratio']=row['candidate']['instructions'][2]/row['base']['instructions'][2]
            rows.append(row);(root/'pairs.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps({'pair':i,'legacy':legacy,'ratio':row['ratio']}),flush=True)
elif a.mode=='profile':
    for legacy in [False,True]:
        label='profile-'+str(int(legacy));run(command('base-profile',legacy),label,{'STUDY_COUNTS':str(root/(label+'-counts.json'))})
