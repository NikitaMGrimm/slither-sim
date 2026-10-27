"""Full shrink traces, process pairs and paired per-tick distributions."""
import argparse
import hashlib
import json
import lzma
import os
from pathlib import Path
import statistics
import subprocess
if not __debug__:
    raise SystemExit('Run without optimization')
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('mode',choices=['trace','count','ticks']);p.add_argument('root',type=Path);p.add_argument('--perf');p.add_argument('--cpu',type=int,default=3);p.add_argument('--compression-cpu',type=int,default=2);a=p.parse_args()
root=a.root.resolve();os.sched_setaffinity(0,{a.cpu})
cases=['official500-shrink','battledome80-shrink'];arms=['base','candidate']
env=dict(os.environ,RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1')
env.pop('STUDY_TICK_COUNTS',None);env.pop('SHRINK_METADATA',None)
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
def binary(arm):return root/f'native-{arm}/bin/study_retention'
def rehash():
    for arm in arms:
        pins=json.loads((root/f'native-{arm}/binaries.json').read_text())
        assert sha(binary(arm))==pins['study_retention']
def run(cmd,log,extra=None):
    with log.open('w') as f:subprocess.run(cmd,env=dict(env,**(extra or {})),stdout=f,stderr=subprocess.STDOUT,check=True)
def expected(case):return json.loads((root/'shrink-traces/selected.json').read_text())[case]['summary']
def quantile(values,q):
    values=sorted(values);position=(len(values)-1)*q;lower=int(position);upper=min(lower+1,len(values)-1)
    return values[lower]+(values[upper]-values[lower])*(position-lower)
def distribution(values):return {'n':len(values),'mean':statistics.fmean(values),'p50':quantile(values,.5),'p90':quantile(values,.9),'p95':quantile(values,.95),'p99':quantile(values,.99),'min':min(values),'max':max(values)}
rehash()
if a.mode=='trace':
    out=root/'shrink-traces';out.mkdir();result={}
    for case in cases:
        records={};summaries={}
        for arm in arms:
            path=out/f'{case}-{arm}.jsonl.xz';digest=hashlib.sha256();size=lines=0;tail=b''
            # Only this streaming trace stage uses two CPUs. Counter stages use
            # CPU3 alone; compression and simulation never exceed CPUs2/3.
            os.sched_setaffinity(0,{a.compression_cpu})
            with (out/f'{case}-{arm}.stderr').open('w') as error:
                child=subprocess.Popen([str(binary(arm)),case,'--trace'],env=env,stdout=subprocess.PIPE,stderr=error,
                    preexec_fn=lambda:os.sched_setaffinity(0,{a.cpu}))
                try:
                    with lzma.open(path,'wb',filters=[{'id':lzma.FILTER_LZMA2,'preset':0,'dict_size':16<<20}]) as f:
                        while block:=child.stdout.read(1<<20):
                            digest.update(block);size+=len(block);lines+=block.count(b'\n');tail=(tail+block)[-(1<<20):];f.write(block)
                    assert child.wait()==0
                finally:
                    if child.poll() is None:child.terminate();child.wait()
            os.sched_setaffinity(0,{a.cpu})
            summary=json.loads((out/f'{case}-{arm}.stderr').read_text().splitlines()[-1])
            assert json.loads(tail.splitlines()[-1])==summary and summary['kind']=='report' and summary['ticks']==3200
            assert lines==3202
            with lzma.open(path,'rb') as f:
                initial=json.loads(f.readline());assert initial['kind']=='initial' and initial['case']==case and initial['checkpoint']['state']
            records[arm]={'sha256':digest.hexdigest(),'bytes':size,'records':lines,'binarySha256':sha(binary(arm))}
            summaries[arm]=summary
        assert summaries['base']==summaries['candidate']
        assert records['base']['sha256']==records['candidate']['sha256'] and records['base']['bytes']==records['candidate']['bytes']
        with lzma.open(out/f'{case}-base.jsonl.xz','rb') as left,lzma.open(out/f'{case}-candidate.jsonl.xz','rb') as right:
            while True:
                x,y=left.read(1<<20),right.read(1<<20);assert x==y
                if not x:break
        result[case]={'equal':True,'summary':summaries['base'],'records':records}
        (out/'selected.json').write_text(json.dumps(result,indent=2)+'\n');print('FULL SHRINK TRACE EQUAL',case,flush=True)
if a.mode=='count':
    assert a.perf;out=root/'shrink-instructions';out.mkdir();result={}
    for case in cases:
        summary=expected(case);rows=[]
        for i in range(-1,5):
            row={'pair':i,'warmup':i<0}
            for arm in (arms if i<0 or i%2==0 else list(reversed(arms))):
                prefix=out/f'{case}-{i}-{arm}'
                run([a.perf,'stat','-x;','-e','instructions:u','-o',str(prefix.with_suffix('.stat')),'--',str(binary(arm)),case],prefix.with_suffix('.log'))
                fields=next(line.split(';') for line in prefix.with_suffix('.stat').read_text().splitlines() if 'instructions:u' in line)
                assert float(fields[4])>=99
                row[arm]=int(fields[0]);row[arm+'ScheduledPct']=float(fields[4])
                assert json.loads(prefix.with_suffix('.log').read_text().splitlines()[-1])==summary
            row['ratio']=row['candidate']/row['base'];rows.append(row)
        ratios=[r['ratio'] for r in rows[1:]]
        result[case]={'baseMedian':statistics.median(r['base'] for r in rows[1:]),'candidateMedian':statistics.median(r['candidate'] for r in rows[1:]),'pairedMedian':statistics.median(ratios),'pairedRange':[min(ratios),max(ratios)],'pairs':rows}
        (out/'selected.json').write_text(json.dumps(result,indent=2)+'\n');print('SHRINK COUNTS',case,result[case]['pairedMedian'],flush=True)
if a.mode=='ticks':
    out=root/'shrink-ticks';out.mkdir();result={}
    for case in cases:
        summary=expected(case);runs={arm:[] for arm in arms};physical=None;scheduled=[]
        for i in range(-1,5):
            for arm in (arms if i<0 or i%2==0 else list(reversed(arms))):
                prefix=out/f'{case}-{i}-{arm}';counts=prefix.with_suffix('.ticks.json');metadata=prefix.with_suffix('.metadata.json')
                run([str(binary(arm)),case],prefix.with_suffix('.log'),{'STUDY_TICK_COUNTS':str(counts),'SHRINK_METADATA':str(metadata)})
                assert json.loads(prefix.with_suffix('.log').read_text().splitlines()[-1])==summary
                values=json.loads(counts.read_text());world=json.loads(metadata.read_text())
                assert len(values)==len(world)==3200 and [r[0] for r in values]==list(range(3200)) and [r['tick'] for r in world]==list(range(3200))
                physical=world if physical is None else physical;assert physical==world
                for r in values:
                    assert r[2]>0 and r[3]/r[2]>=.99;scheduled.append(100*r[3]/r[2])
                if i>=0:runs[arm].append([r[1]*r[2]/r[3] for r in values])
        tick_medians={arm:[statistics.median(run[tick] for run in runs[arm]) for tick in range(3200)] for arm in arms}
        subsets={'shrinking':[r['tick'] for r in physical if r['radiusAfter']<r['radiusBefore']],
                 'culling':[r['tick'] for r in physical if r['culled']>0],
                 'shrinking_without_cull':[r['tick'] for r in physical if r['radiusAfter']<r['radiusBefore'] and r['culled']==0],
                 'unchanged_radius':[r['tick'] for r in physical if r['radiusAfter']==r['radiusBefore']]}
        assert len(subsets['shrinking'])>2000 and len(subsets['culling'])>=25
        selected={name:{**{arm:distribution([tick_medians[arm][tick] for tick in ticks]) for arm in arms},
            'pairedTickRatios':distribution([tick_medians['candidate'][tick]/tick_medians['base'][tick] for tick in ticks])} for name,ticks in subsets.items()}
        result[case]={'perTickAcrossRuns':'median of five matched measured runs per tick, after warmup pair',
            'minimumScheduledPct':min(scheduled),'subsets':selected,'totalCulled':sum(r['culled'] for r in physical),
            'initialFood':physical[0]['foodBefore'],'aliveRange':[min(r['alive'] for r in physical),max(r['alive'] for r in physical)],
            'coldTick':{arm:tick_medians[arm][0] for arm in arms},'physicalSha256':hashlib.sha256(json.dumps(physical,sort_keys=True).encode()).hexdigest()}
        (out/'selected.json').write_text(json.dumps(result,indent=2)+'\n');print('TICK DISTRIBUTIONS',case,json.dumps(result[case]),flush=True)
rehash()
