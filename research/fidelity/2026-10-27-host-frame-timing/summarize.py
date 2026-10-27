"""Select bounded summaries and all-attempt identities; no raw bytes in Git."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics

from analyze import capture, traces

p = argparse.ArgumentParser()
p.add_argument('raw', type=Path)
p.add_argument('out', type=Path)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=True)
runs = []
for config in sorted(a.raw.rglob('host.json')):
    root = config.parent
    meta = root/'run.json'
    r = dict(id=root.relative_to(a.raw).as_posix(), run=json.loads(meta.read_text()) if meta.exists() else None,
             host=(root/'host.log').read_text(), config=json.loads((root/'host.json').read_text()))
    archive = root/'capture/bot0.jsonl.gz'
    if archive.exists():
        r.update(capture=capture(archive), traces=traces(root))
    r['logs'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in root.glob('*.log')}
    runs.append(r)

aggregate = {}
for system in ('windows','wsl'):
    for side in ('baseline','candidate'):
        cohort = [r for r in runs if r['id'].startswith(f'{system}-v2-series')
                  and r['id'].endswith(side) and r['run'] is not None]
        if not cohort:
            continue
        n = sum(r['capture']['receipt']['n'] for r in cohort)
        adj = sum(r['capture']['eligibleAdjacentPairs'] for r in cohort)
        row = dict(repetitions=len(cohort), gaps=n,
            receipt={**{q: statistics.median(r['capture']['receipt'][q] for r in cohort)
                        for q in ('p50','p90','p99')},
                     'max':max(r['capture']['receipt']['max'] for r in cohort)},
            header={**{q:statistics.median(r['capture']['header'][q] for r in cohort)
                    for q in ('p50','p90','p99')},
                    'max':max(r['capture']['header']['max'] for r in cohort)},
            over14Percent=100*sum(r['capture']['over14'] for r in cohort)/n,
            burstPercent=100*sum(r['capture']['burstPairs'] for r in cohort)/max(1,adj))
        for key, get in {
            'handoffP99':lambda r:r['traces']['handoffToRelease']['p99'],
            'absGapMinusHeaderP99':lambda r:r['capture']['absGapMinusHeader']['p99'],
            'wakeP99':lambda r:r['traces']['simulation']['lateness']['p99'],
            'releaseP99':lambda r:r['traces']['release']['lateness']['p99'],
            'advanceP99':lambda r:r['traces']['simulation']['advanceIncludingEncode']['p99'],
            'cpuCorePercent':lambda r:100*r['run']['cpuSeconds']/r['run']['wallSeconds'],
        }.items():
            values = [get(r) for r in cohort]
            row[key] = dict(median=statistics.median(values), min=min(values), max=max(values))
        aggregate[f'{system}-{side}'] = row

official = json.loads((a.raw/'official-near.json').read_text())
stock = json.loads((a.raw/'official-177.json').read_text())
result = dict(schemaVersion=1, dataOrigin='mixed-labelled',
    definitions='Nearest-rank per-run quantiles; aggregate quantiles are medians of runs; maximum receipt gap is largest across runs. CPU includes startup/shutdown. Trace window starts at host wall 6s; receipt intervals wholly after setup+5s.',
    runs=runs, aggregate=aggregate, officialNear=official, official177=stock)
with (a.out/'results.json').open('x',encoding='utf-8',newline='\n') as f:
    json.dump(result,f,indent=2)
    f.write('\n')
for k,v in aggregate.items():
    print(k,json.dumps(v))
