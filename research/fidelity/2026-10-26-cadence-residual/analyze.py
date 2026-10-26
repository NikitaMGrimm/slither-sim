"""Offline, serial, retrospective residual analysis. Outputs contain aliases only."""
import argparse, csv, gzip, hashlib, json, math, random, statistics, sys
from collections import Counter, defaultdict
from pathlib import Path

STUDY=Path(__file__).resolve().parent
ROOT=STUDY.parents[2]
OUT=INPUT=None
sys.path.insert(0,str(ROOT/'tools/fidelity'))
import slither_protocol as sp

def save(name,obj):
    (OUT/name).write_text(json.dumps(obj,indent=2,allow_nan=False)+'\n',encoding='utf-8',newline='\n')
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def q(v):
    v=sorted(v)
    return dict(n=len(v),p50=statistics.median(v) if v else None,p95=v[math.ceil(.95*len(v))-1] if v else None)
def pct(n,d): return 100*n/d if d else None
def cad(times):
    gaps=[b-a for a,b in zip(times,times[1:])]
    return dict(n=len(gaps),le9=pct(sum(g<=9 for g in gaps),len(gaps)))
def corr(a,b):
    if len(a)<2 or statistics.pstdev(a)==0 or statistics.pstdev(b)==0:return None
    ma,mb=statistics.mean(a),statistics.mean(b)
    return statistics.mean((x-ma)*(y-mb) for x,y in zip(a,b))/(statistics.pstdev(a)*statistics.pstdev(b))

def configure(new=False):
    global OUT, INPUT
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs',type=Path,required=new,help='External archive manifest; sources.json supplies identities')
    parser.add_argument('--sparse-output',type=Path,required=True,help='External sparse-view CSV/window outputs')
    parser.add_argument('--out',type=Path,required=True,help='External residual output; analyze requires a new directory')
    args=parser.parse_args()
    OUT=args.out.resolve(); INPUT=args.sparse_output.resolve()
    if OUT==ROOT or ROOT in OUT.parents or OUT==INPUT or OUT in INPUT.parents or INPUT in OUT.parents:
        raise ValueError('outputs must be external and separate from sparse-view inputs')
    pins=json.loads((STUDY/'source-pins.json').read_text(encoding='utf-8'))
    if sha(sp.__file__)!=pins['decoderSha256']:
        raise ValueError('decoder hash differs from pinned analysis')
    if new: OUT.mkdir(parents=True,exist_ok=False)
    elif not OUT.is_dir(): raise ValueError('existing analysis output required')
    return args

def input_specs(manifest):
    specs=[s for s in json.loads(manifest.read_text(encoding='utf-8')) if s['cohort']!='older']
    expected=json.loads((STUDY/'sources.json').read_text(encoding='utf-8'))
    identity=lambda s:(s['id'],s['cohort'],s['sha256'])
    if len(specs)!=len(expected) or {identity(s) for s in specs}!={identity(s) for s in expected}:
        raise ValueError('manifest must retain all twelve pinned primary identities')
    indexed={s['id']:s for s in specs}
    result=[]
    for original in expected:
        spec=dict(indexed[original['id']]); path=Path(spec['path'])
        spec['path']=str(path if path.is_absolute() else manifest.parent/path)
        result.append(spec)
    return result

def measure(spec):
    alias=spec['id']; path=Path(spec['path'])
    assert sha(path)==spec['sha256']
    with gzip.open(INPUT/(alias+'.frames.csv.gz'),'rt',newline='') as f: rows=list(csv.DictReader(f))
    prev=None; eligible={}; runs=defaultdict(list); run=0
    for r in rows:
        r['server']=int(r['server']); r['frame']=int(r['frame'])
        good=bool(prev and prev['active']=='True' and r['active']=='True' and r['epoch']==prev['epoch'] and r['gap']!='')
        r['eligible']=good
        if not(good and r['leftBin']=='6-10'):run+=1
        else:eligible[r['frame']]=r; runs[run].append(r)
        r['run']=run;prev=r
    byframe={r['frame']:r for r in rows}
    target=list(eligible.values()); gaps=[int(r['gap']) for r in target]; rec=[float(r['receiptGap']) for r in target]
    n=len(gaps); exposure=sum(gaps)/1000
    c=Counter();fam=Counter(); intervals=defaultdict(list); last={}; exposures=Counter(); behavior=Counter();
    frameinfo={}; world=None;conn=None;fi=-1; binrun=None
    for item in sp.read(path):
        if not isinstance(item,sp.Frame):continue
        fi+=1;r=byframe[fi]; selected=fi in eligible
        if world is None or item.conn!=conn or any(p[:1]==b'a' for p in item.packets):
            world=sp.World(contract=False);conn=item.conn;last={}
        # Intervals must be continuously visible, active AND remain in this bin.
        if not selected or binrun!=r['run']:last={}
        binrun=r['run']
        if selected:
            dt=int(r['gap'])/1000
            for sid,o in world.snakes.items():
                if sid==world.self_id:continue
                exposures['snakeSeconds']+=dt
                behavior['fastSeconds']+=dt*(o.sp>9)
                behavior['boost14Seconds']+=dt*(o.sp>=13.9)
                delta=abs((o.wang-o.ang+math.pi)%(2*math.pi)-math.pi)
                behavior['turningSeconds']+=dt*(delta>2*math.pi/256)
                behavior['segmentsSeconds']+=dt*o.sct
        info=[];motionids=set()
        for p in item.packets:
            ch=chr(p[0]); world.feed(p,item.server)
            record=dict(c=ch,sid=None,kind='other',repeat=False,dropGate=False)
            for ev in world.events:
                kind=ev[0]
                if kind in ('spawn','remove'):last={k:v for k,v in last.items() if k[1]!=ev[2]}
                if kind in ('rot','fam','anchor','tail','spawn','remove','kill','rsc'):
                    record['sid']=ev[2];record['kind']=kind
                if kind=='rot':
                    record['repeat']=ev[3]==ev[4]
                    if selected and ev[2]!=world.self_id:
                        motionids.add(ev[2]); c['motion']+=1;c['motionRepeat']+=record['repeat']
                        for j,key in enumerate(('angleChanges','wantedChanges','speedChanges')):behavior[key]+=ev[3][j]!=ev[4][j]
                if kind=='fam':record['repeat']=ev[3]==ev[4]
                if selected and (kind in ('fam','tail') or (kind=='anchor' and ev[5])) and ev[2]!=world.self_id:
                    key=('mass_any',ev[2]);old=last.get(key)
                    if old is not None:intervals['mass_any'].append(item.server-old)
                    last[key]=item.server;c['massAny']+=1
                if selected and kind in ('rot','fam','anchor','tail') and ev[2]!=world.self_id:
                    key=(kind,ev[2]);old=last.get(key)
                    if old is not None:intervals[kind].append(item.server-old)
                    last[key]=item.server
                    if kind=='fam':c['mass']+=1;c['massRepeat']+=record['repeat']
            world.events.clear()
            record['own']=record['sid'] is not None and record['sid']==world.self_id
            info.append(record)
            if selected:fam[ch]+=1
        if selected:
            c['motionFrames']+=bool(motionids); c['multiMotionFrames']+=len(motionids)>=2
            c['motionDistinctTotal']+=len(motionids)
            frameinfo[fi]=dict(t=item.server,epoch=r['epoch'],run=r['run'],peers=int(r['before']),info=info)
    hist={k:pct(sum(test(g) for g in gaps),n) for k,test in {
        '0-2':lambda g:g<=2,'3-9':lambda g:3<=g<=9,'10-17':lambda g:10<=g<=17,'18+':lambda g:g>=18}.items()}
    hist['mode']=Counter(gaps).most_common(8)
    pairs=[(int(a['gap']),int(b['gap'])) for rr in runs.values() for a,b in zip(rr,rr[1:])]
    pairsrec=[(float(a['receiptGap']),float(b['receiptGap'])) for rr in runs.values() for a,b in zip(rr,rr[1:])]
    def pairmetrics(pp):
        return dict(n=len(pp),correlation=corr([a for a,b in pp],[b for a,b in pp]),shortLong=pct(sum((a<=2 and b>=12) or (b<=2 and a>=12) for a,b in pp),len(pp)))
    # Deletion counterfactuals retain real timestamps, episodes and all other packets.
    deletion={}
    rules={
      'none':lambda p:False,
      'repeat_motion_mass':lambda p:p['kind'] in ('rot','fam') and p['repeat'],
      'motion':lambda p:p['kind']=='rot', 'mass_h':lambda p:p['c']=='h',
      'food':lambda p:p['c'] in 'FbfwcC<', 'prey':lambda p:p['c'] in 'yj',
      'own':lambda p:p['own'], 'pong':lambda p:p['c']=='p',
      'other_motion':lambda p:p['kind']=='rot' and not p['own'],
    }
    for name,remove in rules.items():
        keep=defaultdict(list);removed=0;totalg=0;short=0
        for f in frameinfo.values():
            if any(not remove(p) for p in f['info']):keep[f['run']].append(f['t'])
            else:removed+=1
        for tt in keep.values():
            gg=[b-a for a,b in zip(tt,tt[1:])];totalg+=len(gg);short+=sum(g<=9 for g in gg)
        deletion[name]=dict(removedFrames=removed,n=totalg,le9=pct(short,totalg))
    # Apply local minimum intervals plus decoded-value suppression to official traces.
    # Gate starts from first observed incremental report in each continuous bin episode.
    gatecf={}
    for gate_name,motionmin,massmin in [('strict',42,105),('tolerant',35,98)]:
        gate={};kept=defaultdict(list); dropped=Counter()
        for f in frameinfo.values():
            remains=[]
            for p in f['info']:
                if p['kind'] in ('rot','fam'):
                    key=(f['run'],p['kind'],p['sid']);lastt=gate.get(key)
                    if p['repeat'] or (lastt is not None and f['t']-lastt<(motionmin if p['kind']=='rot' else massmin)):
                        dropped[p['kind']]+=1;continue
                    gate[key]=f['t']
                remains.append(p)
            if remains:kept[f['run']].append(f['t'])
        gg=[b-a for tt in kept.values() for a,b in zip(tt,tt[1:])]
        gatecf[gate_name]=dict(n=len(gg),le9=pct(sum(g<=9 for g in gg),len(gg)),dropped=dict(dropped))
    # Stable full 1s windows. Actual 7ms grid plus phase randomization of motion trains.
    windows=json.loads((INPUT/(alias+'.windows.json')).read_text())
    stable={(str(w['epoch']),w['start']) for w in windows if w['bin']=='6-10'}
    ww=defaultdict(list)
    for f in frameinfo.values():
        key=(f['epoch'],f['t']//1000*1000)
        if key in stable:ww[key].append(f)
    randomizer=random.Random(20261026)
    grids=[]; shifts=[[] for _ in range(20)]; overlap=[]; fixedrates=[]; counts=[]
    for (ep,start),ff in ww.items():
        lo=math.ceil(start/7);hi=math.ceil((start+1000)/7);base=set();motion=defaultdict(set);actual=set()
        for f in ff:
            tick=round(f['t']/7);actual.add(tick)
            for p in f['info']:
                if p['kind']=='rot' and not p['own']:motion[p['sid']].add(tick)
                else:base.add(tick)
        actual={t for t in actual if lo<=t<hi};base={t for t in base if lo<=t<hi}
        mt=[t for vv in motion.values() for t in vv if lo<=t<hi]
        overlap.append(dict(events=len(mt),occupied=len(set(mt)),max= max(Counter(mt).values(),default=0)))
        grids.append(cad(sorted(t*7 for t in actual)));fixedrates.append(len(base));counts.append(len(actual))
        for trial in range(20):
            occupied=set(base)
            for sid,vv in motion.items():
                shift=randomizer.randrange(6)
                occupied.update(lo+(t-lo+shift)%(hi-lo) for t in vv if lo<=t<hi)
            shifts[trial].append(cad(sorted(t*7 for t in occupied)))
    def pool(v):
        nn=sum(x['n'] for x in v);return sum(x['n']*(x['le9'] or 0) for x in v)/nn if nn else None
    phase=dict(windows=len(ww),gridLe9=pool(grids),randomizedLe9=q([pool(v) for v in shifts]) if ww else None,
        motionEvents=sum(x['events'] for x in overlap),motionOccupied=sum(x['occupied'] for x in overlap),
        maxSameTick=max((x['max'] for x in overlap),default=0),framesPerWindow=q(counts),nonOtherMotionFramesPerWindow=q(fixedrates))
    rates={k:v/exposure for k,v in fam.items()} if exposure else {}
    intsummary={k:dict(**q(v),lt35=pct(sum(x<35 for x in v),len(v)),lt42=pct(sum(x<42 for x in v),len(v)),around42=pct(sum(39<=x<=45 for x in v),len(v)),around105=pct(sum(101<=x<=109 for x in v),len(v)),ge100=pct(sum(x>=100 for x in v),len(v))) for k,v in intervals.items()}
    ss=exposures['snakeSeconds']
    result=dict(id=alias,inputSha256=spec['sha256'],csvSha256=sha(INPUT/(alias+'.frames.csv.gz')),n=n,seconds=exposure,
        le9=pct(sum(g<=9 for g in gaps),n),server=q(gaps),receipt=q(rec),hist=hist,serverPairs=pairmetrics(pairs),receiptPairs=pairmetrics(pairsrec),
        receiptLe9=pct(sum(g<=9 for g in rec),n),receiptShort=pct(sum(g<=2 for g in rec),n),
        snakeSeconds=ss,counters=dict(c),intervals=intsummary,intervalHist={k:dict(Counter(v)) for k,v in intervals.items()},
        behavior=dict(behavior),perSnakeRates={k:v/ss for k,v in behavior.items() if k.endswith('Changes')} if ss else {},
        fastPct=pct(behavior['fastSeconds'],ss),boost14Pct=pct(behavior['boost14Seconds'],ss),turningPct=pct(behavior['turningSeconds'],ss),
        meanSegments=behavior['segmentsSeconds']/ss if ss else None,familyRates=rates,deletion=deletion,localPolicyCounterfactual=gatecf,phase=phase)
    # Derived packet semantics only; no archive metadata or endpoint details.
    with gzip.open(OUT/(alias+'.events.jsonl.gz'),'wt',encoding='utf-8') as stream:
        for f in frameinfo.values():
            stream.write(json.dumps(f,separators=(',',':'))+'\n')
    save(alias+'.json',result)
    print(alias, 'n',n,'le9',result['le9'],'motion/snake/s',c['motion']/ss if ss else None,'phase',phase['gridLe9'],phase['randomizedLe9'],flush=True)
    return result

if __name__=='__main__':
    args=configure(new=True)
    wanted=input_specs(args.inputs.resolve())
    results=[measure(s) for s in wanted]
    save('results.json',results)
    save('provenance.json',dict(scriptSha256=sha(__file__),decoderSha256=sha(sp.__file__),inputs=[dict(id=s['id'],sha256=s['sha256']) for s in wanted]))
