"""Offline sparse-view analysis. Existing decoder owns all protocol interpretation."""
import argparse
from collections import Counter, defaultdict
import csv
import gzip
import hashlib
import json
import math
from pathlib import Path
import statistics
import subprocess
import sys

STUDY = Path(__file__).resolve().parent
ROOT = STUDY.parents[2]
OUT = None
sys.path.insert(0, str(ROOT / 'tools/fidelity'))
import slither_protocol as sp
from capture_quality import audit_capture

BINS = ['0', '1-2', '3-5', '6-10', '11-20', '21+']
def binof(n):
    return next(label for limit, label in zip([0, 2, 5, 10, 20, math.inf], BINS) if n <= limit)
def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def load(path): return json.loads(Path(path).read_text(encoding='utf-8'))
def save(path, value): Path(path).write_text(json.dumps(value, indent=2, allow_nan=False)+'\n', encoding='utf-8', newline='\n')
def quant(values):
    a = sorted(values)
    return dict(n=len(a), min=a[0] if a else None, p50=statistics.median(a) if a else None,
                **{f'p{p}': a[max(0, math.ceil(len(a)*p/100)-1)] if a else None for p in (90,95,99)}, max=a[-1] if a else None)
def summarize(rows):
    s = [r['gap'] for r in rows]
    return dict(server=quant(s), receipt=quant([r['receiptGap'] for r in rows]),
        le9=sum(v<=9 for v in s)/len(s) if s else None,
        gt20=sum(v>20 for v in s)/len(s) if s else None,
        exposureMs=sum(s), meanGapMs=statistics.mean(s) if s else None,
        framesPerSecond=1000*len(s)/sum(s) if sum(s) else None)
def snake_signature(o):
    return None if o is None else (o.ang,o.wang,o.sp,o.fam,o.kills,o.rsc)

def fixed_windows(rows):
    """Integrate causal membership over each whole active server-time window."""
    states={}
    for row in rows:
        a,b=row['server']-row['gap'],row['server']
        for k in range(a//1000,b//1000+1):
            left,right=max(a,k*1000),min(b,(k+1)*1000)
            if right<=left: continue
            key=(row['epoch'],k)
            w=states.setdefault(key,dict(epoch=row['epoch'],start=k*1000,covered=0,bins=set(),frames=0))
            w['covered']+=right-left; w['bins'].add(row['leftBin'])
        key=(row['epoch'],b//1000)
        w=states.setdefault(key,dict(epoch=row['epoch'],start=b//1000*1000,covered=0,bins=set(),frames=0))
        w['frames']+=1; w['bins'].add(row['endBin'])
    return [dict(epoch=w['epoch'],start=w['start'],bin=next(iter(w['bins'])),frames=w['frames'])
            for w in states.values() if w['covered']==1000 and len(w['bins'])==1]

def measure(spec):
    path = Path(spec['path'])
    if sha(path) != spec['sha256']: raise ValueError('input hash mismatch')
    audit = audit_capture(path) if spec['cohort'] != 'older' else None
    if audit and audit['issues']: raise ValueError('audit issues: '+repr(audit['issues']))
    rows, counts, family, witnesses = [], Counter(), Counter(), {}
    world, conn, epoch, previous, active = None, None, 0, None, False
    frame_index = -1
    with gzip.open(OUT/(spec['id']+'.frames.csv.gz'), 'wt', newline='', encoding='utf-8') as stream:
        fields = ['epoch','frame','conn','server','receipt','active','gap','receiptGap','before','after','leftBin','endBin','changed','reported','changedIds','reportedIds','visibleIds','ownId','ownState','ownAnchorAgeMs','ownChanged','ownPresent','foods','prey','foodActivity','preyActivity','families','category','noOtherChange','noSnakeChange','entityQuiet']
        writer = csv.DictWriter(stream, fieldnames=fields); writer.writeheader()
        for item in sp.read(path):
            if isinstance(item, dict):
                if item.get('event') in ('connect','close','connection-failed'):
                    world=None; previous=None; active=False
                continue
            if not isinstance(item, sp.Frame): continue
            frame_index += 1
            setup = any(p[:1] == b'a' for p in item.packets)
            if item.conn != conn or setup or world is None:
                world=sp.World(contract=False); conn=item.conn; epoch+=1; previous=None; active=False
            was_active = active
            before = len(world.snakes) - int(world.self_id in world.snakes)
            changed, reported = set(), set()
            own_changed=False; food_activity=False; prey_activity=False; other_commands=[]; commands=[]
            for p in item.packets:
                c=chr(p[0]); commands.append(c); family[c]+=1
                # Get affected entities from the existing tracker's semantic events.
                snapshots={sid:snake_signature(o) for sid,o in world.snakes.items()} if c in 'eE345d7hrkR' else {}
                world.feed(p,item.server)
                for event in world.events:
                    kind=event[0]
                    if kind in ('spawn','remove','anchor','tail','rot','fam','kill','rsc'):
                        sid=event[2]; reported.add(sid)
                        if kind in ('spawn','remove','anchor','tail') or snapshots.get(sid)!=snake_signature(world.snakes.get(sid)):
                            changed.add(sid)
                    if kind=='game_over' or (kind=='remove' and event[2]==world.self_id): active=False
                world.events.clear()
                if c=='s' and len(p)>7 and sp.u16(p,1)==world.self_id: active=True
                if c in 'FbfwcC<': food_activity=True
                if c in 'yj': prey_activity=True
                if c not in 'seE345d7hrkRgnGN+=FbfwcC<yjp': other_commands.append(c)
            own_changed=world.self_id in changed
            other_changed=changed-{world.self_id}; other_reported=reported-{world.self_id}
            after=len(world.snakes)-int(world.self_id in world.snakes)
            no_snake=not changed
            entity_quiet=no_snake and not food_activity and not prey_activity
            if not commands: category='empty'
            elif set(commands)=={'p'}: category='pong_only'
            elif set(commands)<=set('eE345d7') and not changed: category='redundant_motion_only'
            elif world.self_id in reported and not other_reported and not food_activity and not prey_activity and not other_commands and 'p' not in commands: category='own_only'
            elif entity_quiet and set(commands)<=set('pMlVzmu'): category='maintenance_only'
            elif entity_quiet: category='other_entity_quiet'
            else: category='entity_activity'
            counts['frames']+=1
            if active: counts['activeFrames']+=1
            if category in ('empty','redundant_motion_only','pong_only','other_entity_quiet'):
                witnesses.setdefault(category,dict(conn=conn,frame=frame_index,server=item.server,packets=[p.hex() for p in item.packets]))
            own=world.snakes.get(world.self_id)
            row=dict(epoch=epoch,frame=frame_index,conn=conn,server=item.server,receipt=item.t,active=active,
                gap=item.server-previous['server'] if previous else None,
                receiptGap=round(item.t-previous['receipt'],3) if previous else None,
                before=before,after=after,leftBin=binof(previous['after']) if previous else None,endBin=binof(after),
                changed=len(other_changed),reported=len(other_reported),changedIds=';'.join(map(str,sorted(changed))),reportedIds=';'.join(map(str,sorted(reported))),
                visibleIds=';'.join(map(str,sorted(world.snakes))),ownId=world.self_id,
                ownState=json.dumps(dict(angle=own.ang,wanted=own.wang,speed=own.sp,mass=own.fam,segments=own.sct,head=own.base,anchorServer=own.anchor_server)) if own else '',
                ownAnchorAgeMs=item.server-own.anchor_server if own else None,
                ownChanged=own_changed,ownPresent=world.self_id in world.snakes,foods=len(world.foods),prey=len(world.prey),
                foodActivity=food_activity,preyActivity=prey_activity,families=''.join(commands),category=category,
                noOtherChange=not other_changed,noSnakeChange=no_snake,entityQuiet=entity_quiet)
            writer.writerow(row)
            if previous and was_active and active and not setup:
                if row['gap']<0: raise ValueError('server clock regressed')
                rows.append(row)
                if 'maxGap' not in witnesses or row['gap']>witnesses['maxGap']['gap']:
                    witnesses['maxGap']=dict(row,packets=[p.hex() for p in item.packets])
            previous=row
    windows=fixed_windows(rows)
    save(OUT/(spec['id']+'.windows.json'),windows)
    groups={'all':rows}
    for label in BINS:
        groups['left_'+label]=[r for r in rows if r['leftBin']==label]
        groups['end_'+label]=[r for r in rows if r['endBin']==label]
        for activity in ('noOtherChange','noSnakeChange','entityQuiet'):
            groups['left_'+label+'_'+activity]=[r for r in rows if r['leftBin']==label and r[activity]]
    for label in ('noOtherChange','noSnakeChange','entityQuiet'):
        groups[label]=[r for r in rows if r[label]]
    for label in ('empty','own_only','pong_only','maintenance_only','redundant_motion_only','other_entity_quiet'):
        groups[label]=[r for r in rows if r['category']==label]
    return dict(**spec,audit=None if audit is None else {k:audit[k] for k in ('status','issues','warnings','integrityVerified','completeGzip','network')},
        counts=dict(counts), families=dict(family), summaries={k:summarize(v) for k,v in groups.items()},
        categories=dict(Counter(r['category'] for r in rows)),
        windows={label:quant([w['frames'] for w in windows if w['bin']==label]) for label in BINS},
        witnesses=witnesses, activity=dict(
            meanOtherSnakes=statistics.mean([r['before'] for r in rows]) if rows else None,
            timeWeightedOtherSnakes=sum(r['before']*r['gap'] for r in rows)/sum(r['gap'] for r in rows) if rows else None,
            fractionNoOtherChange=sum(r['noOtherChange'] for r in rows)/len(rows) if rows else None,
            fractionNoSnakeChange=sum(r['noSnakeChange'] for r in rows)/len(rows) if rows else None))

def inputs(manifest):
    specs = load(manifest)
    ids = set()
    hashes = set()
    for spec in specs:
        if spec['id'] in ids or spec['sha256'] in hashes:
            raise ValueError('duplicate input identity')
        if not spec['id'] or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for c in spec['id']):
            raise ValueError('input id must be a portable filename stem')
        if spec['cohort'] not in ('official', 'local', 'older'):
            raise ValueError('unknown cohort')
        ids.add(spec['id']); hashes.add(spec['sha256'])
        path = Path(spec['path'])
        spec['path'] = str(path if path.is_absolute() else manifest.parent / path)
    return specs


def main():
    global OUT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', type=Path, required=True, help='Local archive mapping; sources.json supplies pinned identities')
    parser.add_argument('--out', type=Path, required=True, help='External output directory; primary run requires a new directory')
    parser.add_argument('--older', action='store_true', help='Analyze historical cohort into an existing primary output')
    args = parser.parse_args()
    OUT = args.out.resolve()
    if OUT == ROOT or ROOT in OUT.parents:
        raise ValueError('full analysis outputs must stay outside the repository')
    specs = inputs(args.inputs.resolve())
    if args.older:
        if not OUT.is_dir() or (OUT/'older-results.json').exists():
            raise ValueError('older run requires primary output and no previous older results')
        if load(OUT/'inputs.json') != specs:
            raise ValueError('input mapping changed between cohorts')
    else:
        OUT.mkdir(parents=True, exist_ok=False)
        save(OUT/'inputs.json',specs)
    provenance=dict(analysisBase='503ca4df8104321c7b0e5c28cc1f628c1dd6cc48',
        executionHead=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        planSha256=sha(STUDY/'PLAN.md'),scriptSha256=sha(__file__),decoderSha256=sha(sp.__file__))
    save(OUT/'provenance.json',provenance)
    results=[]; seen=set()
    for spec in specs:
        if (spec['cohort']=='older') != args.older: continue
        if spec['sha256'] in seen: continue
        seen.add(spec['sha256'])
        print('Analyzing '+spec['id'],flush=True)
        try: result=measure(spec)
        except Exception as exc:
            result=dict(**spec,error=type(exc).__name__+': '+str(exc))
        save(OUT/(spec['id']+'.json'),result); results.append(result)
        print(spec['id']+' '+(result.get('error') or json.dumps(result['summaries']['all'])),flush=True)
    save(OUT/('older-results.json' if args.older else 'results.json'),results)
    if any('error' in result for result in results):
        raise SystemExit(2)

if __name__=='__main__': main()
