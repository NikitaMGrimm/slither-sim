"""Select the complete periodic-rebuild amendment; full arrays remain outside Git.

python3 select_long.py LONG_ROOT SHORT_ROOT CHECK_ROOT CHECK_LOG MUTATION_ROOT
Print selected JSON only after all gates pass. Requires the unmodified raw roots.
"""
import gzip,hashlib,io,json,statistics,struct,subprocess,sys,tarfile
from pathlib import Path
if not __debug__:raise SystemExit('Run without optimization')
root,short,checks,check_log,mutation=(Path(x).resolve() for x in sys.argv[1:6])
refs={'base':'4532a76f22958ae3abcca801e90edd30ed9f466f','lazy':'8a75634407d8a106aa0480eeb5aab51d218026d1','candidate':'9a3f6f5fe85184e42b32c2251480b94356c9210e'}
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
def counts(v):
    assert v[1]>0 and v[2]/v[1]>=.99
    return v[0]*v[1]/v[2]
def quantile(a,q):
    a=sorted(a);p=(len(a)-1)*q;i=int(p);j=min(i+1,len(a)-1)
    return a[i]+(a[j]-a[i])*(p-i)
def dist(a):
    return {'n':len(a),'mean':statistics.fmean(a),'p50':quantile(a,.5),'p99':quantile(a,.99),'p999':quantile(a,.999),'min':min(a),'max':max(a)} if a else None
def equal(a,b):
    h=hashlib.sha256();n=0
    with gzip.open(a,'rb') as x,gzip.open(b,'rb') as y:
        while True:
            p,q=x.read(1<<20),y.read(1<<20);assert p==q
            if not p:break
            h.update(p);n+=len(p)
    return {'sha256':h.hexdigest(),'bytes':n}
delivery='9a3f6f5fe85184e42b32c2251480b94356c9210e'
refs['candidate']=load(root/'candidate/pins.json')['runtime']
assert refs['candidate']==delivery
result={'refs':refs,'builds':{},'cases':{}}
published=subprocess.check_output(['git','archive',delivery])
def rust_inputs(stream):
    with tarfile.open(fileobj=stream) as f:
        return {m.name:hashlib.sha256(f.extractfile(m).read()).hexdigest() for m in f.getmembers() if m.isfile() and (m.name.startswith('src/') or m.name in ['Cargo.toml','Cargo.lock','build.rs'])}
published_inputs=rust_inputs(io.BytesIO(published))
with (root/'candidate/source.tar').open('rb') as f:assert rust_inputs(f)==published_inputs
result['publishedRuntime']=delivery
result['measuredRustInputsEqualPublished']=True
fixture=probe=None
for arm,ref in refs.items():
    p=root/arm;pin=load(p/'pins.json')
    assert pin['runtime']==ref and sha(p/'source.tar')==pin['archiveSha256'] and sha(p/'binary')==pin['binarySha256']
    for name,digest in pin['overlaySha256'].items():assert sha(p/'source'/name)==digest
    f=sha(p/'source/src/client/simulation/study_long.rs');c=sha(p/'source/src/study_rebuild_probe.rs')
    fixture=fixture or f;probe=probe or c;assert (fixture,probe)==(f,c)
    result['builds'][arm]=pin
for case in ['official500','official500-orbit','battledome80','official500-shrink']:
    arms={};physical=None;summary=None
    for arm in refs:
        prefix=root/f'{case}-{arm}';data=load(prefix.with_suffix('.json'));rows=data['rows'];events=data['rebuilds']
        report=json.loads(prefix.with_suffix('.log').read_text().splitlines()[-1])
        summary=summary or report;assert report==summary
        assert report['ticks']==len(rows)==60000 and [r['tick'] for r in rows]==list(range(60000))
        common=[{k:r[k] for k in ['tick','alive','births','food','radius','maps']} for r in rows]
        physical=physical or common;assert physical==common
        values=[counts(r['counts']) for r in rows]
        for e in events:counts(e['counts'])
        periodic=[e for e in events if e['old']]
        starts=[e['tick'] for e in periodic if e['started']]
        maintenance_ticks={e['tick'] for e in periodic}
        cycle=[];cycles=[]
        for e in periodic:
            if e['started']:assert not cycle
            cycle.append(e)
            if e['completed']:
                cycles.append(cycle);cycle=[]
        arm_result={'ticks':len(rows),'summary':report,
            'minimumScheduledPct':100*min([r['counts'][2]/r['counts'][1] for r in rows]+[e['counts'][2]/e['counts'][1] for e in events]),
            'birthsPerTick':dist([r['births'] for r in rows]),'birthsLast10000PerTick':statistics.fmean(r['births'] for r in rows[-10000:]),
            'food':dist([r['food'] for r in rows]),'alive':dist([r['alive'] for r in rows]),
            'allTickInstructions':dist(values),'steadyTickInstructions':dist(values[120:]),
            'mapTickInstructions':dist([v for v,r in zip(values,rows) if r['maps']]),
            'normalTickInstructions':dist([v for v,r in zip(values,rows) if r['tick']>=120 and not r['maps'] and r['tick'] not in maintenance_ticks]),
            'coldMaintenanceInstructions':dist([counts(e['counts']) for e in events if not e['old']]),
            'maintenanceBatchInstructions':dist([counts(e['counts']) for e in periodic]),
            'cyclesStarted':len(starts),'cyclesCompleted':len(cycles),'maintenanceBatches':len(periodic),
            'firstStartTick':starts[0] if starts else None,'firstStartSeconds':starts[0]*report['dt'] if starts else None,
            'intervalTicks':dist([b-a for a,b in zip(starts,starts[1:])]),
            'intervalSeconds':dist([(b-a)*report['dt'] for a,b in zip(starts,starts[1:])]),
            'rightCensoredTicks':60000-starts[-1] if starts else 60000,
            'cycleDurationTicks':dist([c[-1]['tick']-c[0]['tick']+1 for c in cycles]),
            'cycleInstructions':dist([sum(counts(e['counts']) for e in c) for c in cycles]),
            'rawCountsSha256':sha(prefix.with_suffix('.json')),
        }
        if arm!='base':
            identity=equal(root/f'{case}-base.bin.gz',prefix.with_suffix('.bin.gz'))
            assert identity==load(prefix.with_suffix('.trace.json'))==load((root/f'{case}-base').with_suffix('.trace.json'))
            arm_result['trace']=identity
        arms[arm]=arm_result
    # Check the full binary trace grammar, all frame bytes and endpoint records.
    frames=wire=states=0;last=None
    with gzip.open(root/f'{case}-base.bin.gz','rb') as f:
        while h:=f.read(16):
            assert len(h)==16;tag,n=struct.unpack('<QQ',h);payload=f.read(n);assert len(payload)==n
            if tag in [0,3]:assert json.loads(payload)['state'];states+=1
            elif tag==1:
                tick,slot,_=struct.unpack('<QQQ',payload);assert tick<60000 and slot<summary['seats']
            elif tag==2:frames+=1;wire+=n
            elif tag==4:assert json.loads(payload)==summary
            else:raise AssertionError(tag)
            last=tag
    assert states==2 and last==4 and frames==summary['frames'] and wire==summary['bytes']
    base_rows=load(root/f'{case}-base.json')['rows']
    for arm in ['lazy','candidate']:
        data=load(root/f'{case}-{arm}.json');at={r['tick'] for r in data['rebuilds'] if r['old']}
        arms[arm]['maintenanceTickDeltaFromBase']=dist([counts(r['counts'])-counts(base_rows[r['tick']]['counts']) for r in data['rows'] if r['tick'] in at])
    result['cases'][case]=arms
result['measuredMaximumBatchBelow2M']=all(c['candidate']['maintenanceBatchInstructions']['max']<2_000_000 for c in result['cases'].values())
assert result['measuredMaximumBatchBelow2M']
result['measuredMaximumMaintenanceTickDeltaBelow2M']=all(c['candidate']['maintenanceTickDeltaFromBase']['max']<2_000_000 for c in result['cases'].values())
assert result['measuredMaximumMaintenanceTickDeltaBelow2M']
if '--long-only' in sys.argv[6:]:
    result['scope']='long-run selection only'
    print(json.dumps(result,indent=2));raise SystemExit(0)
assert (short/'traces.complete').exists()
result['shortTraces']={'official':load(short/'traces/selected.json'),'stock':load(short/'stock-traces/identity.json'),'battledome':load(short/'dome-traces/identity.json')}
assert len(result['shortTraces']['official'])+len(result['shortTraces']['stock'])+len(result['shortTraces']['battledome'])==12
expected_cases={'official500','official500-half','official500-legacy'}
assert set(result['shortTraces']['official'])==expected_cases
for case,row in result['shortTraces']['official'].items():
    identity=equal(short/f'traces/{case}-base.bin.gz',short/f'traces/{case}-candidate.bin.gz')
    assert row['equal'] and all(row[k]==v for k,v in identity.items())
for folder in ['stock-traces','dome-traces']:
    for row in load(short/folder/'identity.json'):
        assert row['equal'];equal(short/folder/f"{row['case']}-base.jsonl.gz",short/folder/f"{row['case']}-candidate.jsonl.gz")
result['shortInstructions']={}
result['shortBuilds']={}
for arm,ref in [('base',refs['base']),('candidate',delivery)]:
    pins=load(short/f'native-{arm}/pins.json')
    assert pins['runtimeCommit']==ref and pins['binarySha256']==sha(short/f'native-{arm}/bin/study_protocols') and pins['archiveSha256']==sha(short/f'native-{arm}/source.tar')
    binaries={n:sha(short/f'native-{arm}/bin'/n) for n in ['study_protocols','perf_baseline','perf_battledome','perf_client_retained']}
    result['shortBuilds'][arm]={**pins,'binaries':binaries}
    for folder in ['stock-traces','dome-traces']:
        for row in load(short/folder/'identity.json'):
            binary='perf_battledome' if folder=='dome-traces' else 'perf_client_retained' if row['case'].startswith('retained-') else 'perf_baseline'
            assert row[arm]['binarySha256']==binaries[binary]
short_counts=load(short/'instructions/selected.json');assert set(short_counts)==expected_cases
scheduled=[]
for case,entry in short_counts.items():
    pairs=entry['pairs'];assert [p['pair'] for p in pairs]==list(range(-1,5))
    assert [p['warmup'] for p in pairs]==[True]+[False]*5
    for p in pairs:
        for arm in ['base','candidate']:
            fields=next(x.split(';') for x in (short/'instructions'/f"{case}-{p['pair']}-{arm}.stat").read_text().splitlines() if 'instructions:u' in x)
            assert int(fields[0])==p[arm] and float(fields[4])>=99
            scheduled.append(float(fields[4]))
    ratios=[p['candidate']/p['base'] for p in pairs[1:]]
    assert entry['pairedMedian']==statistics.median(ratios) and entry['pairedRange']==[min(ratios),max(ratios)]
    assert entry['baseMedian']==statistics.median(p['base'] for p in pairs[1:]) and entry['candidateMedian']==statistics.median(p['candidate'] for p in pairs[1:])
    result['shortInstructions'][case]={k:v for k,v in entry.items() if k!='pairs'}
result['minimumShortCounterScheduledPct']=min(scheduled)
with tarfile.open(checks/'source.tar') as f:check_ref=f.pax_headers['comment']
assert check_ref in [refs['candidate'],delivery]
assert f"CORE CHECKS PASS {check_ref}" in check_log.read_text()
assert '75 passed; 0 failed' in (checks/'focused.log').read_text()
with (checks/'source.tar').open('rb') as f:assert rust_inputs(f)==published_inputs
result['checks']={'runtime':check_ref,'rustPassed':True,'rustLogSha256':sha(checks/'rust.log'),'focusedLogSha256':sha(checks/'focused.log'),'archiveSha256':sha(checks/'source.tar')}
negative=load(mutation/'selected.json');assert negative['runtime']==delivery and negative['regressionFailed'] and negative['logSha256']==sha(mutation/'test.log')
result['mutation']=negative
print(json.dumps(result,indent=2))
