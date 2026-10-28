"""Reduce local-only logs to a bounded public result; no raw command lines/paths."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from bisect import bisect_left

STAGES=['movement','collisionBuild','collisionQuery','deathFood','pickupFoodCache','prey','ambient','admission','broadcast','simulationTick','botDecode','botFrame','botPolicy']
STATS=re.compile(r'stats: ticks/s=([\d.]+) deferred_ticks=(\d+) simulation_load=([\d.]+)% players=(\d+) bot_slots=(\d+)')

def parse_stats(line):
    match=STATS.search(line)
    if match is None:return None
    return {'ticksPerSecond':float(match[1]),'deferredTicks':int(match[2]),
            'simulationLoadPercent':float(match[3]),'players':int(match[4]),'botSlots':int(match[5])}

def historical_socket_projection():
    # Rounded published inputs; different hardware, runtime and traffic mix.
    per_connection_seconds=.349/142.851/500
    extra_seconds=per_connection_seconds*2280
    return {'source':'../2026-10-27-c7a-main-recheck/REPORT.md',
            'runtime':'4532a76f22958ae3abcca801e90edd30ed9f466f',
            'measuredConnections':500,'measuredSocketCorePercent':34.9,'measuredTicksPerSecond':142.851,
            'cpuMicrosecondsPerConnectionPerTick':per_connection_seconds*1e6,
            'additionalConnections':2280,'extrapolatedAdditionalCpuMsPerTick':extra_seconds*1000,
            'extrapolatedAdditionalCorePercentAt7ms':100*extra_seconds/.007,
            'limits':'Historical extrapolation only: older EPYC hardware, portable build, short scripted nonboosting socket workload, 100 ms controls and different packet mix. Not calibrated to the current guest, mature A4 world, official delivery or 2281 sockets; not a capacity result or optimization-saving claim.'}

def quantile(values,p):
    ordered=sorted(values)
    return ordered[__import__('math').ceil((len(ordered)-1)*p)] if ordered else None

def cpu_delta(start,end):
    a=start['process'];b=end['process'];dt=end['unix']-start['unix']
    assert a['pid']==b['pid'] and a['start']==b['start'] and dt>0
    threads=[];old={r['pid']:r for r in start['threads']}
    assert set(old)=={r['pid'] for r in end['threads']}, 'thread membership changed'
    for row in end['threads']:
        before=old[row['pid']]
        assert before['start']==row['start'] and before['cpus']==row['cpus']
        threads.append({'role':row['comm'],'userSeconds':row['user']-before['user'],
                        'kernelSeconds':row['system']-before['system'],'affinity':row['cpus']})
    process_cpu=b['user']+b['system']-a['user']-a['system']
    assert abs(sum(r['userSeconds']+r['kernelSeconds'] for r in threads)-process_cpu)<.2, 'thread totals must reconcile with process CPU'
    return {'wallSeconds':dt,'userSeconds':b['user']-a['user'],'kernelSeconds':b['system']-a['system'],
            'corePercent':100*(b['user']+b['system']-a['user']-a['system'])/dt,
            'peakRssMiB':b['peakKiB']/1024,'threads':threads}

def cpu_at(samples,at,member='server'):
    """Interpolate cumulative counters at a census edge, not CPU percentages."""
    samples=[row[member] for row in samples if row.get(member) is not None]
    edge=bisect_left([row['unix'] for row in samples],at)
    assert 0<edge<len(samples), 'census edge must be bracketed by telemetry'
    first,last=samples[edge-1:edge+1]
    span=last['unix']-first['unix'];assert 0<span<2.5
    fraction=(at-first['unix'])/span
    def blend(a,b):
        assert a['pid']==b['pid'] and a['start']==b['start'] and a['cpus']==b['cpus']
        return dict(b,user=a['user']+(b['user']-a['user'])*fraction,
                    system=a['system']+(b['system']-a['system'])*fraction)
    old={r['pid']:r for r in first['threads']}
    value={'unix':at,'process':blend(first['process'],last['process']),
           'threads':[blend(old[row['pid']],row) for row in last['threads']]}
    return value,{'beforeUnix':first['unix'],'afterUnix':last['unix'],'fraction':fraction}

def interference(hold,excluded):
    first={r['pid']:r for r in hold['processStart']['rows']}
    rows=[]
    for last in hold['processEnd']['rows']:
        before=first.get(last['pid'])
        if before is None or before['start']!=last['start'] or last['pid'] in excluded:continue
        cpu=last['user']+last['system']-before['user']-before['system']
        rows.append(cpu)
    hz=__import__('os').sysconf('SC_CLK_TCK')
    first_cpu=[int(x) for x in hold['start'].get('systemCpu','cpu').split()[1:]]
    last_cpu=[int(x) for x in hold['end'].get('systemCpu','cpu').split()[1:]]
    system_delta=[(b-a)/hz for a,b in zip(first_cpu,last_cpu)]
    return {'otherSurvivingProcessesCpuSeconds':sum(rows),'largestOtherProcessCpuSeconds':max(rows,default=0),
            'busyThresholdCpuSeconds':3.,'quietScreenPass':max(rows,default=0)<=3.,
            'systemCpuDeltaSeconds':system_delta,
            'processListStartSha256':hold['processStart']['sha256'],'processListEndSha256':hold['processEnd']['sha256'],
            'limits':'CPU deltas cover processes present at both boundaries; full local-only command-line lists also retained. Short-lived processes require system CPU context.'}

def reduce(root):
    meta=json.loads((root/'summary.json').read_text())
    telemetry=[json.loads(line) for line in (root/'telemetry.jsonl').read_text().splitlines()]
    censuses=[];sim=[];bots=[];stats=[];last_census=None
    for line in (root/'host.log').read_text().splitlines():
        if line.startswith('CENSUS '):
            row=json.loads(line[7:]);assert len(row['tickNanos'])==100
            if censuses:assert row['tick']==censuses[-1]['tick']+100
            censuses.append(row);last_census=row['unix']
        elif line.startswith('SIM_PROFILE '):
            row=json.loads(line[12:])
            if row['calls'][9]:sim.append(row)
        elif line.startswith('BOT_PROFILE '):
            row=json.loads(line[12:])
            if row['calls'][12]:bots.append(row)
        elif parsed:=parse_stats(line):
            stats.append(dict(parsed,afterCensusUnix=last_census))
    result={k:meta[k] for k in ['bots','seats','serverCpu','clientCpu','arm','binarySha256','status','screen','warmBlocks'] if k in meta}
    result['peakRssMiBWholeRun']=meta['lastServer']['process']['peakKiB']/1024
    result['affinityTransition']={}
    for label in ['warmPin','holdPin']:
        pin=meta[label]
        result['affinityTransition'][label]={
            'startUnix':pin['startUnix'],'endUnix':pin['endUnix'],
            'threads':[{'role':row['comm'],'cpus':row['cpus']} for row in pin['server']['threads']],
            'fullProcessListSha256':pin['processes']['sha256']}
    assert all(row['cpus']==[meta['serverCpu']] for row in meta['holdPin']['server']['threads'])
    assert meta['holds'][0]['start']['unix']-meta['holdPin']['endUnix']>=59., 'post-pin settling minute required'
    result['holds']=[]
    for hold in meta['holds']:
        start=hold['start']['unix'];end=hold['end']['unix']
        rows=[r for r in censuses if start<=r['unix']<=end]
        assert len(rows)>2
        for row in rows:assert 0<row['living']<=meta['seats'] and row['occupied']<=meta['seats']
        ticks=rows[-1]['tick']-rows[0]['tick'];wall=rows[-1]['unix']-rows[0]['unix']
        durations=[v/1e6 for row in rows[1:] for v in row['tickNanos']]
        assert len(durations)==ticks
        overlapping=[(previous,row) for previous,row in zip(censuses,censuses[1:])
                     if previous['unix']<end and row['unix']>start]
        overlapping_durations=[v/1e6 for _,row in overlapping for v in row['tickNanos']]
        assert overlapping[0][0]['unix']<=start and overlapping[-1][1]['unix']>=end
        per_cpu=cpu_delta(hold['start'],hold['end'])
        assert per_cpu['wallSeconds']>=299.9, 'primary hold must cover 300 seconds'
        assert all(r['affinity']==[meta['serverCpu']] for r in per_cpu['threads'])
        host_stats=[r for r in stats if r['afterCensusUnix'] and start+10<=r['afterCensusUnix']<=end-10]
        aligned_start,start_edge=cpu_at(telemetry,rows[0]['unix'])
        aligned_end,end_edge=cpu_at(telemetry,rows[-1]['unix'])
        aligned=cpu_delta(aligned_start,aligned_end)
        total_cpu=aligned['userSeconds']+aligned['kernelSeconds']
        world_cpu=sum(r['userSeconds']+r['kernelSeconds'] for r in aligned['threads']
                      if r['role'].startswith(('slither-simulat','slither-sockets')))
        def budget(cpu):
            milliseconds=cpu/ticks*1000
            return {'cpuMsPerTick':milliseconds,'requiredCorePercentAt7ms':100*milliseconds/7,
                    'impliedTicksPerSecondAtOneCore':1000/milliseconds,
                    'projectedScheduledTicksPerSecond':min(1000/7,1000/milliseconds),
                    'observedCorePercent':100*cpu/wall}
        result['holds'].append({'server':per_cpu,'client':cpu_delta(hold['clientStart'],hold['clientEnd']),
            'tickAlignedCpu':{'server':aligned,'startInterpolation':start_edge,'endInterpolation':end_edge,
                'everythingBudget':budget(total_cpu),'worldAndOneSocketBudget':budget(world_cpu),
                'limits':'Cumulative CPU counters interpolated at census edges from ~1-second /proc samples. Implied solo throughput assumes unchanged per-tick work/execution cost; not a separate run with remote bots or 2281 sockets.'},
            'tickWindow':{'ticks':ticks,'wallSeconds':wall,'ticksPerSecond':ticks/wall,
                          'deferredTicks':rows[-1]['deferred']-rows[0]['deferred'],
                          'firstTick':rows[0]['tick'],'lastTick':rows[-1]['tick'],
                          'cpuPerTickMsApprox':(per_cpu['userSeconds']+per_cpu['kernelSeconds'])/per_cpu['wallSeconds']/(ticks/wall)*1000},
            'tickDurationMs':{key:quantile(durations,p) for key,p in [('p50',.5),('p90',.9),('p95',.95),('p99',.99),('max',1)]},
            'durationBoundaryCoverage':{'interiorTicks':ticks,'overlappingTicks':len(overlapping_durations),
                'wholeHoldMaxUpperBoundMs':max(overlapping_durations),
                'maxExactlyDetermined':max(overlapping_durations)==max(durations),
                'limits':'Percentiles use complete interior 100-tick blocks. The overlapping-block maximum conservatively covers the entire CPU hold plus boundary ticks; equality with interior max determines the whole-hold maximum.'},
            'census':{'samples':len(rows),'livingMin':min(r['living'] for r in rows),'livingMean':sum(r['living'] for r in rows)/len(rows),
                      'occupiedMin':min(r['occupied'] for r in rows),'occupiedMax':max(r['occupied'] for r in rows),
                      'lengthEndpointChangePercent':{key:100*(rows[-1][key]/rows[0][key]-1) for key in ['meanSegments','p50Segments','p90Segments']},
                      'start':{k:v for k,v in rows[0].items() if k not in ['tickNanos','unix']},
                      'end':{k:v for k,v in rows[-1].items() if k not in ['tickNanos','unix']}},
            'hostStats':host_stats,'interference':interference(hold,{meta['serverPid'],meta['clientPid']})})
    def aggregate(rows):
        return {'instructions':[sum(r['instructions'][i] for r in rows) for i in range(24)],
                'calls':[sum(r['calls'][i] for r in rows) for i in range(24)],'reports':len(rows)}
    if 'profileStart' in meta:
        s=aggregate(sim);b=aggregate(bots)
        ticks=s['calls'][9]
        assert all(s['calls'][i]==ticks for i in range(7)), 'one world phase per counted tick required'
        assert b['calls'][11]==b['calls'][12], 'one materialized frame per policy call required'
        result['profile']={'serverCpu':cpu_delta(meta['profileStart'],meta['profileEnd']),
            'simulation':s,'bots':b,'countedTicks':ticks,'countedDecisions':b['calls'][12],
            'phasesPerTick':{STAGES[i]:s['instructions'][i]/ticks for i in range(10)} if ticks else {},
            'botPerDecision':{STAGES[i]:b['instructions'][i]/b['calls'][12] for i in range(10,13)} if b['calls'][12] else {},
            'limits':'Current-thread user instructions. Bot one-second reports and simulation 100-tick reports have independent edge intervals. No kernel instruction claim; nested broadcast/admission rows are subsets of simulationTick.'}
        threads=root/'thread-instructions.json'
        if threads.exists():
            counts=json.loads(threads.read_text())
            assert all(r['scheduledFraction']>=.99 for r in counts['threads'])
            result['profile']['threadInstructions']=counts
        locations=root/'ip-samples/selected.json'
        if locations.exists():
            sampled=json.loads(locations.read_text())
            assert sampled['lostSamples']==0
            assert all(row['runningNanos']/row['enabledNanos']>=.99 for row in sampled['coverage'])
            for field in ['self','inlineInclusive']:
                complete=sampled[field]
                roles=sorted({row['role'] for row in complete})
                sampled[field+'AllWeightByRole']={role:sum(row['instructionWeight'] for row in complete if row['role']==role) for role in roles}
                sampled[field]=[row for role in roles for row in [r for r in complete if r['role']==role][:20]]
            sampled['selection']='Top 20 executable self/inline functions per thread; all module/unattributed weights retained. Complete raw location tables remain local-only.'
            result['profile']['instructionLocations']=sampled
    result['rawHostLogSha256']=hashlib.file_digest((root/'host.log').open('rb'),'sha256').hexdigest()
    return result

def reduce_plain(root):
    meta=json.loads((root/'summary.json').read_text())
    assert meta['status']=='complete'
    start,end=meta['serverStart'],meta['serverEnd']
    assert all(row['cpus']==[0] for point in [start,end] for row in point['threads'])
    assert all(row['cpus']==[1] for point in [meta['clientStart'],meta['clientEnd']] for row in point['threads'])
    # Stats are five-second deltas, timestamped on read within ~0.1 seconds.
    # Discard both edges rather than label straddling intervals as a full hold.
    stats=[dict(parse_stats(row['line']),readUnix=row['readUnix']) for row in meta['stats']
           if start['unix']+6<=row['readUnix']<=end['unix']-1]
    assert stats and all(row['players']==1 and row['botSlots']==2280 for row in stats)
    hold=dict(start=start,end=end,processStart=meta['processStart'],processEnd=meta['processEnd'])
    quiet=interference(hold,{start['process']['pid'],meta['clientStart']['process']['pid']})
    assert quiet['quietScreenPass'], 'busy pristine hold requires retained repetition'
    server=cpu_delta(start,end)
    assert server['wallSeconds']>=299.9
    return {'status':meta['status'],'arm':'plain','seats':2281,'bots':2280,
            'warmupSeconds':meta['warmupSeconds'],'server':server,
            'client':cpu_delta(meta['clientStart'],meta['clientEnd']),
            'hostStats':stats,'interference':quiet,
            'rawHostLogSha256':hashlib.file_digest((root/'host.log').open('rb'),'sha256').hexdigest(),
            'limits':'Pristine CLI; 30-second cold warmup then 300-second CPU hold. No length census or duration hooks. Statistics retain complete interior five-second intervals only. Not a mature-length qualification or a matched hook-overhead estimate.'}

if __name__=='__main__':
    if not __debug__:raise SystemExit('Run without Python optimization')
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('output',type=Path);p.add_argument('runs',nargs='+',type=Path)
    p.add_argument('--plain-control',type=Path)
    a=p.parse_args();runs=[reduce(root) for root in a.runs]
    for run in runs:
        assert run['status']=='complete' and len(run['holds'])==3
        assert 'screen' in run and run['profile']['countedTicks']>0 and run['profile']['countedDecisions']>0
        assert 'threadInstructions' in run['profile'], 'inclusive thread PMU run required'
        assert all(hold['interference']['quietScreenPass'] for hold in run['holds']), 'busy hold requires retained repetition'
    result={'format':'slither-hosted-capacity-2026-10-28-v1','runs':runs,
            'historicalSocketExtrapolation':historical_socket_projection()}
    if a.plain_control:result['pristineColdControl']=reduce_plain(a.plain_control)
    a.output.write_text(json.dumps(result,indent=2)+'\n')
