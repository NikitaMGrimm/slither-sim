"""Authored native parity and complete local-selector rehearsal accounting."""
import argparse
import base64
import gzip
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import subprocess

spec=importlib.util.spec_from_file_location('wall',Path(__file__).with_name('analyze.py'))
wall=importlib.util.module_from_spec(spec);spec.loader.exec_module(wall)


def parity(binary):
    cases=[]
    for radius in (1500.,8000.,14000.):
        for sc in (1.,2.,6.):
            for gap in (-1.,0.,1.,8.,16.*sc,16.*sc+1):
                for rotation in (0.,1.,3.,5.):
                    for sign in (-1.,1.):
                        for angle in (0.,.35,.6,.69,.71,.85):
                            cases.append(((radius-gap)*math.cos(rotation),(radius-gap)*math.sin(rotation),
                                rotation+sign*(math.pi/2-angle),sc,radius))
    data=''.join(json.dumps(row)+'\n' for row in cases)
    native=subprocess.run([str(binary.resolve())],input=data,text=True,capture_output=True,check=True)
    rows=[json.loads(t) for t in native.stdout.splitlines()]
    if len(rows)!=len(cases):raise ValueError('native row count mismatch')
    maximum=0.
    mismatches=[]
    for case,row in zip(cases,rows,strict=True):
        p,dead=wall.constrain(case[:2],*case[2:])
        error=math.dist(p,row[:2]);maximum=max(maximum,error)
        if dead!=row[2] or error>1e-9:
            mismatches.append(dict(input=case,python=[*p,dead],native=row,pointDifference=error))
    requests=[dict(start=[100.,0.],initial=[0.,3.2,2.,8000.],dt=.007,
        changes=[[.05,math.pi/2,3.2,2.,8000.],[.1,math.pi/2,3.2,2.,8000.]]),
        dict(start=[7999.,0.],initial=[0.,14.,2.,8000.],dt=.007,changes=[[.1,0.,14.,2.,8000.]])]
    execution=subprocess.run([str(binary.resolve())],input=''.join(json.dumps(r)+'\n' for r in requests),
        text=True,capture_output=True,check=True)
    executed=[json.loads(t) for t in execution.stdout.splitlines()]
    if len(executed)!=2 or executed[0]['dead'] or math.dist(executed[0]['xy'],[105.,5.])>1e-9 or not executed[1]['dead']:
        raise ValueError('native interval event/terminal contract failed')
    return dict(origin='authored synthetic points',cases=len(cases),maximumPointDifference=maximum,
        mismatchCount=len(mismatches),mismatches=mismatches,
        lethalFlagsIdentical=not any(r['python'][2]!=r['native'][2] for r in mismatches),
        nativeIntervalContractCases=2,nativeIntervalContractPassed=True,
        nativeBinarySha256=wall.sha(binary))


def audit(path):
    digest=hashlib.sha256();payload=0;final=None;lines=0
    counts={d:dict(frames=0,bytes=0) for d in ('i','o')}
    with gzip.open(path,'rb') as stream:
        for line in stream:
            if final:raise ValueError('records after final seal')
            r=json.loads(line)
            if isinstance(r,dict) and r.get('event') in ('capture-end','capture-interrupted'):
                final=r
                if r['integrity']['sha256']!=digest.hexdigest() or r['integrity']['payloadBytes']!=payload or r['integrity']['lines']!=lines or r['integrity']['counts']!=counts or not r['collectorSourcesUnchanged']:
                    raise ValueError('local archive integrity failed')
            else:
                digest.update(line);payload+=len(line);lines+=1
                if isinstance(r,list):
                    data=base64.b64decode(r[2],validate=True)
                    counts[r[0]]['frames']+=1;counts[r[0]]['bytes']+=len(data)
    if not final or final['event']!='capture-end':raise ValueError('local archive incomplete')
    return dict(sha256=wall.sha(path),prefixVerified=True,counts=counts)


def local(root):
    path=root/'capture/bot0.jsonl.gz'
    receipt=audit(path)
    sp=wall.module('local_wall_protocol',wall.REPO/'tools/fidelity/slither_protocol.py')
    data=wall.extract(path,sp)
    if not data['complete'] or data['protocols']!=[19]:raise ValueError('local setup/completion failed')
    with (root/'intervals-private.jsonl').open('x',encoding='utf-8') as stream:
        rows=wall.score_lives(data,'authored-local',stream)
    cells={f'{mode}/{sign}':wall.aggregate([r for r in rows if r['primary'] and r['speedClass']==mode and r['direction']==sign])
        for mode in ('cruise','boost') for sign in (-1,1)}
    result=dict(origin='local-simulator',officialConnections=0,archive=receipt,
        complete=True,lives=len(data['lives']),nearRimAnchors=data['nearRimAnchors'],
        all=wall.aggregate(rows),primary=wall.aggregate([r for r in rows if r['primary']]),cells=cells,
        selectorPassedBothCruiseDirections=all(cells[f'cruise/{sign}']['intervals']>=5 for sign in (-1,1)),
        selectorSha256=wall.sha(Path(__file__).with_name('analyze.py')))
    wall.dump(root/'selector-results.json',result)
    return dict(eligible=len(rows),primary=result['primary']['intervals'],
        passed=result['selectorPassedBothCruiseDirections'],cells={k:v['intervals'] for k,v in cells.items()})


def main():
    p=argparse.ArgumentParser(description=__doc__)
    group=p.add_mutually_exclusive_group(required=True)
    group.add_argument('--native',type=Path);group.add_argument('--local',type=Path)
    p.add_argument('--runner',type=Path,help='use actual Rust interval execution for local P16 scores')
    p.add_argument('--output',type=Path)
    a=p.parse_args();wall.cpu_limit()
    wall.NATIVE=wall.NativeReplay(a.runner) if a.runner else None
    try:result=parity(a.native) if a.native else local(a.local)
    finally:
        if wall.NATIVE:wall.NATIVE.close()
    if a.output:wall.dump(a.output,result)
    print(json.dumps(result))


if __name__=='__main__':main()
