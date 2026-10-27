"""Linux: rebuild, trace, then count the frozen queued-admission/A4 fixtures."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import struct
import subprocess
import tarfile

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('mode',choices=['build','trace','count'])
p.add_argument('repository',type=Path)
p.add_argument('output',type=Path)
p.add_argument('--perf')
p.add_argument('--legacy',action='store_true',help='Separate protocol-13 parity fixture')
p.add_argument('--seeds',nargs='+',type=int,default=[7,29,47])
p.add_argument('--vision',help='Measurement-only feature ref merged into each runtime tree')
p.add_argument('--cpu',type=int,default=0,help='One logical CPU for builds and fixtures')
p.add_argument('--base',default='368abe464c90c998d54c964f2d735ea8880193ea')
p.add_argument('--candidate',default='dcb7fcbf0717f45d6adb23f43ae58d3ab3015457')
args=p.parse_args();root=args.output.resolve();repo=args.repository.resolve()
if args.mode=='build':root.mkdir(parents=True,exist_ok=False)
os.sched_setaffinity(0,{args.cpu})
os.environ.update(CARGO_BUILD_JOBS='1',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',
                  RUSTUP_TOOLCHAIN='1.99.0',CARGO_PROFILE_RELEASE_DEBUG='1')
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
def command(cmd,log,cwd=root,env=None):
    with (root/log).open('w') as stream:
        subprocess.run(cmd,cwd=cwd,env=env,stdout=stream,stderr=subprocess.STDOUT,check=True)

if args.mode=='build':
    for arm,pin in [('base',args.base),('candidate',args.candidate)]:
        if args.vision:
            pin=subprocess.check_output(['git','-C',str(repo),'merge-tree','--write-tree',pin,args.vision],text=True).splitlines()[0]
        kind=subprocess.check_output(['git','-C',str(repo),'cat-file','-t',pin],text=True).strip()
        out=root/('native-'+arm);source=out/'source';source.mkdir(parents=True)
        archive=out/'source.tar'
        subprocess.run(['git','-C',str(repo),'archive','--format=tar','-o',str(archive),pin],check=True)
        with tarfile.open(archive) as f:f.extractall(source,filter='data')
        path=source/'src/client/simulation.rs';path.write_text(path.read_text()+'\npub mod admission_study;\n')
        fixture=Path(__file__).with_name('admission_fixture.rs')
        shutil.copyfile(fixture,source/'src/client/simulation/admission_study.rs')
        fixture_source=source/'src/client/simulation/admission_study.rs'
        if args.legacy:
            from legacy_overlay import legacy_fixture
            fixture_source.write_text(legacy_fixture(fixture_source.read_text()))
        (source/'examples/admission_study.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::admission_study::run(a[1].parse().unwrap(),a[2].parse().unwrap(),a.get(3).is_some_and(|x|x=="--trace"));}\n')
        env=dict(os.environ,CARGO_TARGET_DIR=str(out/'target'))
        command(['cargo','build','--release','--locked','--example','admission_study'],arm+'-build.log',source,env)
        (out/'bin').mkdir();binary=out/'bin/admission_study'
        shutil.copy2(out/'target/release/examples/admission_study',binary)
        (out/'pins.json').write_text(json.dumps({'runtimeRef':pin,'runtimeKind':kind,'archiveSha256':sha(archive),
            'fixtureSha256':sha(fixture_source),'binarySha256':sha(binary)},indent=2)+'\n')

def rehash():
    for arm in ['base','candidate']:
        out=root/('native-'+arm);pin=json.loads((out/'pins.json').read_text())
        assert sha(out/'bin/admission_study')==pin['binarySha256']

def validate(path,summary):
    frames=births=deaths=wire=0;checkpoints=[];last=None
    with gzip.open(path,'rb') as stream:
        while header:=stream.read(16):
            assert len(header)==16
            tag,size=struct.unpack('<QQ',header);payload=stream.read(size);assert len(payload)==size
            if tag in [0,5]:assert json.loads(payload)['state'];checkpoints.append(tag)
            elif tag==1:births+=1
            elif tag==2:
                tick,slot,at=struct.unpack('<QQQ',payload);assert tick<summary['ticks'] and slot<summary['seats']
            elif tag==3:frames+=1;wire+=size
            elif tag==4:deaths+=1
            elif tag==6:assert json.loads(payload)==summary
            else:raise AssertionError(tag)
            last=tag
    assert last==6 and checkpoints==[0,5] and frames==summary['frames']
    assert wire==summary['bytes'] and births==summary['admissions'] and deaths==summary['deaths']

if args.mode=='trace':
    rehash();out=root/'traces';out.mkdir();results={}
    for count in [64,80]:
        for seed in args.seeds:
            case=f'{count}-{seed}';reports={};records={}
            for arm in ['base','candidate']:
                target=out/f'{case}-{arm}.bin.gz';digest=hashlib.sha256();size=0
                with (out/f'{case}-{arm}.stderr').open('w') as err:
                    child=subprocess.Popen([str(root/('native-'+arm)/'bin/admission_study'),str(count),str(seed),'--trace'],stdout=subprocess.PIPE,stderr=err)
                    with gzip.open(target,'wb',compresslevel=1) as stream:
                        while block:=child.stdout.read(1<<20):stream.write(block);digest.update(block);size+=len(block)
                    assert child.wait()==0
                reports[arm]=json.loads((out/f'{case}-{arm}.stderr').read_text().splitlines()[-1])
                validate(target,reports[arm]);records[arm]={'sha256':digest.hexdigest(),'bytes':size}
            assert reports['base']==reports['candidate'] and records['base']==records['candidate']
            with gzip.open(out/f'{case}-base.bin.gz','rb') as left,gzip.open(out/f'{case}-candidate.bin.gz','rb') as right:
                while True:
                    a,b=left.read(1<<20),right.read(1<<20);assert a==b
                    if not a:break
            results[case]={'equal':True,**records['base'],'summary':reports['base']}
            (out/'selected.json').write_text(json.dumps(results,indent=2)+'\n');print('PARITY',case,flush=True)
    rehash();(root/'traces.complete').write_text(f'{len(results)} complete equal traces\n')

if args.mode=='count':
    assert args.perf and (root/'traces.complete').exists();rehash()
    out=root/'instructions';out.mkdir();results={}
    for count in [64,80]:
        case=f'{count}-7';expected=json.loads((root/'traces/selected.json').read_text())[case]['summary'];rows=[]
        for index in range(-1,5):
            row={'pair':index,'warmup':index<0}
            for arm in (['base','candidate'] if index<0 or index%2==0 else ['candidate','base']):
                prefix=out/f'{case}-{index}-{arm}'
                command([args.perf,'stat','-x;','-e','instructions:u','-o',str(prefix)+'.stat','--',str(root/('native-'+arm)/'bin/admission_study'),str(count),'7'],str(prefix.relative_to(root))+'.log')
                fields=next(line.split(';') for line in Path(str(prefix)+'.stat').read_text().splitlines() if 'instructions:u' in line)
                assert float(fields[4])>=99
                row[arm]=int(float(fields[0]));assert json.loads(Path(str(prefix)+'.log').read_text().splitlines()[-1])==expected
            row['ratio']=row['candidate']/row['base'];rows.append(row)
        ratios=[r['ratio'] for r in rows[1:]]
        results[case]={'baseMedian':statistics.median(r['base'] for r in rows[1:]),
            'candidateMedian':statistics.median(r['candidate'] for r in rows[1:]),
            'pairedMedian':statistics.median(ratios),'pairedRange':[min(ratios),max(ratios)],'pairs':rows}
        (out/'selected.json').write_text(json.dumps(results,indent=2)+'\n');print('COUNT',case,flush=True)
    rehash()
