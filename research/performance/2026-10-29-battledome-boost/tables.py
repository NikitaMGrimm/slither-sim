"""Print #262 and high-boost rows side by side from their selected results."""
from pathlib import Path
import json,sys
base,boost=[json.loads(Path(p).read_text()) for p in sys.argv[1:]]
def f(x):return '—' if x is None else f'{x:.3f}'
def triplet(v,keys=('p50','p99','max')):return ' / '.join(f(v[k]) for k in keys)
rows=[(name,w) for target in [80,160,320] for name,data in [('#262 A4',base),('High boost',boost)] for w in data['windows'] if w['label']==f'up-{target}']
print('| Bots | Treatment | CPU p50 / p99 / max ms | Deferred | Excess p99 / max ms | Gap jitter p99 ms | Residual stalls >20 / >100 / >250 ms | Alive mean / min | Boost % | Boost births |')
print('| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |')
for name,w in rows:
 c=w['client'];s=c['residualStalls'];b=w['boost']
 print(f"| {w['bots']} | {name} | {triplet(w['worldCpuMsPerTick'])} | {w['deferredTicks']} | {triplet(c['frameExcessMs'],('p99','max'))} | {f(c['gapResidualMs']['p99'])} | {s['20']} / {s['100']} / {s['250']} | {w['alive']['mean']:.2f} / {w['alive']['min']} | {b['aliveTickShare']*100:.2f} | {b['foodBirths']} |")
print('\n| Bots | Treatment | Added-cohort initial p50 / p95 / max ms (n) | Respawn p50 / p95 / max ms (n) | Pending last / maximum observed age ms | Mean length first → last | Food first → last |')
print('| --- | --- | ---: | ---: | ---: | ---: | ---: |')
for name,w in rows:
 s=w['spawns'];j=s['addedCohortInitial'];p=s['respawns'];c=w['census']
 print(f"| {w['bots']} | {name} | {triplet(j,('p50','p95','max'))} ({j['n']}) | {triplet(p,('p50','p95','max'))} ({p['n']}) | {s['pendingAtLastWorkerReports']} / {f(s['pendingMaxMs'])} | {c['meanSegments']['first']:.2f} → {c['meanSegments']['last']:.2f} | {c['food']['first']} → {c['food']['last']} |")
print('\n| High-boost bots | Slow-tail n | Mean world ms | Admission ms / % | Socket ms | Broadcast ms | Minimap ms (nested) | Search / refused ticks | Food-epoch ticks |')
print('| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |')
for w in boost['windows']:
 s=w['slowestCpu1Percent'];p=s['phasesMs'];e=s['eventTicks']
 print(f"| {w['bots']} | {s['n']} | {f(s['worldMeanMs'])} | {f(p['admission'])} / {100*p['admission']/s['worldMeanMs']:.2f} | {f(p['socket'])} | {f(p['broadcast'])} | {f(s['nestedMs']['minimap'])} | {e['admissionSearch']} / {e['searchRefused']} | {e['foodEpoch']} |")
