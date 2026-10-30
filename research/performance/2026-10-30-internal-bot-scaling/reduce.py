"""Reduce completed guest results; run remotely after all timed work ends."""
import json
import math
from pathlib import Path
import statistics
import sys


def stats(values):
    values=sorted(values)
    if not values:return {'n':0}
    return {'n':len(values),'mean':statistics.mean(values),
            **{name:values[max(0,math.ceil(len(values)*q)-1)]
               for name,q in [('min',0),('p50',.5),('p95',.95),('p99',.99),('max',1)]}}


def mode_summary(rows):
    samples=[s for row in rows for s in row['samples']]
    decisions=sum(s['decisions'] for s in samples)
    observations=sum(s['observations'] for s in samples)
    def per_tick(field):return stats([s[field]*1000 for s in samples])
    def per_bot(field,count):return sum(s[field] for s in samples)*1e6/count if count else None
    return {
        'windows':len(rows),'ticks':len(samples),'decisions':decisions,'observations':observations,
        'worldCpuMs':per_tick('worldCpu'),'worldWallMs':per_tick('worldWall'),
        'admissionCpuMs':per_tick('admissionCpu'),'admissionWallMs':per_tick('admissionWall'),
        'observationWallMs':per_tick('observationWall'),'decisionWallMs':per_tick('decisionWall'),
        'observationProcessCpuUsPerBot':per_bot('observationCpu',observations),
        'observationWallUsPerBot':per_bot('observationWall',observations),
        'policyLoopCpuUsPerBot':per_bot('policyLoopCpu',decisions),
        'decisionProcessCpuUsPerBot':per_bot('decisionCpu',decisions),
        'decisionWallUsPerBot':per_bot('decisionWall',decisions),
        'wholeWindowTicksPerSecond':stats([r['ticks']/r['wallSeconds'] for r in rows]),
        'worldPlusAdmissionCostTicksPerSecond':stats([
            r['ticks']/sum(s['worldCpu']+s['admissionCpu'] for s in r['samples']) for r in rows]),
        'wholeWindowProcessCpuMsPerTick':stats([r['processCpuSeconds']*1000/r['ticks'] for r in rows]),
        'wholeWindowMainCpuMsPerTick':stats([r['mainCpuSeconds']*1000/r['ticks'] for r in rows]),
        'replayWallMs':per_tick('replayWall'),
        'admitted':sum(s['admitted'] for s in samples),'refused':sum(s['refused'] for s in samples),
        'poses':sum(s['poses'] for s in samples),
        'initialMemoryKiB':[r['initial']['memoryKiB'] for r in rows],
        'loadedFixtureMemoryKiB':[r['worldOnlyMemoryKiB'] for r in rows],
        'finalMemoryKiB':[r['final']['memoryKiB'] for r in rows],
        'perWindow':[{'wallSeconds':r['wallSeconds'],'processCpuSeconds':r['processCpuSeconds'],
                     'mainCpuSeconds':r['mainCpuSeconds'],
                     'finalStateSha256':r['finalStateSha256']} for r in rows],
    }


def phase_summary(row,ordinary):
    samples=row['samples']
    phases=['movementSeconds','collisionSeconds','deathSeconds','foodSeconds','preySeconds','ambientSeconds']
    total_cpu=sum(s['worldCpu'] for s in samples)
    total_wall=sum(s['worldWall'] for s in samples)
    result={}
    for phase in phases:
        wall=sum(s['profile'][phase] for s in samples)
        # Any off-CPU interval can lie entirely in this phase. The bounds are
        # conservative individually, and are not additive across phases.
        lower=sum(max(0,s['profile'][phase]-max(0,s['worldWall']-s['worldCpu'])) for s in samples)
        upper=sum(min(s['profile'][phase],s['worldCpu']) for s in samples)
        result[phase]={'wallMsPerTick':wall*1000/len(samples),
                       'cpuMsPerTickBounds':[lower*1000/len(samples),upper*1000/len(samples)],
                       'cpuShareBounds':[lower/total_cpu,upper/total_cpu]}
    phase_wall=sum(s['profile'][key] for s in samples for key in phases)
    assert phase_wall<=total_wall+1e-6
    ordinary_means=[statistics.mean(s['worldCpu'] for s in r['samples']) for r in ordinary]
    profiled_mean=total_cpu/len(samples)
    return {'instrumentedWorldCpuMsPerTick':profiled_mean*1000,
            'worldWallMsPerTick':total_wall*1000/len(samples),
            'offCpuFractionBound':max(0,total_wall-total_cpu)/total_wall,
            'profileToOrdinaryCpuRatios':[profiled_mean/v for v in ordinary_means],
            'phases':result,'unbracketedWallMsPerTick':(total_wall-phase_wall)*1000/len(samples),
            'counts':{key:sum(s['profile'][key] for s in samples)
                      for key in ['ticks','bodyPairs','segmentTests','foodPairs','pickupTests']}}


def reduce(root):
    root=Path(root)
    selected={'pins':json.loads((root/'measurement-pins.json').read_text()),
              'method':{'cadenceTicks':6,'stagger':'(tick + slot) % 6 == 0',
                        'timing':'Timing::default(): all lag and processing fields zero',
                        'world':'Config::official(N), unchanged adaptive radius/density, no wire radius cap',
                        'structurallyAbsent':['wire encoding','socket egress','minimap','leaderboard','ForagerBot frame adapter']},
              'populations':[]}
    for n in [1000,2500,5000,10000]:
        directory=root/'results'/str(n)
        qualification=json.loads((directory/'warm-qualification.json').read_text())
        census=json.loads((directory/'fixture/census.json').read_text())
        config=json.loads((directory/'fixture/world.json').read_text())['config']
        population={'population':n,'qualification':qualification,'census':census,'resolvedConfig':config}
        if not qualification['stabilized']:
            selected['populations'].append(population)
            continue
        serial=json.loads((directory/'verify-serial.json').read_text())
        parallel=json.loads((directory/'verify-parallel.json').read_text())
        assert serial['actionSha256']==parallel['actionSha256']
        reference=serial['finalStateSha256']
        reference_work=tuple(sum(s[key] for s in serial['samples']) for key in ['admitted','refused','poses'])
        population['parity']={'serialParallelActionSha256':serial['actionSha256'],
                              'allFinalStateSha256':reference}
        population['modes']={}
        for mode in ['serial','parallel','simulation']:
            rows=[json.loads((directory/f'pair-{pair}-{mode}.json').read_text()) for pair in range(5)]
            assert all(r['ticks']==512 and len(r['samples'])==512 and not r['profiled'] and not r['verify'] for r in rows)
            assert all(r['finalStateSha256']==reference for r in rows)
            assert all(tuple(sum(s[key] for s in r['samples']) for key in ['admitted','refused','poses'])==reference_work for r in rows)
            population['modes'][mode]=mode_summary(rows)
            if mode!='simulation':
                profile=json.loads((directory/f'profile-{mode}.json').read_text())
                assert profile['finalStateSha256']==reference
                population.setdefault('phaseDiagnostics',{})[mode]=phase_summary(profile,rows)
        selected['populations'].append(population)
    (root/'selected-results.json').write_text(json.dumps(selected,indent=2)+'\n')
    return selected


if __name__=='__main__':
    result=reduce(sys.argv[1])
    print(json.dumps([{'population':p['population'],'stabilized':p['qualification']['stabilized'],
                      'ticksPerSecond':{k:v['wholeWindowTicksPerSecond'] for k,v in p.get('modes',{}).items()}}
                     for p in result['populations']],indent=2))
