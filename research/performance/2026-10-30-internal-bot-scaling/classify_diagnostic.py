"""Conservative source-context buckets for the separate user-CPU samples."""
import collections
import bisect
import json
import math
from pathlib import Path
import re
import sys
from run import digest
from reduce import stats, phase_summary


def records(path):
    record=[]
    with Path(path).open() as stream:
        for line in stream:
            if line.strip():record.append(line)
            elif record:
                yield ''.join(record);record=[]
        if record:yield ''.join(record)


def operation_phase(record):
    if not any(name in record for name in ['5Scene4tick','::Scene::tick','5Scene10storm_tick','::Scene::storm_tick']):return None
    if 'pickup_guard_after_step' in record:return 'guard audit outside world brackets'
    for phase,names in [
        ('A4',['3bot6kernel','7Forager6decide','::bot::kernel::','::bot::Forager::decide']),
        ('observation',['11observation','::observation::']),
        ('admission',['5Scene9admission','::Scene::admission']),
        ('world',['9simulator','::simulator::']),
    ]:
        if any(name in record for name in names):return phase
    return 'other operation'


def header(record):
    match=re.search(r'(\d+\.\d+):\s+(\d+) cpu-clock:u:',record.splitlines()[0])
    return (float(match[1]),int(match[2])) if match else None


def world_phase(record,scope,ranges):
    if scope=='admission':return 'spawn/admission'
    if any(name in record for name in ['::EaterGrid::build','9EaterGrid5build']):return 'pickup grid construction'
    for line in map(int,re.findall(r'/src/simulator\.rs:(\d+)',record)):
        if ranges['pickup grid construction'][0]<=line<=ranges['pickup grid construction'][1]:
            return 'pickup grid construction'
        for label,(first,last) in ranges.items():
            if label!='pickup grid construction' and first<=line<=last:return label
    for label,names in [
        ('collisions',['strikes_scaled','BoundsIndex']),
        ('food pickup/cache',['find_eater','indexed_hits','dense_hits','PelletIndex']),
        ('movement',['advance_directed','advance_segment','advance_motion']),
        ('death processing',['death_food']),('prey',['advance_prey']),
    ]:
        if any(name in record for name in names):return label
    return 'unresolved world / clock brackets'


def diagnostic_costs(row):
    samples=row['samples'];decisions=sum(s['decisions'] for s in samples)
    observations=sum(s['observations'] for s in samples)
    return {'ticks':row['ticks'],'worldCpuMs':stats([s['worldCpu']*1000 for s in samples]),
        'worldPlusAdmissionCpuMs':stats([(s['worldCpu']+s['admissionCpu'])*1000 for s in samples]),
        'wholeWindowTicksPerSecond':row['ticks']/row['wallSeconds'],
        'observationProcessCpuUsPerBot':sum(s['observationCpu'] for s in samples)*1e6/observations if observations else None,
        'policyLoopCpuUsPerBot':sum(s['policyLoopCpu'] for s in samples)*1e6/decisions if decisions else None}


def guard_summary(samples):
    guards=[s['pickupGuard'] for s in samples]
    paths=collections.Counter(g['path'] for g in guards)
    combinations=collections.Counter((g['declineFixedCells'],g['declineFoodLength'],g['declineCoverArea']) for g in guards)
    return {'ticks':len(guards),'fixedCellLimit':1048576,'cellSize':128,'foodGuardMinimum':4096,
        **{key:stats([g[key] for g in guards]) for key in ['boxCells','foodLen','coverArea','circles','foodGuardLimit']},
        'paths':dict(paths),'regularGeometryEveryTick':all(g['regularGeometry'] for g in guards),
        'declines':{key:sum(g[key] for g in guards) for key in ['declineFixedCells','declineFoodLength','declineCoverArea']},
        'combinations':[{'aFixedCells':a,'bFoodLength':b,'cCoverArea':c,'ticks':count}
                        for (a,b,c),count in sorted(combinations.items())]}


def classify(root):
    root=Path(root)
    lines=(root/'src/observation/mod.rs').read_text().splitlines()
    def locate(text):
        found=[i+1 for i,line in enumerate(lines) if text in line]
        assert len(found)==1,(text,found)
        return found[0]
    ranges={
        'food order restoration':(locate('for (word, bits) in marks.iter_mut()'),locate('for (i, p) in w.prey.iter()')-1),
        'prey candidate scan':(locate('for (i, p) in w.prey.iter()'),locate('let visible_opponents =')-1),
        'snake candidate scan':(locate('let visible_opponents ='),locate('for i in visible_opponents {')+2),
        'food candidate queries':(locate('for row in grid.row(b.min.y)'),locate('// Ascending bit order restores')-1),
    }
    overhead=json.loads((root/'diagnostic/overhead.json').read_text())
    source=(root/'src/simulator.rs').read_text().splitlines()
    def sim_line(text):
        found=[i+1 for i,line in enumerate(source) if text in line]
        assert len(found)==1,(text,found)
        return found[0]
    phase_ranges={};start=sim_line('fn advance<const PROFILE: bool>')
    for label,field in [('movement','movement'),('collisions','collision'),('death processing','death'),
                        ('food pickup/cache','food'),('prey','prey'),('ambient/maintenance','ambient')]:
        end=sim_line('profile.'+field+'_seconds +=')
        phase_ranges[label]=(start,end);start=end+1
    phase_ranges['pickup grid construction']=(sim_line('let dense = self.config.food_index'),
                                               sim_line('// Eating changes only mass, which no pickup test reads')-1)
    result={'attribution':'sampled user CPU, source context; inclusive subfunctions assigned once; not removable shares',
            'pins':json.loads((root/'diagnostic/pins.json').read_text()),
            'classifierSha256':digest(Path(__file__)),
            'sourceRanges':ranges,'worldPhaseSourceRanges':phase_ranges,
            'slowTickDefinition':'top ceil(1% * 512) by sampled-run world+admission CPU; also report excluding the first restored tick',
            'windows':[]}
    for label,n in [('1000',1000),('10000',10000),('botstorm-12000',12000)]:
        all_samples=collections.Counter();phases=collections.Counter();observations=collections.Counter()
        sampled=json.loads((root/f'diagnostic/{label}-sampled.json').read_text())
        tick_samples=sampled['samples']
        slow=lambda values:set(s['tick'] for s in sorted(values,key=lambda s:s['worldCpu']+s['admissionCpu'],reverse=True)[:math.ceil(len(values)*.01)])
        slow_ticks=slow(tick_samples);warm_slow=slow(tick_samples[1:])
        intervals=sorted((s[scope+'StartMonotonic'],s[scope+'EndMonotonic'],s['tick'],scope)
                         for s in tick_samples for scope in ['admission','world'])
        assert all(a[1]<=b[0] for a,b in zip(intervals,intervals[1:]))
        starts=[v[0] for v in intervals]
        world_counts={key:collections.Counter() for key in ['all world ticks','slowest 1%','slowest 1% excluding first']}
        world_events=collections.Counter();unparsed=0
        for record in records(root/f'diagnostic/{label}-stacks.txt'):
            parsed=header(record)
            if parsed is None:unparsed+=1;continue
            timestamp,weight=parsed;all_samples['all']+=weight
            position=bisect.bisect_right(starts,timestamp)-1
            if position>=0 and timestamp<intervals[position][1]:
                _,_,tick,scope=intervals[position]
                category=world_phase(record,scope,phase_ranges)
                for key,include in [('all world ticks',True),('slowest 1%',tick in slow_ticks),
                                    ('slowest 1% excluding first',tick in warm_slow)]:
                    if include:world_counts[key][category]+=weight;world_events[key]+=1
            phase=operation_phase(record)
            if phase is None:
                all_samples['outside operation or unwound']+=weight
                continue
            all_samples['operation']+=weight
            phases[phase]+=weight
            if phase=='observation':
                if any(name in record for name in ['8FoodGrid3new','::FoodGrid::new']):bucket='food grid construction'
                elif any(name in record for name in ['8Observer3new','::Observer::new']):bucket='other shared-index construction'
                else:
                    matches=re.findall(r'/src/observation/mod\.rs:(\d+)',record)
                    line=int(matches[0]) if matches else None
                    bucket=next((key for key,(first,last) in ranges.items() if line is not None and first<=line<=last),
                               'other observation / copying / unresolved source')
                observations[bucket]+=weight
        assert all_samples['all']>0,'No recognized cpu-clock periods; inspect perf script format'
        assert all_samples['operation']>0,'No tick stacks recognized; inspect symbols/unwinding'
        assert world_events['all world ticks']>0,'No samples matched monotonic world/admission brackets'
        report=(root/f'diagnostic/{label}-report.txt').read_text()
        lost=re.search(r'Total Lost Samples:\s*([\d,]+)',report)
        def shares(counts,total):
            return {key:{'sampleNanoseconds':value,'share':value/total if total else None}
                    for key,value in counts.most_common()}
        ordinary=json.loads((root/f'diagnostic/{label}-ordinary.json').read_text())
        profiled=json.loads((root/f'diagnostic/{label}-phases.json').read_text())
        assert ordinary['finalStateSha256']==profiled['finalStateSha256']
        guards=lambda row:[s['pickupGuard'] for s in row['samples']]
        assert guards(ordinary)==guards(sampled)==guards(profiled)
        slow_profile={'samples':[s for s in profiled['samples'] if s['tick'] in warm_slow]}
        slow_plain={'samples':[s for s in ordinary['samples'] if s['tick'] in warm_slow]}
        cadence=[]
        for remainder in range(6):
            rows=[s for s in profiled['samples'] if s['tick']%6==remainder]
            cadence.append({'tickMod6':remainder,'ticks':len(rows),
                'worldCpuMsMean':sum(s['worldCpu'] for s in rows)*1000/len(rows),
                'ambientWallMsMean':sum(s['profile']['ambientSeconds'] for s in rows)*1000/len(rows),
                'foodWallMsMean':sum(s['profile']['foodSeconds'] for s in rows)*1000/len(rows)})
        result['windows'].append({'label':label,'population':n,'overhead':next(v for v in overhead if v['label']==label),
             'unsampledDiagnosticCosts':diagnostic_costs(ordinary),
             'stepProfileDiagnostic':phase_summary(profiled,[ordinary]),
             'sameVirtualSlowTicksPhaseDiagnostic':phase_summary(slow_profile,[slow_plain]),
             'phaseCadenceByTickMod6':cadence,
             'initialCensus':ordinary['initial'],'finalCensus':ordinary['final'],
             'pickupGuards':{'method':'post-step circle fields plus final food + food pickups - ambient births; exact pickup-entry reconstruction',
                 'allThreeGuardTracesEqual':True,'allTicks':guard_summary(tick_samples),
                 'slowest1pctExcludingFirst':guard_summary([s for s in tick_samples if s['tick'] in warm_slow])},
             'lostSamples':int(lost[1].replace(',','')) if lost else None,'unparsedRecords':unparsed,
             'coverage':dict(all_samples),
             'slowWorldTicks':[{'tick':s['tick'],'worldCpuMs':s['worldCpu']*1000,
                 'admissionCpuMs':s['admissionCpu']*1000} for s in tick_samples if s['tick'] in slow_ticks],
             'slowWorldTicksExcludingFirst':sorted(warm_slow),
             'worldTickAttribution':{key:{'samples':world_events[key],
                 'weightedSampleNanoseconds':sum(counts.values()),'phases':shares(counts,sum(counts.values()))}
                 for key,counts in world_counts.items()},
             'operationPhases':shares(phases,all_samples['operation']),
             'observationBuckets':shares(observations,phases['observation'])})
    (root/'diagnostic/selected.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))


if __name__=='__main__':classify(sys.argv[1])
