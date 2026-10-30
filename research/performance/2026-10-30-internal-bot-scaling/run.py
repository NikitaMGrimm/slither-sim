"""Run this study on the authorized idle Linux guest, never on the local PC.

Arguments: execution directory, populations (default 1000 2500 5000 10000).
The directory contains a source snapshot, examples/internal_scaling.rs and its
portable release executable. All generated checkpoints and samples stay there.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import sys
import time


def stable(records, population):
    """First eligible four complete census bins; no performance-based selection."""
    if not records or records[-1]['tick'] * .007 < 300:
        return None
    latest = math.floor(records[-1]['tick'] * .007 / 30)
    bins = {}
    for record in records:
        key = math.floor(record['tick'] * .007 / 30)
        if key < latest:
            bins.setdefault(key, []).append(record)
    keys = sorted(bins)[-4:]
    if len(keys) != 4 or keys != list(range(keys[-1]-3, keys[-1]+1)):
        return None
    selected = [bins[key] for key in keys]
    if any(len(values) < 3 for values in selected):
        return None
    fields = [('meanLength', .05), ('p90Length', .10), ('food', .05)]
    summaries = []
    for values in selected:
        summaries.append({
            'meanLength': statistics.mean(v['census']['meanLength'] for v in values),
            'p90Length': statistics.mean(v['census']['p90Length'] for v in values),
            'food': statistics.mean(v['census']['world']['food'] for v in values),
            'minimumLiving': min(v['census']['world']['living'] for v in values),
        })
    for field, tolerance in fields:
        center = statistics.mean(v[field] for v in summaries)
        if center <= 0 or any(abs(v[field]/center-1) > tolerance for v in summaries):
            return None
    if min(v['minimumLiving'] for v in summaries) < .99*population:
        return None
    return {'firstQualifyingTick':records[-1]['tick'], 'binNumbers':keys,
            'bins':summaries, 'definition':'four complete 30-virtual-second bins; length/food 5%, p90 10%, living >=99%; minimum 300 virtual seconds'}


def digest(path):
    value=hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda:stream.read(1<<20),b''):
            value.update(chunk)
    return value.hexdigest()


def emit(event, **values):
    print(json.dumps({'event':event, 'unix':time.time(), **values}),flush=True)


def execute(root, populations):
    root=root.resolve()
    binary=root/'target/release/examples/internal_scaling'
    pins=root/'measurement-pins.json'
    assert pins.exists(), 'Freeze source/build pins before running'
    frozen=json.loads(pins.read_text())
    assert digest(binary)==frozen['binarySha256']
    output=root/'results'
    output.mkdir(exist_ok=False)
    for n in populations:
        destination=output/str(n)
        destination.mkdir()
        checkpoint=destination/'fixture'
        warm_log=destination/'warm.jsonl'
        began=time.monotonic()
        records=[]
        acceptance=None
        with warm_log.open('w') as out:
            child=subprocess.Popen([str(binary),'warm',str(checkpoint),str(n),'257143'],
                                   stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True)
            try:
                for line in child.stderr:
                    out.write(line);out.flush()
                    row=json.loads(line)
                    if row['kind']=='warm':
                        records.append(row)
                        emit('warm-progress',population=n,tick=row['tick'],
                             virtualSeconds=row['tick']*.007,
                             meanLength=row['census']['meanLength'],food=row['census']['world']['food'],
                             elapsedSeconds=time.monotonic()-began)
                        if acceptance is None:
                            acceptance=stable(records,n)
                            if acceptance is not None:
                                (checkpoint/'STOP_WARM').write_text('first stability screen passed\n')
                assert child.wait()==0, 'warmup failed'
            finally:
                if child.poll() is None:
                    child.terminate();child.wait(timeout=15)
        qualification={'population':n,'stabilized':acceptance is not None,'acceptance':acceptance,
                       'elapsedSeconds':time.monotonic()-began,
                       'fixtureSha256':digest(checkpoint/'world.json'),
                       'metaSha256':digest(checkpoint/'meta.json')}
        (destination/'warm-qualification.json').write_text(json.dumps(qualification,indent=2)+'\n')
        emit('warm-finished',**qualification)
        if acceptance is None:
            emit('censored-population',population=n,reason='frozen warmup limit; no stabilized timing row')
            continue
        # Verification and replay-tape generation precede all counted timing.
        verified={}
        for mode in ['serial','parallel']:
            path=destination/f'verify-{mode}.json'
            subprocess.run([str(binary),'measure',str(checkpoint),mode,'512','verify',str(path)],check=True)
            verified[mode]=json.loads(path.read_text())
        for field in ['actionSha256','finalStateSha256']:
            assert verified['serial'][field]==verified['parallel'][field],field
        assert digest(destination/'verify-serial.tape')==digest(destination/'verify-parallel.tape')
        shutil.copyfile(destination/'verify-serial.tape',checkpoint/'tape.bin')
        reference=verified['serial']['finalStateSha256']
        # Keep controller work outside child timing brackets and no concurrent analysis.
        for pair in range(-1,5):
            order=['serial','parallel'] if pair%2==0 else ['parallel','serial']
            order=(['simulation']+order) if pair%2 else (order+['simulation'])
            for mode in order:
                path=destination/f'pair-{pair}-{mode}.json'
                emit('measurement-start',population=n,pair=pair,mode=mode)
                subprocess.run([str(binary),'measure',str(checkpoint),mode,'512','plain',str(path)],check=True)
                row=json.loads(path.read_text())
                assert row['finalStateSha256']==reference,(n,pair,mode,'state mismatch')
                emit('measurement-end',population=n,pair=pair,mode=mode)
        for mode in ['serial','parallel']:
            path=destination/f'profile-{mode}.json'
            subprocess.run([str(binary),'measure',str(checkpoint),mode,'512','profile',str(path)],check=True)
            assert json.loads(path.read_text())['finalStateSha256']==reference
        emit('population-complete',population=n)
    assert digest(binary)==frozen['binarySha256']
    emit('complete')


if __name__=='__main__':
    execute(Path(sys.argv[1]),[int(v) for v in sys.argv[2:]] or [1000,2500,5000,10000])
