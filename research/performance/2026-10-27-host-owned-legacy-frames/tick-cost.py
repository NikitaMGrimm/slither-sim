"""Per-simulation-tick legacy increment, outside construction and drains."""
import argparse,hashlib,json,os,statistics,subprocess
from pathlib import Path
if not __debug__:raise SystemExit('run without -O')
p=argparse.ArgumentParser(description=__doc__);p.add_argument('output',type=Path);p.add_argument('--cpu',type=int,default=1);a=p.parse_args();os.sched_setaffinity(0,{a.cpu});r=a.output.resolve();out=r/'tick-cost';out.mkdir();cases=['official500','official500-half','official500-legacy'];rows=[];ids=None
expected=json.loads((r/'run/traces/selected.json').read_text());pins={arm:json.loads((r/'run'/('native-'+arm)/'pins.json').read_text()) for arm in ['base','candidate']}
def rehash():
 for arm in pins:assert hashlib.sha256((r/'run'/('native-'+arm)/'bin/study_protocols').read_bytes()).hexdigest()==pins[arm]['binarySha256']
rehash()
for i in range(-1,5):
 for arm in (['base','candidate'] if i<0 or i%2==0 else ['candidate','base']):
  costs={}
  for case in cases[i%3:]+cases[:i%3]:
   path=out/f'{i}-{arm}-{case}.json';binary=r/'run'/('native-'+arm)/'bin/study_protocols'
   with path.with_suffix('.log').open('x') as log:subprocess.run([str(binary),case],env=dict(os.environ,RAYON_NUM_THREADS='1',STUDY_TICK_COUNTS=str(path)),stdout=log,stderr=subprocess.STDOUT,check=True)
   assert json.loads(path.with_suffix('.log').read_text().splitlines()[-1])==expected[case]['summary']
   data=[v for v in json.loads(path.read_text()) if v[0]>=120 and v[0]!=720 and v[4]==500];ticks=[v[0] for v in data];ids=ids or ticks;assert ticks==ids and len(data)==853
   assert all(v[2]>0 and v[3]/v[2]>=.99 for v in data);costs[case]=statistics.mean(v[1] for v in data)
  row={'pair':i,'arm':arm,'costs':costs,'halfExtra':(costs[cases[1]]-costs[cases[0]])/250,'allExtra':(costs[cases[2]]-costs[cases[0]])/500};rows.append(row);(out/'rows.json').write_text(json.dumps(rows,indent=2)+'\n');print('TRIPLET',i,arm,flush=True)
selected={}
for arm in ['base','candidate']:
 rr=[v for v in rows if v['pair']>=0 and v['arm']==arm];selected[arm]={'ticks':853,'cases':{c:statistics.median(v['costs'][c] for v in rr) for c in cases},'extra':{k:{'median':statistics.median(v[k] for v in rr),'range':[min(v[k] for v in rr),max(v[k] for v in rr)]} for k in ['halfExtra','allExtra']}}
(out/'selected.json').write_text(json.dumps(selected,indent=2)+'\n');print(json.dumps(selected),flush=True)

rehash()
