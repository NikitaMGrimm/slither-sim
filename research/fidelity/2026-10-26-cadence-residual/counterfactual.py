"""Synthetic timestamp occupancy sensitivity, never a physical-world replay."""
import csv,gzip,json,math,random,statistics
from collections import defaultdict,Counter
from pathlib import Path
import analyze as analysis
analysis.configure()
from analyze import OUT,INPUT,save,q,cad,pct,sha

data=json.loads((OUT/'results.json').read_text())
def loadwindows(alias):
    allowed={(str(w['epoch']),w['start']) for w in json.loads((INPUT/(alias+'.windows.json')).read_text()) if w['bin']=='6-10'}
    ww=defaultdict(list)
    with gzip.open(OUT/(alias+'.events.jsonl.gz'),'rt') as f:
        for line in f:
            v=json.loads(line);key=(v['epoch'],v['t']//1000*1000)
            if key in allowed:ww[key].append(v)
    return [dict(start=k[1],frames=ff,peers=statistics.mean(f['peers'] for f in ff)) for k,ff in ww.items()]

def group(p):
    if p['kind']=='fam' and not p['own']:return 'mass'
    if p['kind'] in ('anchor','tail') and not p['own']:return 'body'
    if p['c'] in 'FbfwcC<':return 'food'
    return 'rest'

def occupancy(w,replace=(),donor=None,offset=0,phase=False,rng=None):
    start=w['start'];lo=math.ceil(start/7);hi=math.ceil((start+1000)/7);size=hi-lo
    keep=set();motion=defaultdict(set)
    for f in w['frames']:
        tick=round(f['t']/7)-lo
        if not 0<=tick<size:continue
        for p in f['info']:
            if group(p) in replace:continue
            if phase and p['kind']=='rot' and not p['own']:motion[p['sid']].add(tick)
            else:keep.add(tick)
    if phase:
        for vv in motion.values():
            shift=rng.randrange(6);keep.update((t+shift)%size for t in vv)
    if donor:
        # Map relative donor milliseconds to candidate's grid. Same frame stays together.
        for f in donor['frames']:
            if any(group(p) in replace for p in f['info']):keep.add((round((f['t']-donor['start'])/7)+offset)%size)
    return cad(sorted(t*7 for t in keep))

official=[x for x in data if x['id'].startswith('official')]
donors={x['id']:loadwindows(x['id']) for x in official}
result=[]
for x in data:
    if not x['id'].startswith('bots-500'):continue
    ww=loadwindows(x['id']);allout={};phaseout=[]
    for familyset in [('mass',),('body',),('food',),('mass','body','food')]:
        name='+'.join(familyset);trials=[];bydonor={}
        for offalias,dd in donors.items():
            values=[]
            for trial in range(12):
                rng=random.Random(20261026+trial);nn=short=0
                for w in ww:
                    near=[d for d in dd if round(d['peers'])==round(w['peers'])]
                    if not near:near=sorted(dd,key=lambda d:abs(d['peers']-w['peers']))[:10]
                    donor=rng.choice(near);offset=rng.randrange(143)
                    z=occupancy(w,familyset,donor,offset)
                    nn+=z['n'];short+=z['n']*(z['le9'] or 0)
                values.append(short/nn if nn else None)
            bydonor[offalias]=q(values);trials+=values
        allout[name]=dict(pooledRuns=q(trials),perOfficialCapture=bydonor)
    # Combination checks interaction, using all six donors without selecting a winner.
    for trial in range(20):
        rng=random.Random(20261026+trial);nn=short=0
        for w in ww:
            dd=donors[rng.choice(list(donors))]
            near=[d for d in dd if round(d['peers'])==round(w['peers'])] or dd
            z=occupancy(w,('mass','body','food'),rng.choice(near),rng.randrange(143),True,rng)
            nn+=z['n'];short+=z['n']*(z['le9'] or 0)
        phaseout.append(short/nn if nn else None)
    # Concentration at the six nominal tick phases; per-window maxima and collision ratios.
    phase=[]
    for w in ww:
        ticks=[]
        for f in w['frames']:
            ticks.extend(round(f['t']/7) for p in f['info'] if p['kind']=='rot' and not p['own'])
        if ticks:
            ct=Counter(t%6 for t in ticks)
            phase.append(100*max(ct.values())/len(ticks))
    result.append(dict(id=x['id'],windows=len(ww),substitution=allout,combinedWithPhase=q(phaseout),maxPhaseShare=q(phase)))
    print(x['id'],{k:round(v['pooledRuns']['p50'],2) for k,v in allout.items()},'combined',q(phaseout),'phase',q(phase),flush=True)

# Exact peer-count sensitivity, with one fixed reference time-exposure mixture.
cells={}
for x in data:
    d=defaultdict(list)
    with gzip.open(INPUT/(x['id']+'.frames.csv.gz'),'rt',newline='') as f:
        prev=None
        for r in csv.DictReader(f):
            if prev and r['epoch']==prev['epoch'] and prev['active']==r['active']=='True' and r['leftBin']=='6-10':d[int(r['before'])].append(int(r['gap']))
            prev=r
    cells[x['id']]={str(k):dict(n=len(v),seconds=sum(v)/1000,le9=pct(sum(g<=9 for g in v),len(v))) for k,v in d.items()}
weights={str(k):sum(cells[x['id']].get(str(k),{}).get('seconds',0) for x in official) for k in range(6,11)}
den=sum(weights.values());weights={k:v/den for k,v in weights.items()}
standardized={alias:sum(weights[k]*cell[k]['le9'] for k in weights) for alias,cell in cells.items() if all(k in cell for k in weights)}
restricted={k:v/(1-weights['10']) for k,v in weights.items() if k!='10'}
common={alias:sum(restricted[k]*cell[k]['le9'] for k in restricted) for alias,cell in cells.items() if all(k in cell for k in restricted)}
save('counterfactuals.json',dict(local=result,peerCells=cells,referenceExposureWeights=weights,exactPeerStandardizedLe9=standardized,common6to9Weights=restricted,common6to9Le9=common,scriptSha256=sha(__file__)))
print('standardized',standardized)
