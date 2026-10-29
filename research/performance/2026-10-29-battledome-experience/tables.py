"""Render report tables from the selected aggregates; no private raw input."""
import json,sys
from pathlib import Path
d=json.loads(Path(sys.argv[1]).read_text())
def f(v):return '—' if v is None else f'{v:.3f}'
def triple(v,keys=('p50','p99','max')):return ' / '.join(f(v[k]) for k in keys)
print('| Bots | World CPU p50 / p99 / max ms | Deferred | Header excess p50 / p99 / max ms | Gap residual p99 ms | Residual stalls >100 / >250 ms | Alive mean / min | Boost % | Boost births |')
print('| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |')
for w in d['windows']:
 g=w['client']['frameExcessMs'];st=w['client']['residualStalls'];alive=w['alive'];boost=w['boost']
 print(f"| {w['label']} | {triple(w['worldCpuMsPerTick'])} | {w['deferredTicks']} | {triple(g)} | {f(w['client']['gapResidualMs']['p99'])} | {st['100']} / {st['250']} | {alive['mean']:.2f} / {alive.get('min','—')} | {boost['aliveTickShare']*100:.2f} | {boost['foodBirths']} |")
print('\n| Bots | Raw frame gap p50 / p99 / max ms | Raw gaps >100 / >250 ms (not automatically stalls) |')
print('| --- | ---: | ---: |')
for w in d['windows']:
 g=w['client']['gapsMs'];st=w['client']['stalls']
 print(f"| {w['label']} | {triple(g)} | {st['100']} / {st['250']} |")
print('\n| Bots | Added-cohort initial p50 / p95 / max ms (n) | Respawn p50 / p95 / max ms (n) | Pending last / max age ms | Deaths <3 s / <10 s / all completed |')
print('| --- | ---: | ---: | ---: | ---: |')
for w in d['windows']:
 s=w['spawns'];a=s.get('addedCohortInitial',s['initialCompletedThroughWindow']);b=s['respawns']
 print(f"| {w['label']} | {triple(a,('p50','p95','max'))} ({a['n']}) | {triple(b,('p50','p95','max'))} ({b['n']}) | {s['pendingAtLastWorkerReports']} / {f(s['pendingMaxMs'])} | {s['deathsUnder3s']} / {s['deathsUnder10s']} / {s['lifeSeconds']['n']} |")
print('\n| Bots | Slow 1% CPU mean ms | Admission % | Broadcast % | Socket % | Food epoch current/prior ticks | Searches / strict / fallback / refused (all ticks) |')
print('| --- | ---: | ---: | ---: | ---: | ---: | --- |')
for w in d['windows']:
 t=w['slowestCpu1Percent'];p=t['phasesMs'];total=t['worldMeanMs'];e=w['allTicks']['eventTicks']
 print(f"| {w['label']} | {f(total)} | {p['admission']/total*100:.1f} | {p['broadcast']/total*100:.1f} | {p['socket']/total*100:.1f} | {t.get('foodEpochOrPreviousTick',0)} / {t['n']} | {e['admissionSearch']} / {e['strictAccepted']} / {e['fallbackAccepted']} / {e['searchRefused']} |")
