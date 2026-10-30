"""Run the separate center-seeking comparison after the A4 series has ended."""
import json
import math
from pathlib import Path
import statistics
import subprocess
import sys
import time
from run import digest, emit


def stable_storm(records,n):
    if not records or records[-1]['tick']*.007<300:return None
    latest=math.floor(records[-1]['tick']*.007/30)
    bins={}
    for row in records:
        key=math.floor(row['tick']*.007/30)
        if key<latest:bins.setdefault(key,[]).append(row)
    keys=sorted(bins)[-4:]
    if len(keys)!=4 or keys!=list(range(keys[-1]-3,keys[-1]+1)):return None
    means=[]
    for key in keys:
        rows=bins[key]
        if len(rows)<3:return None
        means.append({'living':statistics.mean(v['census']['world']['living'] for v in rows),
                      'minimumLiving':min(v['census']['world']['living'] for v in rows),
                      'meanLength':statistics.mean(v['census']['meanLength'] for v in rows),
                      'deathsPerVirtualSecond':statistics.mean(v['deathsPerVirtualSecond'] for v in rows),
                      'admissionsPerVirtualSecond':statistics.mean(v['admissionsPerVirtualSecond'] for v in rows)})
    if min(v['minimumLiving'] for v in means)<.99*n:return None
    for field,tolerance in [('living',.01),('meanLength',.10),('deathsPerVirtualSecond',.15),('admissionsPerVirtualSecond',.15)]:
        center=statistics.mean(v[field] for v in means)
        if center<=0 or any(abs(v[field]/center-1)>tolerance for v in means):return None
    return {'firstQualifyingTick':records[-1]['tick'],'binNumbers':keys,'bins':means,
            'definition':'four complete 30-virtual-second bins; population 1%, length 10%, positive death/admission rates 15%; minimum 300 virtual seconds'}


def execute(root):
    root=Path(root).resolve()
    pins=json.loads((root/'botstorm-build.json').read_text())
    binary=root/'bin/internal_botstorm'
    assert digest(binary)==pins['binarySha256']
    destination=root/'botstorm-results'
    destination.mkdir(exist_ok=False)
    for n in [12000,6000,9000]:
        if n!=12000:
            primary=json.loads((destination/'12000/qualification.json').read_text())
            if not primary['stabilized'] or primary['completeSeconds']>=1800:
                emit('botstorm-optional-not-measured',population=n,reason='12k did not complete stabilized counting within 30 minutes')
                continue
        output=destination/str(n);output.mkdir()
        fixture=output/'fixture'
        began=time.monotonic();records=[];acceptance=None
        with (output/'warm.jsonl').open('w') as log:
            child=subprocess.Popen([str(binary),'warm',str(fixture),str(n)],stdout=subprocess.DEVNULL,
                                   stderr=subprocess.PIPE,text=True)
            try:
                for line in child.stderr:
                    log.write(line);log.flush()
                    row=json.loads(line)
                    if row['kind']=='warm':
                        records.append(row)
                        emit('botstorm-warm',population=n,tick=row['tick'],meanLength=row['census']['meanLength'],
                             deathsPerVirtualSecond=row['deathsPerVirtualSecond'],
                             elapsedSeconds=time.monotonic()-began)
                        if acceptance is None:
                            acceptance=stable_storm(records,n)
                            if acceptance is not None:(fixture/'STOP_WARM').write_text('first stability screen passed\n')
                assert child.wait()==0,'botstorm warmup failed'
            finally:
                if child.poll() is None:child.terminate();child.wait(timeout=15)
        receipt={'population':n,'stabilized':acceptance is not None,'acceptance':acceptance,
                 'fixtureSha256':digest(fixture/'world.json'),'metaSha256':digest(fixture/'meta.json')}
        if acceptance is not None:
            reference=None;reference_work=None
            for window in range(-1,5):
                path=output/f'window-{window}.json'
                emit('botstorm-measurement-start',population=n,window=window)
                subprocess.run([str(binary),'measure',str(fixture),'512','plain',str(path)],check=True)
                row=json.loads(path.read_text())
                work=tuple(sum(s[key] for s in row['samples']) for key in ['admitted','refused','poses','deaths'])
                if reference is None:reference=row['finalStateSha256'];reference_work=work
                assert row['finalStateSha256']==reference and work==reference_work
                emit('botstorm-measurement-end',population=n,window=window)
            path=output/'profile.json'
            subprocess.run([str(binary),'measure',str(fixture),'512','profile',str(path)],check=True)
            row=json.loads(path.read_text())
            assert row['finalStateSha256']==reference
            assert tuple(sum(s[key] for s in row['samples']) for key in ['admitted','refused','poses','deaths'])==reference_work
            receipt['finalStateSha256']=reference
        receipt['completeSeconds']=time.monotonic()-began
        (output/'qualification.json').write_text(json.dumps(receipt,indent=2)+'\n')
        emit('botstorm-population-complete',**receipt)
    assert digest(binary)==pins['binarySha256']
    emit('botstorm-complete')


if __name__=='__main__':execute(sys.argv[1])
