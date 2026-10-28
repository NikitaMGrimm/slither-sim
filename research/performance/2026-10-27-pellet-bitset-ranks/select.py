"""Select complete pellet-rank evidence; raw arrays and executables stay outside Git.

python3 select.py REPOSITORY RAW_ROOT CHECK_ROOT CHECK_LOG PILOT_ROOT
"""
import gzip
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tarfile

if not __debug__:
    raise SystemExit('Run without optimization')
repo, root = (Path(v).resolve() for v in sys.argv[1:3])
validated = '47e416a3eacc99163a3af12fa5eb11af317f9e28'
base = '4532a76f22958ae3abcca801e90edd30ed9f466f'
candidate = '47e416a3eacc99163a3af12fa5eb11af317f9e28'
measurement = root / 'integrated-measurement'
cases = {'official500', 'official500-half', 'official500-legacy'}
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
result = {'base': base, 'candidate': candidate, 'validated': validated, 'builds': {}, 'traces': {}, 'instructions': {}, 'profiles': {}}
subprocess.run(['git', '-C', str(repo), 'diff', '--exit-code', candidate, validated, '--', 'src', 'Cargo.toml', 'Cargo.lock', 'build.rs'], check=True)

def equal_streams(left, right, opener):
    digest = hashlib.sha256()
    size = 0
    with opener(left, 'rb') as a, opener(right, 'rb') as b:
        while True:
            x, y = a.read(1 << 20), b.read(1 << 20)
            assert x == y, (left.name, right.name)
            if not x:
                break
            digest.update(x)
            size += len(x)
    return {'sha256': digest.hexdigest(), 'bytes': size}

for arm, ref in [('base', base), ('candidate', candidate)]:
    folder = measurement / f'native-{arm}'
    pins = json.loads((folder / 'pins.json').read_text())
    assert pins['runtimeCommit'] == ref and sha(folder / 'source.tar') == pins['archiveSha256']
    binaries = {name: sha(folder / 'bin' / name) for name in ['study_protocols', 'perf_baseline', 'perf_battledome', 'perf_client_retained']}
    assert binaries['study_protocols'] == pins['binarySha256']
    result['builds'][arm] = {**pins, 'binaries': binaries}
assert (measurement / 'traces.complete').exists()
rows = json.loads((measurement / 'traces/selected.json').read_text())
assert set(rows) == cases
for case, row in rows.items():
    assert row['equal'] and row['summary']['case'] == case
    identity = equal_streams(measurement / f'traces/{case}-base.bin.gz', measurement / f'traces/{case}-candidate.bin.gz', gzip.open)
    assert all(row[key] == value for key, value in identity.items())
result['traces']['official'] = rows
for folder, names in [('stock-traces', {'portable', 'protocol', 'client-official-500', 'client-battledome-64', 'retained-step_all', 'retained-next_frame'}),
                      ('dome-traces', {'client-battledome-40', 'client-battledome-64', 'client-battledome-80'})]:
    rows = json.loads((measurement / folder / 'identity.json').read_text())
    assert {row['case'] for row in rows} == names and len(rows) == len(names)
    for row in rows:
        case = row['case']
        assert row['equal']
        identity = equal_streams(measurement / folder / f'{case}-base.jsonl.gz', measurement / folder / f'{case}-candidate.jsonl.gz', gzip.open)
        binary = 'perf_battledome' if folder == 'dome-traces' else ('perf_client_retained' if case.startswith('retained-') else 'perf_baseline')
        for arm in ['base', 'candidate']:
            assert all(row[arm][key] == value for key, value in identity.items())
            assert row[arm]['binarySha256'] == result['builds'][arm]['binaries'][binary]
    result['traces'][folder] = rows
scheduled = []
rows = json.loads((measurement / 'instructions/selected.json').read_text())
assert set(rows) == cases
for case, entry in rows.items():
    pairs = entry['pairs']
    assert len(pairs) == 6 and [r['pair'] for r in pairs] == list(range(-1, 5))
    assert [r['warmup'] for r in pairs] == [True] + [False] * 5
    for row in pairs:
        for arm in ['base', 'candidate']:
            prefix = measurement / 'instructions' / f"{case}-{row['pair']}-{arm}"
            fields = next(line.split(';') for line in prefix.with_suffix('.stat').read_text().splitlines() if 'instructions:u' in line)
            assert int(fields[0]) == row[arm]
            scheduled.append(float(fields[4]))
            assert json.loads(prefix.with_suffix('.log').read_text().splitlines()[-1]) == result['traces']['official'][case]['summary']
        assert row['ratio'] == row['candidate'] / row['base']
    ratios = [r['candidate'] / r['base'] for r in pairs[1:]]
    selected = {'baseMedian': statistics.median(r['base'] for r in pairs[1:]),
                'candidateMedian': statistics.median(r['candidate'] for r in pairs[1:]),
                'pairedMedian': statistics.median(ratios), 'pairedRange': [min(ratios), max(ratios)]}
    assert all(entry[key] == value for key, value in selected.items())
    result['instructions'][case] = {**selected, 'warmupPairs': 1, 'measuredPairs': 5}
assert min(scheduled) >= 99
result['minimumProcessCounterScheduledPct'] = min(scheduled)

# Preserve the completed pre-integration matrix separately from final pairs.
before=json.loads((root/'measurement/instructions/selected.json').read_text())
assert set(before)==cases
result['preIntegrationRuntime']={'base':'194d13047775cca92e2a7f5163e9fdd28e918073','candidate':'81c3e503ea657f5e9416910bb4d9c8841efb9774'}
result['preIntegrationInstructions']={}
for case,entry in before.items():
    pairs=entry['pairs']
    assert [r['pair'] for r in pairs]==list(range(-1,5))
    for pair in pairs:
        for arm in ['base','candidate']:
            fields=next(line.split(';') for line in (root/'measurement/instructions'/f"{case}-{pair['pair']}-{arm}.stat").read_text().splitlines() if 'instructions:u' in line)
            assert int(fields[0])==pair[arm] and float(fields[4])>=99
    ratios=[r['candidate']/r['base'] for r in pairs[1:]]
    assert entry['pairedMedian']==statistics.median(ratios)
    assert entry['pairedRange']==[min(ratios),max(ratios)]
    assert entry['baseMedian']==statistics.median(r['base'] for r in pairs[1:])
    assert entry['candidateMedian']==statistics.median(r['candidate'] for r in pairs[1:])
    result['preIntegrationInstructions'][case]={k:v for k,v in entry.items() if k!='pairs'}

phase_names=['validation','setup','movement_boost_body','collision_records','collision_order_index','collision_tests','death_drops','pickup_cache','prey','refill_admission','finish','pre_world']
sub_names=['index_build_collision_and_prey','collision_candidate_query','collision_exact','movement_math','append_contract_body','speed','kiosk','cache_remove','eater_grid_build','pellet_lookup_tests_sort','admission_has_room','cache_push','prey_all','update_mass','validation_all','prey_decisions','pickup_circle','prey_motion','unused','pellet_position_or_rank_refresh']
profile_ticks=None
for label,path,ref in [('base',root/'base-profile','194d13047775cca92e2a7f5163e9fdd28e918073'),('candidate',root/'candidate-profile','81c3e503ea657f5e9416910bb4d9c8841efb9774')]:
    control=path/'control'
    pins=json.loads((control/'pins.json').read_text())
    assert pins['runtime']==ref and sha(control/'binary')==pins['binarySha256'] and sha(control/'source.tar')==pins['archiveSha256']
    profiles=json.loads((path/'selected.json').read_text());assert set(profiles)=={'coarse','nested'}
    for mode,row in profiles.items():
        folder=path/mode;raw=json.loads((folder/'phases.json').read_text())
        ticks=[r['tick'] for r in raw];profile_ticks=ticks if profile_ticks is None else profile_ticks
        assert len(raw)==853 and ticks==profile_ticks and len(set(ticks))==853 and all(t>=120 and t!=720 for t in ticks)
        means={};scheduled=[]
        for stage,name in enumerate(phase_names):
            values=[]
            for item in raw:
                found=[v for v in item['phases'] if v[0]==stage];assert len(found)==1
                _,count,enabled,running=found[0];assert enabled>0 and running/enabled>=.99
                scheduled.append(running/enabled);values.append(count*enabled/running)
            means[name]=statistics.fmean(values)
        assert row['runtime']==ref and row['ticks']==853 and row['phases']==means
        assert row['worldTotal']==sum(means[n] for n in phase_names[:-1])
        assert row['minimumScheduledPct']==100*min(scheduled)
        assert row['nestedInstructions']=={n:statistics.fmean(r['sub'][i] for r in raw) for i,n in enumerate(sub_names)}
        assert row['nestedCalls']=={n:statistics.fmean(r['calls'][i] for r in raw) for i,n in enumerate(sub_names)}
        assert sha(folder/'binary')==row['binarySha256'] and sha(control/'binary')==row['controlSha256']
        assert row['pins']==json.loads((folder/'source-pins.json').read_text())
        assert row['pins']['ref']==ref and row['pins']['archiveSha256']==sha(folder/'source.tar')
        for file,digest in row['pins']['overlay'].items():assert sha(folder/'source'/file)==digest
        identity=equal_streams(folder/'trace',control/'trace',open)
        assert row['traceEqual'] and row['traceSha256']==identity['sha256']
    result['profiles'][label]=profiles
for profile_ref, final_ref in [('194d13047775cca92e2a7f5163e9fdd28e918073',base),('81c3e503ea657f5e9416910bb4d9c8841efb9774',candidate)]:
    changed=subprocess.check_output(['git','-C',str(repo),'diff','--name-only',profile_ref,final_ref,'--','src','Cargo.toml','Cargo.lock','build.rs'],text=True).splitlines()
    assert changed==['src/client/simulation/broadcast.rs'], changed
result['profileIntegration']={'coreSourcesUnchanged':True,'commonHostChange':'src/client/simulation/broadcast.rs'}
checks=Path(sys.argv[3]).resolve()
with tarfile.open(checks/'source.tar') as stream:assert stream.pax_headers['comment']==validated
assert f'CORE CHECKS PASS {validated}' in Path(sys.argv[4]).read_text()
assert '74 passed; 0 failed' in (checks/'focused.log').read_text()
assert (checks/'pickup.log').read_text().count('test result: ok.') == 2
assert '0 failed' in (checks/'pickup.log').read_text()
assert 'live_ranks_cross_word_boundaries_and_survive_lazy_rebuilds ... ok' in (checks/'rust.log').read_text()
result['checks']={'rustPassed':True,'archiveSha256':sha(checks/'source.tar'),'focusedLogSha256':sha(checks/'focused.log'),'pickupLogSha256':sha(checks/'pickup.log'),'rustLogSha256':sha(checks/'rust.log')}
negative=root/'negative-v2';control=json.loads((negative/'selected.json').read_text())
assert control['runtime']==candidate and control['regressionFailed']
assert control['logSha256']==sha(negative/'test.log')
with tarfile.open(negative/'source.tar') as stream:
    assert stream.pax_headers['comment']==candidate
    assert hashlib.sha256(stream.extractfile('src/food_grid.rs').read()).hexdigest()==control['originalFoodGridSha256']
assert sha(negative/'source/src/food_grid.rs')==control['mutatedFoodGridSha256']
result['negative']=control
assert 'DOCS AND NINE GUARDS PASS' in (root/'docs-guards.log').read_text()
result['documentationAndGuards']={'passed':True,'logSha256':sha(root/'docs-guards.log')}
# Feasibility pilots share one immutable current-main binary, but retain their
# own overlaid sources, traces and alternating pairs outside Git.
pilots=Path(sys.argv[5]).resolve()
result['pilots']={}
for name,folder,names in [
    ('collision_memberships',pilots,['bounds_index.rs','simulator.rs']),
    ('cover_floor',pilots/'pickup-cover-floor',['food_grid.rs']),
    ('rank_and_floor',pilots/'pickup-rank-floor',['food_grid.rs']),
]:
    data=json.loads((folder/'selected.json').read_text())
    assert data['base']=='194d13047775cca92e2a7f5163e9fdd28e918073' and data['traceEqual']
    assert [r['pair'] for r in data['pairs']]==list(range(-1,5))
    for pair in data['pairs']:
        for arm in ['base','candidate']:
            stat=next(s.split(';') for s in (folder/f"{pair['pair']}-{arm}.stat").read_text().splitlines() if 'instructions:u' in s)
            assert int(stat[0])==pair[arm] and float(stat[4])>=99
    ratios=[r['candidate']/r['base'] for r in data['pairs'][1:]]
    assert data['medianPairedReduction']==100*(1-statistics.median(ratios))
    assert data['range']==[100*(1-max(ratios)),100*(1-min(ratios))]
    candidate_folder=folder/'candidate' if name=='collision_memberships' else folder
    assert sha(candidate_folder/'binary')==data['binarySha256']['candidate']
    assert sha(pilots/'base/binary')==data['binarySha256']['base']
    identity=equal_streams(candidate_folder/'trace',pilots/'base/trace',open)
    assert identity['sha256']==result['profiles']['base']['coarse']['traceSha256']
    source_hashes={n:sha(candidate_folder/'source/src'/n) for n in names}
    if name=='collision_memberships':
        for n,digest in source_hashes.items():
            assert digest==data['overlaySha256'][n], n
    result['pilots'][name]={**{k:v for k,v in data.items() if k!='pairs'},
        'sourceSha256':source_hashes}
print(json.dumps(result,indent=2,allow_nan=False))
