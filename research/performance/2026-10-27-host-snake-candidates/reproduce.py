"""Linux x86-64: build, trace, count, then measure baseline legacy tick cost."""
import argparse,gzip,hashlib,json,os,shutil,statistics,struct,subprocess
from pathlib import Path

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('mode',choices=['build','trace','count'])
p.add_argument('repository',type=Path);p.add_argument('output',type=Path)
p.add_argument('--base',default='ca7ac2f2e03507d13ec5078dec049d34c5108e82')
p.add_argument('--candidate',default='7f766d71320dd2857de16b5676769c352ad734c0')
p.add_argument('--perf');p.add_argument('--cpu',type=int,default=0)
p.add_argument('--pairs',type=int,default=5);p.add_argument('--raw-only',action='store_true')
a=p.parse_args();root=a.output.resolve();repo=a.repository.resolve();study=Path(__file__).resolve().parent
assert a.pairs>0;os.sched_setaffinity(0,{a.cpu})
env=dict(os.environ,CARGO_BUILD_JOBS='1',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTUP_TOOLCHAIN='1.99.0',CARGO_PROFILE_RELEASE_DEBUG='1')
env.pop('STUDY_TICK_COUNTS',None)
cases=['official500','official500-half','official500-legacy'];arms=['base','candidate']
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def run(cmd,log,cwd=root,extra=None):
    with Path(log).open('w') as f:subprocess.run(cmd,cwd=cwd,env=dict(env,**(extra or {})),stdout=f,stderr=subprocess.STDOUT,check=True)
def binary(arm):return root/('native-'+arm)/'bin/study_protocols'
def rehash():
    for arm in arms:assert sha(binary(arm))==json.loads((root/('native-'+arm)/'pins.json').read_text())['binarySha256']

if a.mode=='build':
    root.mkdir(parents=True,exist_ok=False)
    for arm,ref in zip(arms,[a.base,a.candidate]):
        out=root/('native-'+arm);source=out/'source';source.mkdir(parents=True)
        archive=out/'source.tar'
        subprocess.run(['git','-C',str(repo),'archive','--format=tar','-o',str(archive),ref],check=True)
        import tarfile
        with tarfile.open(archive) as f:f.extractall(source,filter='data')
        s=source/'src/client/simulation.rs';s.write_text(s.read_text()+'\npub mod study_protocols;\n')
        shutil.copy2(study/'protocol_fixture.rs',source/'src/client/simulation/study_protocols.rs')
        child=source/'src/client/simulation/study_protocols';child.mkdir()
        shutil.copy2(study/'tick_counter.rs',child/'tick_counter.rs')
        (source/'examples/study_protocols.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_protocols::run(&a[1],a.get(2).is_some_and(|s|s=="--trace"));}\n')
        old=source/'research/performance/2026-10-26-host-known-slots';food=source/'research/performance/2026-10-26-host-food-sharing'
        harness=out/'harness';harness.mkdir()
        for n in ['trace_compare.py','instructions.py','loopback.py','battledome-catalog.patch']:shutil.copy2(old/n,harness/n)
        run(['patch','-p1','-i',str(food/'harness.patch')],out/'harness.log',harness)
        run(['patch','-p1','-i',str(food/'client-reconnect.patch')],out/'reconnect.log',source)
        run(['patch','-o',str(source/'examples/perf_battledome.rs'),str(source/'examples/perf_baseline.rs'),str(harness/'battledome-catalog.patch')],out/'catalog.log')
        target=out/'target';names=['study_protocols','perf_baseline','perf_battledome','perf_client_retained']
        run(['cargo','build','--release','--locked',*[x for n in names for x in ['--example',n]]],out/'build.log',source,{'CARGO_TARGET_DIR':str(target)})
        (out/'bin').mkdir()
        for n in names:shutil.copy2(target/'release/examples'/n,out/'bin'/n)
        (out/'pins.json').write_text(json.dumps({'runtimeCommit':ref,'archiveSha256':sha(archive),'fixtureSha256':sha(study/'protocol_fixture.rs'),'counterSha256':sha(study/'tick_counter.rs'),'binarySha256':sha(binary(arm))},indent=2)+'\n')
        print('BUILT',arm,flush=True)

def validate(path,summary):
    frames=wire=0;states=[];last=None
    with gzip.open(path,'rb') as f:
        while h:=f.read(16):
            assert len(h)==16;tag,n=struct.unpack('<QQ',h);data=f.read(n);assert len(data)==n
            if tag in [0,3]:assert json.loads(data)['state'];states.append(hashlib.sha256(data).hexdigest())
            elif tag==1:
                tick,slot,_=struct.unpack('<QQQ',data);assert tick<1080 and slot<500
            elif tag==2:frames+=1;wire+=n
            elif tag==4:assert json.loads(data)==summary
            else:raise AssertionError(tag)
            last=tag
    assert last==4 and len(states)==2 and frames==summary['frames'] and wire==summary['bytes']
    return states

if a.mode=='trace':
    rehash();out=root/'traces';out.mkdir();result={};physical=None
    for case in cases:
        reports={};records={}
        for arm in arms:
            path=out/f'{case}-{arm}.bin.gz';digest=hashlib.sha256();size=0
            with (out/f'{case}-{arm}.stderr').open('w') as err:
                child=subprocess.Popen([str(binary(arm)),case,'--trace'],env=env,stdout=subprocess.PIPE,stderr=err)
                with gzip.open(path,'wb',compresslevel=1) as f:
                    while block:=child.stdout.read(1<<20):f.write(block);digest.update(block);size+=len(block)
                assert child.wait()==0
            reports[arm]=json.loads((out/f'{case}-{arm}.stderr').read_text().splitlines()[-1]);states=validate(path,reports[arm])
            physical=physical or states;assert states==physical,'wire mode changed the physical world'
            records[arm]={'sha256':digest.hexdigest(),'bytes':size}
        assert reports['base']==reports['candidate'] and records['base']==records['candidate']
        with gzip.open(out/f'{case}-base.bin.gz','rb') as left,gzip.open(out/f'{case}-candidate.bin.gz','rb') as right:
            while True:
                x,y=left.read(1<<20),right.read(1<<20);assert x==y
                if not x:break
        result[case]={'equal':True,**records['base'],'checkpointSha256':physical,'summary':reports['base']}
        (out/'selected.json').write_text(json.dumps(result,indent=2)+'\n');print('PARITY',case,flush=True)
    if not a.raw_only:
        for name,extra in [('stock-traces',[]),('dome-traces',['--battledome'])]:
            run(['python3',str(root/'native-base/harness/trace_compare.py'),str(root/'native-base/bin'),str(root/'native-candidate/bin'),str(root/name),*extra],root/(name+'.log'))
    rehash();(root/'traces.complete').write_text('Three complete raw traces equal; catalog selection in logs\n')

if a.mode=='count':
    assert a.perf and (root/'traces.complete').exists();rehash();out=root/'instructions';out.mkdir();result={}
    for case in cases:
        expected=json.loads((root/'traces/selected.json').read_text())[case]['summary'];rows=[]
        for i in range(-1,a.pairs):
            row={'pair':i,'warmup':i<0}
            for arm in (arms if i<0 or i%2==0 else list(reversed(arms))):
                prefix=out/f'{case}-{i}-{arm}'
                run([a.perf,'stat','-x;','-e','instructions:u','-o',str(prefix.with_suffix('.stat')),'--',str(binary(arm)),case],prefix.with_suffix('.log'))
                fields=next(l.split(';') for l in prefix.with_suffix('.stat').read_text().splitlines() if 'instructions:u' in l);assert float(fields[4])>=99
                row[arm]=int(float(fields[0]));assert json.loads(prefix.with_suffix('.log').read_text().splitlines()[-1])==expected
            row['ratio']=row['candidate']/row['base'];rows.append(row)
        v=rows[1:];result[case]={'baseMedian':statistics.median(r['base'] for r in v),'candidateMedian':statistics.median(r['candidate'] for r in v),'pairedMedian':statistics.median(r['ratio'] for r in v),'pairedRange':[min(r['ratio'] for r in v),max(r['ratio'] for r in v)],'pairs':rows}
        (out/'selected.json').write_text(json.dumps(result,indent=2)+'\n');print('COUNT',case,flush=True)
    rehash()

