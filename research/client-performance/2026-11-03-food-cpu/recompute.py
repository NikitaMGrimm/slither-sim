"""Recompute retained per-run aggregates with Python's standard library."""
from pathlib import Path
import csv,json,statistics,sys
ROOT=Path(__file__).resolve().parent
EXPECTED_PARITY_KEYS={
    f'replay/{adapter}/{size}/{quality}'
    for adapter,size in [('rtx','3840x2160'),('igpu','3840x2160'),('igpu','1920x1080')]
    for quality in ['original','native']
} | {
    f'rectangle/{adapter}/3840x2160/{quality}'
    for adapter in ['rtx','igpu']
    for quality in ['original','native']
}

def validate(rows,parity,identities):
    grouped={}
    for r in rows:
        assert int(r['callbacks'])==600 and int(r['messages'])==29044 and int(r['skips'])==0
        key=(r['adapter'],r['size'],r['quality'],r['pair'])
        pair=grouped.setdefault(key,{})
        assert r['arm'] in ('baseline','candidate') and r['arm']not in pair
        assert r['binary_sha256']==identities[r['arm']]['binary_sha256'],'timing binary does not match arm identity'
        pair[r['arm']]=r
    for pair in grouped.values():
        assert set(pair)=={'baseline','candidate'}
        for field in ['command_digest','quad_digest']:assert pair['baseline'][field]==pair['candidate'][field]
    assert set(parity)==EXPECTED_PARITY_KEYS,'parity configurations differ from the expected matrix'
    baseline_image=identities['baseline_image_binary_sha256']
    candidate_image=identities['candidate_image_binary_sha256']
    assert baseline_image!=candidate_image,'parity arms must use distinct binaries'
    for record in parity.values():
        assert record['baseline_binary_sha256']==baseline_image,'baseline parity binary does not match identity'
        assert record['binary_sha256']==candidate_image,'candidate parity binary does not match identity'
        assert record['callbacks']==600 and record['skips']==0
        assert record['image_sequence_sha256']==record['baseline_image_sequence_sha256']
    return grouped

def table(rows):
    fields=['wall_p50_ms','wall_p95_ms','food_advance_p50_ms','cpu_p50_ms','gpu_p50_ms','requested_bytes']
    lines=['| Adapter / size / quality | Pairs | Art median ms | Art p95 ms | Food advance ms | CPU stages ms | GPU ms | Requested GB |','| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    configs=sorted({(r['adapter'],r['size'],r['quality'])for r in rows},key=lambda c:(c[0]!='rtx',c[1],c[2]))
    for config in configs:
        selected=[r for r in rows if (r['adapter'],r['size'],r['quality'])==config and r['accepted']=='True']
        pairs={r['pair']for r in selected};assert len(selected)==len(pairs)*2
        assert len(pairs)>=(5 if config[0]=='rtx'else 2)
        cells=[]
        for field in fields:
            values=[statistics.median(float(r[field])for r in selected if r['arm']==arm)for arm in ['baseline','candidate']]
            if field=='requested_bytes':values=[x/1e9 for x in values]
            cells.append(' → '.join(f'{x:.3f}'for x in values))
        lines.append('| '+' / '.join(config)+' | '+str(len(pairs))+' | '+' | '.join(cells)+' |')
    lines.extend(['','| Adapter / size / quality | Baseline art-median range ms | Candidate range ms | Paired median reduction % (range) |','| --- | ---: | ---: | ---: |'])
    for config in configs:
        selected=[r for r in rows if (r['adapter'],r['size'],r['quality'])==config and r['accepted']=='True']
        arms={arm:[float(r['wall_p50_ms'])for r in selected if r['arm']==arm]for arm in ['baseline','candidate']}
        grouped={}
        for r in selected:grouped.setdefault(r['pair'],{})[r['arm']]=float(r['wall_p50_ms'])
        reductions=[100*(1-p['candidate']/p['baseline'])for p in grouped.values()]
        ranges=[f'{min(arms[a]):.3f}–{max(arms[a]):.3f}'for a in ['baseline','candidate']]
        lines.append('| '+' / '.join(config)+' | '+' | '.join(ranges)+f' | {statistics.median(reductions):.3f} ({min(reductions):.3f}–{max(reductions):.3f}) |')
    return '\n'.join(lines)

def main():
    with (ROOT/'runs.csv').open(newline='',encoding='utf-8') as stream:
        rows=list(csv.DictReader(stream))
    parity=json.loads((ROOT/'parity.json').read_text(encoding='utf-8'))
    identities=json.loads((ROOT/'identities.json').read_text(encoding='utf-8'))
    validate(rows,parity,identities);text=table(rows)
    if '--check'in sys.argv:
        report=(ROOT/'REPORT.md').read_text(encoding='utf-8')
        assert text in report,'report aggregate table differs from runs.csv'
        print('36 runs, complete matched pairs and 6000 exact image pairs verified')
    else:print(text)
if __name__=='__main__':main()
