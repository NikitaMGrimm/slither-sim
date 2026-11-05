"""Recompute selected quantile tables; full journals remain outside Git."""
import argparse
import csv
import json
import math
import re
import statistics
from pathlib import Path
ROOT=Path(__file__).parent

def ms(v): return f"{float(v)/1e6:.3f}"
def block(report,name,table):
    pattern=rf"(<!-- {name} -->\n).*?(\n<!-- /{name} -->)"
    assert len(re.findall(pattern,report,re.S))==1
    return re.sub(pattern,lambda m:m[1]+table+m[2],report,flags=re.S)
def recompute():
    runs=list(csv.DictReader((ROOT/'runs.csv').open(encoding='utf-8')))
    identities=json.loads((ROOT/'identities.json').read_text(encoding='utf-8'))
    assert identities['baseline']['exe_sha256']!=identities['candidate']['exe_sha256']
    supplemental=identities['supplemental_cell_binaries']
    assert supplemental['baseline-cell']['exe_sha256']!=supplemental['candidate-cell']['exe_sha256']
    assert all(re.fullmatch('[a-f0-9]{64}',entry['exe_sha256']) for entry in [identities['baseline'],identities['candidate'],*supplemental.values()])
    assert len(runs)==240
    keys=set()
    for r in runs:
        key=tuple(r[x] for x in ['round','arm','backend','bots','mode','level'])
        assert key not in keys; keys.add(key)
        assert int(r['frames'])==30 and int(r['failures'])==0
        for k,v in r.items():
            if k.endswith('_ns'): assert math.isfinite(float(v)) and float(v)>=0
    assert keys=={(str(n),a,b,str(c),m,l) for n in range(1,6) for a in ['baseline','candidate'] for b in ['vulkan','dx12'] for c in [100,500] for m in ['Native','Original'] for l in ['default','mid','out']}
    report=(ROOT/'REPORT.md').read_text(encoding='utf-8')
    lines=['| Adapter / bots / view | CPU total p95 before → after ms | Submit p95 before → after ms | Before / after run p95 range ms |','| --- | ---: | ---: | ---: |']
    for backend in ['vulkan','dx12']:
        for bots in ['100','500']:
            for level in ['mid','out']:
                sides=[[r for r in runs if (r['backend'],r['bots'],r['mode'],r['level'],r['arm'])==(backend,bots,'Native',level,arm)] for arm in ['baseline','candidate']]
                vals=[[float(r['cpu_p95_ns']) for r in side] for side in sides]
                sub=[[float(r['submit_p95_ns']) for r in side] for side in sides]
                lines.append(f"| {backend} / {bots} / {level} | {ms(statistics.median(vals[0]))} → {ms(statistics.median(vals[1]))} | {ms(statistics.median(sub[0]))} → {ms(statistics.median(sub[1]))} | {ms(min(vals[0]))}–{ms(max(vals[0]))} / {ms(min(vals[1]))}–{ms(max(vals[1]))} |")
    report=block(report,'TIMING','\n'.join(lines))
    rows=list(csv.DictReader((ROOT/'profile.csv').open(encoding='utf-8')))
    lines=['| Adapter / bots / view | Prepare / HUD / sync / submit p95 ms | GPU p95 ms | CPU stages ranked by p95 ms |','| --- | ---: | ---: | --- |']
    for backend in ['vulkan','dx12']:
        for bots in ['100','500']:
            for level in ['mid','out']:
                phases={r['stage']:float(r['p95_ns']) for r in rows if (r['backend'],r['bots'],r['level'])==(backend,bots,level)}
                assert all(k in phases for k in ['prepare_ns','hud_ns','sync_ns','submit_ns','gpu_ns'])
                ranked=sorted([(k,v) for k,v in phases.items() if k.endswith('_end') and k not in ['submit_end','sync_end','build_end']],key=lambda x:-x[1])[:4]
                lines.append(f"| {backend} / {bots} / {level} | {' / '.join(ms(phases[k]) for k in ['prepare_ns','hud_ns','sync_ns','submit_ns'])} | {ms(phases['gpu_ns'])} | {', '.join(k+': '+ms(v) for k,v in ranked)} |")
    report=block(report,'PROFILE','\n'.join(lines))
    assert (ROOT/'cells.csv').exists(), 'missing matched-cell data'
    if (ROOT/'cells.csv').exists():
        cells=list(csv.DictReader((ROOT/'cells.csv').open(encoding='utf-8')))
        assert len(cells)==80
        keys=[tuple(r[k] for k in ['round','arm','backend','bots','level']) for r in cells]
        assert len(set(keys))==80
        assert set(keys)=={(str(n),a,b,c,l) for n in range(1,6) for a in ['baseline-cell','candidate-cell'] for b in ['vulkan','dx12'] for c in ['100','500'] for l in ['mid','out']}
        assert all(int(r['frames'])==30 and re.fullmatch('[a-f0-9]{64}',r['raw_sha256']) for r in cells)
        lines=['| Adapter / bots / view | CPU p95 before → after ms | Submit p95 before → after ms | Submit paired p95 delta range ms |','| --- | ---: | ---: | ---: |']
        for backend in ['vulkan','dx12']:
            for bots in ['100','500']:
                for level in ['mid','out']:
                    sides=[sorted([r for r in cells if (r['backend'],r['bots'],r['level'],r['arm'])==(backend,bots,level,arm)],key=lambda r:int(r['round'])) for arm in ['baseline-cell','candidate-cell']]
                    cpu=[[float(r['cpu_p95_ns']) for r in side] for side in sides]
                    sub=[[float(r['submit_p95_ns']) for r in side] for side in sides]
                    delta=[b-a for a,b in zip(*sub)]
                    lines.append(f"| {backend} / {bots} / {level} | {ms(statistics.median(cpu[0]))} → {ms(statistics.median(cpu[1]))} | {ms(statistics.median(sub[0]))} → {ms(statistics.median(sub[1]))} | {ms(min(delta))} to {ms(max(delta))} |")
        report=block(report,'CELLS','\n'.join(lines))
    images=json.loads((ROOT/'image-controls.json').read_text(encoding='utf-8'));assert len(images)==72
    configs=set()
    for x in images:
        assert re.fullmatch('[a-f0-9]{64}',x['before']) and x['before']==x['after']
        assert x['equal'] and x['changed_bytes']==0 and x['max_byte_difference']==0
        m=re.fullmatch(r'(100|500)-3840-(default|mid|out)-(Native|Original)-f(10|25|39)\.png',Path(x['file']).name);assert m
        key=(x['backend'],*m.groups());assert key not in configs;configs.add(key)
    assert configs=={(b,c,l,m,f) for b in ['vulkan','dx12'] for c in ['100','500'] for l in ['default','mid','out'] for m in ['Native','Original'] for f in ['10','25','39']}
    return report
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');a=p.parse_args();r=recompute()
    if a.check: assert r==(ROOT/'REPORT.md').read_text(encoding='utf-8'),'stale report'
    else: (ROOT/'REPORT.md').write_text(r,encoding='utf-8',newline='\n')
    print('Selected timing, profile, image and identity checks passed.')
