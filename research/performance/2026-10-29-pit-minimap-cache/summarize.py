"""Recompute report instruction and CPU tables from selected per-pair results.

Usage: python summarize.py SELECTED_JSON
All five counted pairs are retained; the discarded warmup is excluded.
CPU quantiles are medians and ranges of per-window quantiles, never summed.
"""
import json,statistics,sys
from pathlib import Path

data=json.loads(Path(sys.argv[1]).read_text())
def spread(values):
    return {'median':statistics.median(values),'min':min(values),'max':max(values)}
output={}
for protocol,item in data.items():
    pairs=[p for p in item['pairs'] if not p['warmup']]
    assert len(pairs)==5 and [p['pair'] for p in pairs]==list(range(5))
    groups={}
    for group in pairs[0]['base']:
        if not all(p[a][group]['n'] for p in pairs for a in ['base','candidate']):continue
        groups[group]={
            'instructionsChangePercent':spread([(p['candidate'][group]['instructionsMean']/p['base'][group]['instructionsMean']-1)*100 for p in pairs]),
            'cpuMs':{a:{q:spread([p[a][group]['cpuMs'][q] for p in pairs]) for q in ['p50','p99','max']} for a in ['base','candidate']},
            'ticksPerWindow':{a:sorted({p[a][group]['n'] for p in pairs}) for a in ['base','candidate']}}
    output[protocol]=groups
print(json.dumps(output,indent=2))
