"""Released Linux bench only: immutable full-host forks, proof traces and five pairs.

Usage: python run_pairs.py OUTPUT PRIOR_REPRODUCE [13|19] [spawn|minimap|markers]
Requires release binaries/pins and successful correctness gates. No builds.
"""
from contextlib import ExitStack
from pathlib import Path
import hashlib,importlib.util,json,os,queue,subprocess,sys,threading
root=Path(sys.argv[1]).resolve();prior=Path(sys.argv[2]).resolve();protocol=int(sys.argv[3])
row=sys.argv[4] if len(sys.argv)>4 else 'spawn';prefix=f'{row}-{protocol}'
assert sys.platform=='linux' and protocol in [13,19] and row in ['spawn','minimap','markers']
assert (root/'required-checks-pass.json').exists()
assert json.loads((root/'required-checks-pass.json').read_text())['passed']
assert (root/'parity-pass.json').exists() and json.loads((root/'parity-pass.json').read_text())['passed']
os.sched_setaffinity(0,{1})
spec=importlib.util.spec_from_file_location('prior',prior);shared=importlib.util.module_from_spec(spec);spec.loader.exec_module(shared)

def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def same(a,b):
    with a.open('rb') as x,b.open('rb') as y:
        while True:
            l,r=x.read(1<<20),y.read(1<<20)
            if l!=r:return False
            if not l:return True

def terminal(path):
    import struct
    tags=[]
    with path.open('rb') as f:
        while h:=f.read(16):
            assert len(h)==16;tag,n=struct.unpack('<QQ',h);data=f.read(n);assert len(data)==n
            if tag in [0,3,4]:json.loads(data)
            tags.append(tag)
    assert tags[0]==0 and tags[-2:]==[3,4] and 2 in tags

class Parent:
    def __init__(self,arm):
        self.arm=arm;self.exe=root/arm/'bin/study_fixed'
        self.pins=json.loads((root/arm/'pins.json').read_text());assert self.pins['releaseBuild']
        assert sha(self.exe)==self.pins['binaries']['study_fixed']
        self.log=(root/f'parent-{prefix}-{arm}.log').open('x');self.lines=queue.Queue()
        self.process=subprocess.Popen([str(self.exe),str(protocol),'360','--measure',row],stdin=subprocess.PIPE,
            stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True,bufsize=1,
            env=dict(os.environ,RAYON_NUM_THREADS='1'),preexec_fn=lambda:os.sched_setaffinity(0,{0}))
        def read():
            for line in self.process.stderr:
                self.log.write(line);self.log.flush();self.lines.put(line.rstrip())
            self.lines.put(None)
        self.thread=threading.Thread(target=read,daemon=True);self.thread.start()
        try:
            line=self.lines.get(timeout=120);assert line and line.startswith('READY '),line
            self.ready=json.loads(line[6:])
        except BaseException:
            self.close();raise
    def request(self,label,direct=False):
        path=root/f'{prefix}-{label}-{self.arm}.json';assert not path.exists()
        self.process.stdin.write(('parent ' if direct else 'child ')+str(path)+'\n');self.process.stdin.flush()
        line=self.lines.get(timeout=120);assert line=='DONE '+str(path),line
        terminal(path.with_suffix('.trace'))
        result=json.loads(path.read_text());rows=result['ticks'];summary=result['summary']
        expected=72 if row=='spawn' else 0
        assert len(rows)==360 and summary['waiting']==expected
        if row=='spawn':
            assert summary['blockedAttempts']==4096*summary['blockedSearches']
            assert summary['blockedExhausted']==summary['blockedSearches']>=120
        else:
            assert any(r['minimap'] for r in rows)
            if row=='markers':assert sum(r['marker'] for r in rows)>=3
        for tick_row in rows:
            s=tick_row['sample'];assert tick_row['waitingBefore']==tick_row['waitingAfter']==expected
            assert s['enabledNs']==0 or s['runningNs']/s['enabledNs']>=.99
        assert sha(self.exe)==self.pins['binaries']['study_fixed']
        return path
    def close(self):
        try:
            if self.process.poll() is None:
                try:self.process.stdin.write('quit\n');self.process.stdin.flush()
                except BrokenPipeError:pass
                self.process.wait(timeout=5)
        finally:
            if self.process.poll() is None:self.process.kill();self.process.wait()
            self.thread.join(timeout=5);self.process.stdin.close();self.process.stderr.close();self.log.close()

rows=[]
with ExitStack() as cleanup:
    parents={}
    for arm in ['base','candidate']:
        p=Parent(arm);parents[arm]=p;cleanup.callback(p.close)
    assert parents['base'].ready==parents['candidate'].ready
    proofs={}
    for arm,p in parents.items():
        proofs[arm]=[p.request(f'proof-{i}') for i in [0,1]]
        assert same(*(x.with_suffix('.trace') for x in proofs[arm]))
    assert same(proofs['base'][0].with_suffix('.trace'),proofs['candidate'][0].with_suffix('.trace'))
    for pair in range(-1,5):
        shared.inventory(root,f'{prefix}-pair-{pair}-before')
        paths={}
        for arm in (['base','candidate'] if pair==-1 or pair%2==0 else ['candidate','base']):
            paths[arm]=parents[arm].request(f'pair-{pair}')
        shared.inventory(root,f'{prefix}-pair-{pair}-after')
        assert same(*(paths[a].with_suffix('.trace') for a in ['base','candidate']))
        rows.append({'pair':pair,'warmup':pair==-1,'paths':{a:p.name for a,p in paths.items()}})
        (root/f'pairs-{prefix}.json').write_text(json.dumps(rows,indent=2)+'\n')
    for arm,p in parents.items():
        direct=p.request('direct-parent',True)
        assert same(direct.with_suffix('.trace'),proofs[arm][0].with_suffix('.trace'))
print('Five alternating pairs and repeated-fork/direct-parent proofs complete',protocol)
