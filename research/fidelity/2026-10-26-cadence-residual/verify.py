"""Independent CSV/semantic-output reconciliation and deterministic invariants."""
import csv,gzip,json,math,statistics
from collections import Counter,defaultdict
import analyze as analysis
analysis.configure()
from analyze import OUT,INPUT,save,sha,pct,q

results=json.loads((OUT/'results.json').read_text())
checks=[];extra={}
for x in results:
    alias=x['id'];families=Counter();gg=[];rr=[];cells=Counter();ns=0
    with gzip.open(INPUT/(alias+'.frames.csv.gz'),'rt',newline='') as f:
        prev=None
        for r in csv.DictReader(f):
            if prev and prev['epoch']==r['epoch'] and prev['active']==r['active']=='True' and r['leftBin']=='6-10':
                gg.append(int(r['gap']));rr.append(float(r['receiptGap']));families.update(r['families']);cells[int(r['before'])]+=int(r['gap'])
            prev=r
    ef=Counter();e=0;phase=defaultdict(list)
    allowed={(str(w['epoch']),w['start']) for w in json.loads((INPUT/(alias+'.windows.json')).read_text()) if w['bin']=='6-10'}
    ticks=defaultdict(list)
    with gzip.open(OUT/(alias+'.events.jsonl.gz'),'rt') as f:
        for line in f:
            r=json.loads(line);e+=1;ef.update(p['c'] for p in r['info'])
            key=(r['epoch'],r['t']//1000*1000)
            if key in allowed:
                ticks[key].extend(round(r['t']/7) for p in r['info'] if p['kind']=='rot' and not p['own'])
    ps=[100*max(Counter(t%6 for t in vv).values())/len(vv) for vv in ticks.values() if vv]
    assert len(gg)==e==x['n']
    assert families==ef
    assert sum(gg)/1000==x['seconds']
    if gg:assert abs(pct(sum(g<=9 for g in gg),len(gg))-x['le9'])<1e-10
    near=[g for g in gg if 3<=g<=9]
    extra[alias]=dict(tickProxyMeanMs=statistics.mean(near) if near else None,
        exact7MultiplePct=pct(sum(g%7==0 for g in gg),len(gg)),maxMotionPhaseShare=q(ps),
        receiptMinusHeaderMs=q([r-g for r,g in zip(rr,gg)]),receiptOvershoot7Pct=pct(sum(r-g>7 for r,g in zip(rr,gg)),len(gg)),
        meanPeersByExposure=sum(k*v for k,v in cells.items())/sum(cells.values()) if cells else None)
    checks.append(dict(id=alias,n=len(gg),csvAndRawFamilyCountsEqual=True,exposureReproduced=True))

official=[x for x in results if x['id'].startswith('official')]
assert sum(x['n'] for x in official)==195617
assert [x['n'] for x in results if x['id'].startswith('bots-500')]==[17147,4425,13071]
assert all(x['n']==0 for x in results if x['id'].startswith('bots-0'))
# Counterfactual primitives: occupied every tick yields 100%; every other tick 0%.
from analyze import cad
assert cad([0,7,14,21])==dict(n=3,le9=100.0)
assert cad([0,14,28])==dict(n=2,le9=0.0)
assert cad([0,7,21])==dict(n=2,le9=50.0)
save('verification.json',dict(status='pass',checks=checks,extra=extra,syntheticCadenceChecks=3,scriptSha256=sha(__file__)))
for x in results:
    if not x['n']:continue
    ss=x['snakeSeconds'];ints=x['intervals'];a=extra[x['id']]
    print(x['id'],'tick',round(a['tickProxyMeanMs'],4),'phase',a['maxMotionPhaseShare'],'intervals',{k:ints[k] for k in ['rot','fam','mass_any','anchor']},'motion,h,allmass/s',*[round(x['counters'][k]/ss,3) for k in ['motion','mass','massAny']], 'policy',x['localPolicyCounterfactual'],'pairs',x['receiptPairs'])
