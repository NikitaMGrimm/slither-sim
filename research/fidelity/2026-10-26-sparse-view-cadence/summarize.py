"""Pool samples, never capture medians; keep all source results separately."""
from collections import Counter,defaultdict
import csv,gzip,json,math,statistics
from pathlib import Path
import analyze as a

import argparse
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, required=True, help='External analysis output directory')
args = parser.parse_args()
OUT=args.out.resolve()
class Summary:
    def __init__(self): self.server=Counter();self.receipt=Counter()
    def add(self,row):
        self.server[int(row['gap'])]+=1; self.receipt[float(row['receiptGap'])]+=1
    def result(self):
        def quant(counter):
            n=counter.total()
            def at(rank):
                total=0
                for v,c in sorted(counter.items()):
                    total+=c
                    if total>rank: return v
            return dict(n=n,min=min(counter) if n else None,
                p50=(at((n-1)//2)+at(n//2))/2 if n else None,
                **{f'p{p}':at(math.ceil(n*p/100)-1) if n else None for p in (90,95,99)},max=max(counter) if n else None)
        n=self.server.total(); duration=sum(v*c for v,c in self.server.items())
        return dict(server=quant(self.server),receipt=quant(self.receipt),exposureMs=duration,
            le9=sum(c for v,c in self.server.items() if v<=9)/n if n else None,
            gt20=sum(c for v,c in self.server.items() if v>20)/n if n else None,
            framesPerSecond=1000*n/duration if duration else None)

results=a.load(OUT/'results.json')+a.load(OUT/'older-results.json')
groups=defaultdict(Summary); windows=defaultdict(list); categories=defaultdict(Counter)
quiet=defaultdict(Counter); changed=defaultdict(Counter); witnesses={}
activity_counts=defaultdict(Counter)
for result in results:
    if 'error' in result: continue
    name=result['id']; cohort=result['cohort']; previous=None
    with gzip.open(OUT/(name+'.frames.csv.gz'),'rt',encoding='utf-8') as stream:
        for row in csv.DictReader(stream):
            eligible=previous and previous['epoch']==row['epoch'] and previous['active']=='True' and row['active']=='True'
            previous=row
            if not eligible: continue
            keys=['all','left_'+row['leftBin'],'end_'+row['endBin']]
            for activity in ('noOtherChange','noSnakeChange','entityQuiet'):
                if row[activity]=='True': keys.extend([activity,'left_'+row['leftBin']+'_'+activity])
            keys.append(row['category'])
            for label,column in [('food_activity','foodActivity'),('prey_activity','preyActivity'),('own_changed','ownChanged')]:
                if row[column]=='True': keys.append(label)
            for k in keys: groups[cohort+'/'+k].add(row)
            categories[cohort][row['category']]+=1
            changed[cohort][int(row['changed'])]+=1
            activity_counts[cohort]['ownChanged']+=row['ownChanged']=='True'
            activity_counts[cohort]['foodActivity']+=row['foodActivity']=='True'
            activity_counts[cohort]['preyActivity']+=row['preyActivity']=='True'
            activity_counts[cohort]['ownPresent']+=row['ownPresent']=='True'
            if row['entityQuiet']=='True': quiet[cohort][''.join(sorted(row['families']))]+=1
            if cohort=='official' and (row['category']=='redundant_motion_only' or (row['leftBin'] in ('0','1-2') and int(row['gap'])>=100)):
                key='redundant_motion_only' if row['category']=='redundant_motion_only' else 'sparse_long_gap'
                witnesses.setdefault(key,dict(capture=name,sourceSha256=result['sha256'],**row))
            if cohort=='official' and row['leftBin'] in ('0','1-2'):
                old=witnesses.get('maximum_primary_sparse_gap')
                if old is None or int(row['gap'])>int(old['gap']):
                    witnesses['maximum_primary_sparse_gap']=dict(capture=name,sourceSha256=result['sha256'],**row)
            if cohort=='older' and row['leftBin']=='0':
                old=witnesses.get('maximum_older_zero_other_gap')
                if old is None or int(row['gap'])>int(old['gap']):
                    witnesses['maximum_older_zero_other_gap']=dict(capture=name,sourceSha256=result['sha256'],**row)
            if cohort=='official' and row['entityQuiet']=='True' and set(row['families'])=={'h'}:
                witnesses.setdefault('unchanged_mass_only',dict(capture=name,sourceSha256=result['sha256'],**row))
    for w in a.load(OUT/(name+'.windows.json')): windows[cohort+'/'+w['bin']].append(w['frames'])
pooled=dict(groups={k:v.result() for k,v in groups.items()},
    windows={k:a.quant(v) for k,v in windows.items()},categories=dict(categories),
    changedOtherSnakeHistogram={k:dict(v) for k,v in changed.items()},quietFamilySignatures=dict(quiet),activity=dict(activity_counts))
a.save(OUT/'pooled.json',pooled)
for key,witness in witnesses.items():
    source=next(r['path'] for r in results if r['id']==witness['capture'])
    if a.sha(source)!=witness['sourceSha256']: raise ValueError('witness archive hash mismatch')
    for i,frame in enumerate(x for x in a.sp.read(source) if isinstance(x,a.sp.Frame)):
        if i==int(witness['frame']):
            witness['packetsHex']=[p.hex() for p in frame.packets];break
a.save(OUT/'selected-witnesses.json',witnesses)

def table(group):
    for b in a.BINS:
        v=groups[group+'/left_'+b].result(); s=v['server']; w=a.quant(windows[group+'/'+b])
        print(group,b,s['n'],s['p50'],s['p95'],s['max'],round(100*(v['le9'] or 0),2),round(v['exposureMs']/1000,3),'windows',w['n'],w['p50'])
for c in ('official','older','local'): table(c)
print('official entity quiet signatures',quiet['official'])
print('official activity',[(k,groups['official/'+k].result()) for k in ('noOtherChange','noSnakeChange','entityQuiet','own_only','pong_only','redundant_motion_only','other_entity_quiet')])
