"""Bounded assembly from retained results; no model or interval reselection."""
import argparse
from collections import defaultdict
import importlib.util
import json
import math
from pathlib import Path
import subprocess

spec=importlib.util.spec_from_file_location('wall',Path(__file__).with_name('analyze.py'))
wall=importlib.util.module_from_spec(spec);spec.loader.exec_module(wall)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--analysis',type=Path,required=True)
    p.add_argument('--local',type=Path,required=True)
    p.add_argument('--parity',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();wall.cpu_limit()
    result=json.loads((a.analysis/'selected-results.json').read_text(encoding='utf-8'))
    result['planCommit']=subprocess.check_output(['git','rev-parse',result['planCommit']],text=True).strip()
    result['analysisCommit']='400d6d4f'
    result['analysisCommit']=subprocess.check_output(['git','rev-parse',result['analysisCommit']],text=True).strip()
    intervals=[json.loads(t) for t in (a.analysis/'intervals-private.jsonl').read_text(encoding='utf-8').splitlines()]
    for row in intervals:row['budgets']={int(k):v for k,v in row['budgets'].items()}
    def brief(rows):
        summary=wall.aggregate(rows)
        return {k:summary[k] for k in ('intervals','seconds','lives')} | dict(models={m:dict(
            medianEndpoint=v['aliveEndpoint']['median'],p95Endpoint=v['aliveEndpoint']['p95'],
            maximumEndpoint=v['aliveEndpoint']['maximum'],aliveIntervals=v['aliveEndpoint']['n'],
            modelLethal=v['modelLethal'],signedRadialMean=v['aliveRadial']['mean'],
            signedTangentialMean=v['aliveTangential']['mean'],freeMedian=v['freeEndpoint']['median'])
            for m,v in summary['models'].items()})
    result['perSourceAll']={name:brief([r for r in intervals if r['source']==name]) for name in sorted({r['source'] for r in intervals})}
    result['modeDirection']={f'{mode}/{sign}':brief([r for r in intervals if r['speedClass']==mode and r['direction']==sign])
        for mode in ('cruise','boost','intermediate') for sign in (-1,1)}
    by_life=defaultdict(list)
    for r in intervals:by_life[r['source'],r['connection'],r['incarnation']].append(r)
    result['lifeExposure']=[dict(source=source,connection=conn,incarnation=inc,
        intervals=len(rows),primary=sum(r['primary'] for r in rows),
        countRange=[min(r['count'] for r in rows),max(r['count'] for r in rows)],
        modes=sorted({r['speedClass'] for r in rows}),directions=sorted({r['direction'] for r in rows}))
        for (source,conn,inc),rows in sorted(by_life.items())]
    local=json.loads((a.local/'selector-results.json').read_text(encoding='utf-8'))
    parity=json.loads(a.parity.read_text(encoding='utf-8'))
    powers=wall.power()['rows']
    extra=[]
    for r in powers:
        sc=wall.scale(r['count'])
        v=(5.39+.4*sc)/.032 if r['mode']=='cruise' else 14/.032
        gap=16*sc*math.sin(r['angle'])
        no_slide=v*r['seconds']*math.sin(r['angle'])
        fresh_stale=2*v*r['seconds']*(.033/.008)*.042
        extra.append(dict(**r,headProjectionIdealGap=gap,noSlidingFreeDiagnostic=no_slide,
            withinFrozenIntervalDuration=.05<=r['seconds']<=.7,
            twiceBudgetWithZeroHeadingAge=r['twiceHeldBudget']+fresh_stale))
    result['power']['headProjectionHeldPassingDesigns']=sum(r['headProjectionIdealGap']>r['twiceHeldBudget'] for r in extra)
    result['power']['headProjectionStalePassingDesigns']=sum(r['headProjectionIdealGap']>r['twiceStaleBudget'] for r in extra)
    result['power']['noSlidingFreeHeldPassingDesigns']=sum(r['noSlidingFreeDiagnostic']>r['twiceHeldBudget'] for r in extra)
    result['power']['noSlidingFreeStalePassingDesigns']=sum(r['noSlidingFreeDiagnostic']>r['twiceStaleBudget'] for r in extra)
    eligible=[r for r in extra if r['withinFrozenIntervalDuration']]
    result['power']['withinFrozenIntervalDurationDesigns']=len(eligible)
    result['power']['longerDurationsRequireAnotherArcEstimator']=True
    result['power']['shortIntervalZeroHeadingAgePassingDesigns']={name:sum(r[field]>r['twiceBudgetWithZeroHeadingAge'] for r in eligible)
        for name,field in (('P15','P15SeparationCeiling'),('T16','T16SeparationCeiling'),
            ('P0','headProjectionIdealGap'),('H-free-diagnostic','noSlidingFreeDiagnostic'),('D90','D90IdealSignal'))}
    result['power']['scope']='stable-contact separation bounds and optimistic diagnostic signals on the frozen grid; not a universal impossibility proof or a death-time model'
    result['power']['captureProposed']=False
    result['power']['reason']='unresolved near-equivalent leads/projections fail systematic-uncertainty separation; no tested candidate schedule passes the full staleness screen'
    result['localRehearsal']={k:local[k] for k in ('origin','officialConnections','archive','complete','lives',
        'nearRimAnchors','all','primary','cells','selectorPassedBothCruiseDirections','selectorSha256')}
    sp=wall.module('local_assembly_protocol',wall.REPO/'tools/fidelity/slither_protocol.py')
    decoded=wall.extract(a.local/'capture/bot0.jsonl.gz',sp)
    lives={(l['connection'],l['incarnation']):l for l in decoded['lives']}
    age,unstable,primary=[],0,0
    with (a.local/'intervals-private.jsonl').open(encoding='utf-8') as stream:
        for text in stream:
            r=json.loads(text);life=lives[r['connection'],r['incarnation']]
            first,last=(life['anchors'][r[k]] for k in ('first','last'))
            age.append(max(first['age'],last['age']))
            unstable+=any(e[5]!=first['radius'] for e in life['states'] if first['order']<e[1]<=last['order'])
            primary+=r['primary']
    result['localRehearsal']['primaryBottleneck']=dict(endpointHeadingAgeMaximumMs=wall.quant(age),
        intervalsWithUnchangedReceivedRadius=len(age)-unstable,
        intervalsFailing150msEndpointAge=sum(v>150 for v in age),primaryIntervals=primary)
    result['nativeCheck']=parity
    result['nativeCheck']['interpretation']='exact-center-boundary Python/Rust branch differences are retained; final P16 scores execute the Rust function directly'
    # Keep a bounded witness, with every authored mismatch retained externally.
    result['nativeCheck']['mismatchWitness']=parity['mismatches'][:2]
    del result['nativeCheck']['mismatches']
    result['assembly']=dict(baseAnalysisSha256=wall.sha(a.analysis/'selected-results.json'),
        localSelectorSha256=wall.sha(a.local/'selector-results.json'),
        additionalFields='local full-selector dry run, authored native boundary witnesses and power diagnostics; no new interval selection',
        preliminaryPassesRetainedOutsideGit=True)
    result['decision']='current projection is descriptively compatible; exact rule and alternatives unresolved; no mechanics change or capture proposed'
    result['verificationSha256']={p.name:wall.sha(p) for p in wall.HERE.iterdir() if p.suffix in ('.py','.ps1')}
    wall.dump(a.output,result)
    wall.dump(a.analysis.parent/'supplemental-power-private.json',extra)
    print(json.dumps(dict(decision=result['decision'],localSelectorPassed=local['selectorPassedBothCruiseDirections'])))


if __name__=='__main__':main()
