"""Reduce live/coarse logs using the linked #248 percentile definition.

Inputs are raw external run directories plus selected.json; no production writes.
Primary and diagnostic windows remain distinct. Output contains cited aggregates.
"""
from pathlib import Path
import sys,json,math,statistics,hashlib
r=Path(sys.argv[1]).resolve()
def due(tick):return math.floor(tick*.007/.504)>math.floor((tick-1)*.007/.504)
def q(xs):
 xs=sorted(xs);assert xs
 return {'p50':xs[math.ceil((len(xs)-1)*.5)],'p99':xs[math.ceil((len(xs)-1)*.99)],'max':xs[-1],'mean':statistics.mean(xs)}
PHASES=['movement','collisionBuild','collisionQuery','deathFood','pickupFoodCache','prey','ambient','admission','broadcast']
def summarize(rows):
 if not rows:return None
 result={'ticks':len(rows),'worldCpuMsPerTick':q([x['world'] for x in rows]),'simulationCpuMsPerTick':q([x['sim'] for x in rows]),'socketCpuMsPerTick':q([x['io'] for x in rows]),'maintenanceFraction':statistics.mean(x['food'] for x in rows),'postReportFraction':statistics.mean(x['postReport'] for x in rows)}
 if all('phases' in x for x in rows):
  result['phaseMeanCpuMsPerTick']={k:statistics.mean(x['phases'][i] for x in rows) for i,k in enumerate(PHASES)}
  result['bracketedPhysicsMeanCpuMsPerTick']=sum(result['phaseMeanCpuMsPerTick'][p] for p in PHASES[:7])
  result['unbracketedSimulationMeanCpuMsPerTick']=statistics.mean(x['sim']-sum(x['phases'][:9]) for x in rows)
 return result

def describe(rows):
 tail=sorted(rows,key=lambda x:x['world'],reverse=True)[:math.ceil(len(rows)*.01)]
 by_tick={x['tick']:x for x in rows}
 return {'all':summarize(rows),'maintenance':summarize([x for x in rows if x['food']]),'ordinary':summarize([x for x in rows if not x['food']]),'postReport':summarize([x for x in rows if x['postReport']]),'ordinaryWithoutPostReport':summarize([x for x in rows if not(x['food'] or x['postReport'])]),'slowestOnePercent':summarize(tail),'tailNeighborSocketMeansMs':{str(d):statistics.mean(by_tick[x['tick']+d]['io'] for x in tail if x['tick']+d in by_tick) for d in [-1,0,1]}}

def read_run(name):
 root=r/'runs'/name;meta=json.loads((root/'summary.json').read_text());cs={};bs={}
 for line in (root/'host.log').read_text().splitlines():
  if line.startswith('CAPACITY '):v=json.loads(line[9:]);bs[v['tick']]=v
  elif line.startswith('CENSUS '):v=json.loads(line[7:]);cs[v['tick']]=v
 return meta,cs,bs

def rows_in(cs,bs,start,end,detail=False):
 rows=[]
 for t,b in sorted(bs.items()):
  if t-100 not in cs or cs[t-100]['unix']<start or cs[t]['unix']>end:continue
  assert len(b['rows'])==100
  if detail and len(b.get('phaseRows',[]))!=100:continue
  if not detail:assert not b.get('phaseRows'), 'detail export contaminated primary'
  for i,(sim,io,wall) in enumerate(b['rows']):
   tick=t-99+i;x={'tick':tick,'sim':sim/1e6,'io':io/1e6,'world':(sim+io)/1e6,'food':due(tick),'postReport':tick%100==1}
   if detail:
    assert x['food']==b['foodFlags'][i],('food cadence mismatch',tick)
    x['phases']=[v/1e6 for v in b['phaseRows'][i][:9]]
   rows.append(x)
  if detail:
   assert all(sum(v[i] for v in b['phaseRows'])==b['stages'][i] for i in range(9)), 'phase delta reconciliation'
 return rows

def main():
 selected=json.loads((r/'selected.json').read_text());out={'limits':'Fresh asynchronous live cohorts, descriptive only. Primary per-tick phases unavailable; coarse export adds diagnostic overhead. Neighbor bins do not reassign socket CPU. Post-report ticks retain observer CPU.','cadenceValidatedTicks':{'base':0,'candidate':0},'actualFoodGapCounts':{'base':{},'candidate':{}},'cases':{}}
 for name,accepted in selected['cases'].items():
  meta,cs,bs=read_run(name)
  # Validate all complete actual-flag blocks, including those near coarse edges.
  for t,b in bs.items():
   if len(b.get('foodFlags',[]))==100:
    for i,flag in enumerate(b['foodFlags']):assert flag==due(t-99+i)
    out['cadenceValidatedTicks'][meta['arm']]+=100
  actual=[tick-99+i for tick,block in sorted(bs.items()) if len(block.get('foodFlags',[]))==100 for i,flag in enumerate(block['foodFlags']) if flag]
  for before,after in zip(actual,actual[1:]):
   gaps=out['actualFoodGapCounts'][meta['arm']];key=str(after-before);gaps[key]=gaps.get(key,0)+1
  h=meta['holds'][0];rows=rows_in(cs,bs,h['startUnix'],h['endUnix'])
  assert len(rows)==accepted['holds'][0]['ticks']
  assert q([x['world'] for x in rows])==accepted['holds'][0]['worldCpuMsPerTick']
  coarse=rows_in(cs,bs,meta['coarseStartUnix'],meta['coarseEndUnix'],True)
  check=r/(name+'-coarse.json')
  out['cases'][name]={'arm':meta['arm'],'primary':describe(rows),'coarse':describe(coarse) if coarse else None,'coarseQualityPasses':check.exists()}
 assert all(n>=2000 for n in out['cadenceValidatedTicks'].values())
 (r/'tail-selected.json').write_text(json.dumps(out,indent=2)+'\n')
 # Keep whole-source and binary identities, never the per-file overlay map.
 for arm,pin in selected['pins'].items():
  raw=json.loads((r/arm/'pins.json').read_text())
  files=[]
  for p in sorted((r/arm/'source/src').rglob('*.rs'),key=lambda p:p.relative_to(r/arm/'source').as_posix()):
   rel=p.relative_to(r/arm/'source').as_posix().encode();files.append(rel+b'\0'+p.read_bytes()+b'\0')
  selected['pins'][arm]={k:v for k,v in pin.items() if k!='overlays'}|{'instrumentedRustSourceSha256':hashlib.sha256(b''.join(files)).hexdigest()}
 (r/'publication/results.json').write_text(json.dumps(selected,indent=2)+'\n')
 published=json.loads(json.dumps(out))
 for case in published['cases'].values():
  for key in ['all','postReport','ordinaryWithoutPostReport']:case['primary'].pop(key,None)
  case['coarse']={'ticks':case['coarse']['all']['ticks'],'maintenanceTicks':case['coarse']['maintenance']['ticks'],'slowestOnePercent':case['coarse']['slowestOnePercent']}
 (r/'publication/tail-results.json').write_text(json.dumps(published,indent=2)+'\n')
 print('PRIMARY AND COARSE TAIL REDUCTION COMPLETE')
if __name__=='__main__':main()
