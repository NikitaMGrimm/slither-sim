"""Rebuild, trace and count the five frozen raw-protocol fixtures on Linux."""
import gzip,hashlib,json,os,shutil,statistics,struct,subprocess,sys,tarfile
from pathlib import Path
import argparse
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('mode',choices=['build','trace','pairs'])
p.add_argument('repository',type=Path)
p.add_argument('output',type=Path)
p.add_argument('--perf')
opts=p.parse_args()
repo=opts.repository.resolve();root=opts.output.resolve();study=Path(__file__).resolve().parent
if opts.mode=='build':root.mkdir(parents=True,exist_ok=False)
os.sched_setaffinity(0,{0,2})
os.environ.update(CARGO_BUILD_JOBS='1',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTUP_TOOLCHAIN='1.99.0',CARGO_PROFILE_RELEASE_DEBUG='1')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
cases=['official500','official500-half','dome64','dome80','dome80-legacy']
pins={'base':'fa92c47118c1ebc97d88ae70cd5c7b20fa94e999','candidate':'20bd42d669148c806c3dcda46a19fe239780ecc8'}
def run(cmd,log,cwd=root,env=None):
    print('START',log,flush=True)
    with (root/log).open('w') as stream:subprocess.run(cmd,cwd=cwd,env=env,stdout=stream,stderr=subprocess.STDOUT,check=True)
    print('PASS',log,flush=True)

if opts.mode=='build':
    identities={}
    for arm,pin in pins.items():
        out=root/('native-'+arm);out.mkdir(exist_ok=False);source=out/'source';source.mkdir()
        archive=out/'source.tar'
        subprocess.run(['git','-C',str(repo),'archive','--format=tar','-o',str(archive),pin],check=True)
        with tarfile.open(archive) as stream:stream.extractall(source,filter='data')
        path=source/'src/client/simulation.rs';path.write_text(path.read_text()+'\npub mod study_protocols;\n')
        shutil.copyfile(study/'protocol_fixture.rs',source/'src/client/simulation/study_protocols.rs')
        (source/'examples/study_protocols.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_protocols::run(&a[1],a.get(2).is_some_and(|s|s=="--trace"));}\n')
        env=dict(os.environ,CARGO_TARGET_DIR=str(out/'target'))
        names=['study_protocols']
        run(['taskset','-c','0','cargo','build','--release','--locked',*[flag for n in names for flag in ['--example',n]]],f'{arm}-native-build.log',source,env)
        for case in cases:
            run(['taskset','-c','0',str(out/'target/release/examples/study_protocols'),case],f'{arm}-{case}-gate.log')
        copied=out/'bin';copied.mkdir()
        for name in names:shutil.copy2(out/'target/release/examples'/name,copied/name)
        identities[arm]={'commit':pin,'archive':sha(archive),'fixture':sha(study/'protocol_fixture.rs'),'binaries':{n:sha(copied/n) for n in names}}
    (root/'native-pins.json').write_text(json.dumps(identities,indent=2)+'\n')

def identities():
    ids=json.loads((root/'native-pins.json').read_text())
    for arm,info in ids.items():
        for name,digest in info['binaries'].items():assert sha(root/('native-'+arm)/'bin'/name)==digest
    return ids

def validate_trace(path,summary):
    frames=wire_bytes=0;last=None;checkpoint_tags=[]
    with gzip.open(path,'rb') as stream:
        while header:=stream.read(16):
            assert len(header)==16
            tag,size=struct.unpack('<QQ',header);payload=stream.read(size);assert len(payload)==size
            if tag in [0,3]:
                assert json.loads(payload)['state'];checkpoint_tags.append(tag)
            elif tag==1:
                tick,slot,at=struct.unpack('<QQQ',payload);assert tick<1080 and slot<summary['seats']
            elif tag==2:frames+=1;wire_bytes+=size
            elif tag==4:assert json.loads(payload)==summary
            else:raise AssertionError(tag)
            last=tag
    assert last==4 and checkpoint_tags==[0,3] and frames==summary['frames'] and wire_bytes==summary['bytes']

if opts.mode=='trace':
    identities();out=root/'protocol-traces';out.mkdir(exist_ok=False);results={}
    for case in cases:
        reports={};digests={}
        for arm in pins:
            destination=out/f'{case}-{arm}.bin.gz';h=hashlib.sha256();size=0
            with (out/f'{case}-{arm}.stderr').open('w') as err:
                p=subprocess.Popen(['taskset','-c','0',str(root/('native-'+arm)/'bin/study_protocols'),case,'--trace'],stdout=subprocess.PIPE,stderr=err)
                with gzip.open(destination,'wb',compresslevel=1) as stream:
                    while block:=p.stdout.read(1<<20):stream.write(block);h.update(block);size+=len(block)
                assert p.wait()==0
            reports[arm]=json.loads((out/f'{case}-{arm}.stderr').read_text().splitlines()[-1]);digests[arm]={'sha256':h.hexdigest(),'bytes':size}
            validate_trace(destination,reports[arm])
        assert reports['base']==reports['candidate']
        with gzip.open(out/f'{case}-base.bin.gz','rb') as a,gzip.open(out/f'{case}-candidate.bin.gz','rb') as b:
            while True:
                left,right=a.read(1<<20),b.read(1<<20);assert left==right
                if not left:break
        results[case]={'equal':True,'trace':digests,'summary':reports['base']}
        (out/'selected.json').write_text(json.dumps(results,indent=2)+'\n');print('PARITY',case,results[case],flush=True)
    identities();(root/'traces.complete').write_text('Five complete raw-protocol traces equal\n')

if opts.mode=='pairs':
    identities();assert (root/'traces.complete').exists()
    perf=opts.perf;assert perf,'--perf required for pairs'
    out=root/'native-instructions';out.mkdir(exist_ok=False);results={}
    for case in cases:
        rows=[];reference=json.loads((root/'protocol-traces/selected.json').read_text())[case]['summary']
        for i in range(-1,5):
            row={'pair':i,'warmup':i<0}
            for arm in (['base','candidate'] if i<0 or i%2==0 else ['candidate','base']):
                prefix=out/f'{case}-{i}-{arm}'
                run(['taskset','-c','0',perf,'stat','-x;','-e','instructions:u','-o',str(prefix)+'.stat','--',str(root/('native-'+arm)/'bin/study_protocols'),case],str(prefix.relative_to(root))+'.log')
                fields=next(l.split(';') for l in Path(str(prefix)+'.stat').read_text().splitlines() if 'instructions:u' in l)
                assert float(fields[4])>=99;row[arm]=int(float(fields[0]))
                report=json.loads(Path(str(prefix)+'.log').read_text().splitlines()[-1]);reference=reference or report;assert report==reference
            row['ratio']=row['candidate']/row['base'];rows.append(row)
        selected=rows[1:];results[case]={'baseMedian':statistics.median(r['base'] for r in selected),'candidateMedian':statistics.median(r['candidate'] for r in selected),'pairedMedian':statistics.median(r['ratio'] for r in selected),'pairedRange':[min(r['ratio'] for r in selected),max(r['ratio'] for r in selected)],'pairs':rows,'summary':reference}
        (out/'selected.json').write_text(json.dumps(results,indent=2)+'\n');print('RESULT',case,results[case],flush=True)
    identities()
