"""Select complete retention evidence; raw arrays and binaries remain outside Git.

python3 select.py REPOSITORY RAW_TASK_ROOT VALIDATED_REF > results.json
"""
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tarfile
if not __debug__:
    raise SystemExit('Run without optimization')
repo,root=(Path(v).resolve() for v in sys.argv[1:3]);validated=sys.argv[3]
measurement=root/'measurement-v2'
base='339041fec0ebde58e5da0b544a6ab2cc6c720850';candidate='3e22127756da910488ac3f4dbb998806d87a749d'
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
result={'base':base,'candidate':candidate,'validated':validated,'builds':{},'traces':{},'instructions':{},'tickDistributions':{}}
subprocess.run(['git','-C',str(repo),'diff','--exit-code',candidate,validated,'--','src','Cargo.toml','Cargo.lock','build.rs'],check=True)
fixture=Path(__file__).with_name('retention_fixture.rs')
for arm,ref in [('base',base),('candidate',candidate)]:
    folder=measurement/f'native-{arm}';pins=json.loads((folder/'pins.json').read_text())
    assert pins['runtimeCommit']==ref and sha(folder/'source.tar')==pins['archiveSha256']
    binaries=json.loads((folder/'binaries.json').read_text())
    assert set(binaries)=={'study_protocols','study_retention','perf_baseline','perf_battledome','perf_client_retained'}
    for name,digest in binaries.items():assert sha(folder/'bin'/name)==digest
    assert binaries['study_protocols']==pins['binarySha256']
    assert (folder/'source/src/study_retention.rs').read_bytes()==fixture.read_bytes()
    result['builds'][arm]={**pins,'binaries':binaries,'retentionFixtureSha256':sha(fixture)}
assert (measurement/'traces.complete').exists()
standard=json.loads((measurement/'traces/selected.json').read_text())
standard_cases={'official500','official500-half','official500-legacy'}
shrink_cases={'official500-shrink','battledome80-shrink'}
assert set(standard)==standard_cases and all(row['equal'] for row in standard.values())
result['traces']['official']=standard
for name,expected in [('stock-traces',6),('dome-traces',3)]:
    rows=json.loads((measurement/name/'identity.json').read_text());assert len(rows)==expected
    for row in rows:
        assert row['equal'] and row['base']['sha256']==row['candidate']['sha256']
        for arm in ['base','candidate']:
            binary='perf_battledome' if name=='dome-traces' else ('perf_client_retained' if row['case'].startswith('retained-') else 'perf_baseline')
            assert row[arm]['binarySha256']==result['builds'][arm]['binaries'][binary]
    result['traces'][name]=rows
shrinking=json.loads((measurement/'shrink-traces/selected.json').read_text());assert set(shrinking)==shrink_cases
for case,row in shrinking.items():
    assert row['equal'] and row['summary']['ticks']==3200
    left,right=row['records']['base'],row['records']['candidate']
    assert left['sha256']==right['sha256'] and left['bytes']==right['bytes'] and left['records']==right['records']==3202
    for arm in ['base','candidate']:
        assert row['records'][arm]['binarySha256']==result['builds'][arm]['binaries']['study_retention']
result['traces']['shrinking']=shrinking
scheduled=[]
for folder,expected in [('instructions',standard_cases),('shrink-instructions',shrink_cases)]:
    path=measurement/folder/'selected.json';raw=json.loads(path.read_text());assert set(raw)==expected
    selected={}
    for case,entry in raw.items():
        rows=entry['pairs'];assert len(rows)==6 and [r['pair'] for r in rows]==list(range(-1,5))
        assert [r['warmup'] for r in rows]==[True]+[False]*5
        for row in rows:
            for arm in ['base','candidate']:
                stat=measurement/folder/f"{case}-{row['pair']}-{arm}.stat"
                fields=next(line.split(';') for line in stat.read_text().splitlines() if 'instructions:u' in line)
                assert int(fields[0])==row[arm];scheduled.append(float(fields[4]))
            assert row['ratio']==row['candidate']/row['base']
        ratios=[r['candidate']/r['base'] for r in rows[1:]]
        computed={'baseMedian':statistics.median(r['base'] for r in rows[1:]),'candidateMedian':statistics.median(r['candidate'] for r in rows[1:]),
                  'pairedMedian':statistics.median(ratios),'pairedRange':[min(ratios),max(ratios)]}
        assert all(entry[key]==value for key,value in computed.items())
        selected[case]={**computed,'warmupPairs':1,'measuredPairs':5}
    result['instructions'][folder]={'cases':selected,'rawSha256':sha(path)}
assert min(scheduled)>=99;result['minimumProcessCounterScheduledPct']=min(scheduled)

def quantile(values,q):
    values=sorted(values);position=(len(values)-1)*q;low=int(position);high=min(low+1,len(values)-1)
    return values[low]+(values[high]-values[low])*(position-low)
def distribution(values):
    return {'n':len(values),'mean':statistics.fmean(values),'p50':quantile(values,.5),'p90':quantile(values,.9),'p95':quantile(values,.95),'p99':quantile(values,.99),'min':min(values),'max':max(values)}
tick_results=json.loads((measurement/'shrink-ticks/selected.json').read_text());assert set(tick_results)==shrink_cases
for case,entry in tick_results.items():
    physical=None;runs={arm:[] for arm in ['base','candidate']};scheduled=[]
    for i in range(-1,5):
        for arm in ['base','candidate']:
            prefix=measurement/'shrink-ticks'/f'{case}-{i}-{arm}'
            rows=json.loads(prefix.with_suffix('.ticks.json').read_text())
            metadata=json.loads(prefix.with_suffix('.metadata.json').read_text())
            assert len(rows)==len(metadata)==3200 and [r[0] for r in rows]==[r['tick'] for r in metadata]==list(range(3200))
            physical=metadata if physical is None else physical;assert physical==metadata
            for values,world in zip(rows,metadata):
                assert values[2]>0 and values[3]/values[2]>=.99 and values[4]==world['alive']
                scheduled.append(100*values[3]/values[2])
            if i>=0:runs[arm].append([r[1]*r[2]/r[3] for r in rows])
    medians={arm:[statistics.median(values[tick] for values in runs[arm]) for tick in range(3200)] for arm in runs}
    subsets={'shrinking':[r['tick'] for r in physical if r['radiusAfter']<r['radiusBefore']],
             'culling':[r['tick'] for r in physical if r['culled']>0],
             'shrinking_without_cull':[r['tick'] for r in physical if r['radiusAfter']<r['radiusBefore'] and r['culled']==0],
             'unchanged_radius':[r['tick'] for r in physical if r['radiusAfter']==r['radiusBefore']]}
    assert len(subsets['shrinking'])>2000 and len(subsets['culling'])>=25
    for subset,ticks in subsets.items():
        for arm in runs:assert entry['subsets'][subset][arm]==distribution([medians[arm][tick] for tick in ticks])
        assert entry['subsets'][subset]['pairedTickRatios']==distribution([medians['candidate'][tick]/medians['base'][tick] for tick in ticks])
    assert min(scheduled)==entry['minimumScheduledPct']
    assert entry['totalCulled']==sum(r['culled'] for r in physical)
    assert entry['physicalSha256']==hashlib.sha256(json.dumps(physical,sort_keys=True).encode()).hexdigest()
    result['tickDistributions'][case]=entry
checks=root/'checks-final-v2'
with tarfile.open(checks/'source.tar') as f:assert f.pax_headers['comment']==validated
assert f'CHECKS PASS {validated} rust' in (root/'checks-final-v2.log').read_text()
assert '5 passed; 0 failed' in (checks/'focused.log').read_text()
result['checks']={'rustPassed':True,'archiveSha256':sha(checks/'source.tar'),'rustLogSha256':sha(checks/'rust.log'),'focusedLogSha256':sha(checks/'focused.log')}
negative=root/'negative';control=json.loads((negative/'selected.json').read_text())
assert control['runtime']==candidate and control['regressionFailed']
result['negative']={**control,'logSha256':sha(negative/'test.log')}
integration='dea716ef1ea01fe97f0e1bcefda5e725c7c858a9'
checks=root/'checks-integration'
with tarfile.open(checks/'source.tar') as f:assert f.pax_headers['comment']==integration
assert 'CHECKS PASS refs/heads/perf/mirrored-food-retention rust' in (root/'checks-integration.log').read_text()
assert '5 passed; 0 failed' in (checks/'focused.log').read_text()
subprocess.run(['git','-C',str(repo),'diff','--exit-code',candidate,integration,'--','src/simulator.rs','src/food_grid.rs','src/physics.rs','src/bounds_index.rs','Cargo.toml','Cargo.lock','build.rs'],check=True)
result['integration']={'runtime':integration,'main':'ef866d73259e35fe8659a1996d7b97c36d511e0b',
                       'coreMatchesMeasured':True,'rustPassed':True,'archiveSha256':sha(checks/'source.tar'),
                       'rustLogSha256':sha(checks/'rust.log'),'focusedLogSha256':sha(checks/'focused.log')}
assert 'DOCS AND SEVEN GUARDS PASS' in (root/'docs-guards.log').read_text()
result['documentationAndGuards']={'passed':True,'logSha256':sha(root/'docs-guards.log')}
print(json.dumps(result,indent=2))
