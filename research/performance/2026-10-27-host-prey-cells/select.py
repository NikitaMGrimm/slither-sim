"""Select bounded results from the complete prey-cell study.

python3 select.py REPOSITORY RAW_OUTPUT VALIDATED_REF > results.json
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
repo, root = (Path(v).resolve() for v in sys.argv[1:3])
validated = sys.argv[3]
base = 'aee542100496b8cbc031b8ab19bb58e78bd8af2b'
candidate = '2496288635d5d5fc70ed04ca71e79bf8ef111653'
measurement = root/'measurement'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
result = {'base':base,'candidate':candidate,'validated':validated,'pins':{},'instructions':{},'traces':{},'profiles':{}}
subprocess.run(['git','-C',str(repo),'diff','--exit-code',candidate,validated,'--','src','Cargo.toml','Cargo.lock','build.rs'],check=True)
for arm, ref in [('base',base),('candidate',candidate)]:
    folder = measurement/f'native-{arm}'
    pins = json.loads((folder/'pins.json').read_text())
    assert pins['runtimeCommit']==ref and sha(folder/'source.tar')==pins['archiveSha256']
    assert sha(folder/'bin/study_protocols')==pins['binarySha256']
    result['pins'][arm] = pins
assert (measurement/'traces.complete').exists()
official = json.loads((measurement/'traces/selected.json').read_text())
assert len(official)==3 and all(v['equal'] for v in official.values())
result['traces']['official']=official
for name in ['stock-traces','dome-traces']:
    rows=json.loads((measurement/name/'identity.json').read_text())
    assert len(rows)==(6 if name=='stock-traces' else 3)
    for row in rows:
        assert row['equal'] and row['base']['sha256']==row['candidate']['sha256']
        for arm in ['base','candidate']:
            binary='perf_battledome' if name=='dome-traces' else ('perf_client_retained' if row['case'].startswith('retained-') else 'perf_baseline')
            assert sha(measurement/f'native-{arm}'/'bin'/binary)==row[arm]['binarySha256']
    result['traces'][name]=rows
counts=measurement/'instructions/selected.json'
raw=json.loads(counts.read_text())
scheduled=[]
for case, value in raw.items():
    rows=value['pairs']
    assert len(rows)==6 and [r['pair'] for r in rows]==list(range(-1,5))
    assert [r['warmup'] for r in rows]==[True]+[False]*5
    for row in rows:
        for arm in ['base','candidate']:
            stat=measurement/'instructions'/f"{case}-{row['pair']}-{arm}.stat"
            fields=next(line.split(';') for line in stat.read_text().splitlines() if 'instructions:u' in line)
            assert int(fields[0])==row[arm]
            scheduled.append(float(fields[4]))
        assert row['ratio']==row['candidate']/row['base']
    measured=rows[1:]
    assert value['baseMedian']==statistics.median(r['base'] for r in measured)
    assert value['candidateMedian']==statistics.median(r['candidate'] for r in measured)
    assert value['pairedMedian']==statistics.median(r['ratio'] for r in measured)
    assert value['pairedRange']==[min(r['ratio'] for r in measured),max(r['ratio'] for r in measured)]
    result['instructions'][case]={'baseMedian':value['baseMedian'],'candidateMedian':value['candidateMedian'],
        'pairedMedian':value['pairedMedian'],'pairedRange':value['pairedRange'],'measuredPairs':5,'warmupPairs':1}
assert len(raw)==3 and min(scheduled)>=99
result['minimumScheduledPct']=min(scheduled)
result['rawCountsSha256']=sha(counts)
for arm, ref in [('base',base),('candidate',candidate)]:
    folder=root/f'profile-{arm}'
    pins=json.loads((folder/'pins.json').read_text())
    assert pins['runtime']==ref
    selected=json.loads((folder/'selected.json').read_text())
    assert len(selected)==3 and all(r['traceEqual'] and r['ticks']==853 for r in selected.values())
    assert sha(folder/'bin/study_protocols')==pins['binarySha256']
    result['profiles'][arm]={'pins':pins,'cases':selected}
checks=root/'checks-final'
with tarfile.open(checks/'source.tar') as f:
    assert f.pax_headers['comment']==validated
assert f'CHECKS PASS {validated} rust' in (root/'checks-final.log').read_text()
assert '4 passed; 0 failed' in (checks/'focused.log').read_text()
result['checks']={'rustPassed':True,'archiveSha256':sha(checks/'source.tar'),'rustLogSha256':sha(checks/'rust.log'),'focusedLogSha256':sha(checks/'focused.log')}
negative=root/'negative'
control=json.loads((negative/'selected.json').read_text())
assert control['oracleFailed']
result['negative']={**control,'logSha256':sha(negative/'test.log')}
print(json.dumps(result,indent=2))
