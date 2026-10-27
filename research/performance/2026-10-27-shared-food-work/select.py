"""Select complete shared-food evidence; raw arrays and executables stay outside Git.

python3 select.py REPOSITORY RAW_ROOT WHOLE_BASE_PROFILE FOOD_BASE_PROFILE VALIDATED_REF
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
repo, root, whole, food = (Path(v).resolve() for v in sys.argv[1:5])
validated = sys.argv[5]
base = 'ef866d73259e35fe8659a1996d7b97c36d511e0b'
candidate = '8ae1336cc1156063f860ee0d8f77f70c130f64aa'
measurement = root / 'measurement'
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

for label, path, ref, count in [('wholeBase', whole, base, 32), ('foodBase', food, base, 37), ('foodCandidate', root / 'profile-candidate-v2', candidate, 37)]:
    pins = json.loads((path / 'pins.json').read_text())
    assert pins['runtime'] == ref and sha(path / 'source.tar') == pins['archiveSha256']
    assert sha(path / 'bin/study_protocols') == pins['binarySha256']
    selected = json.loads((path / 'selected.json').read_text())
    assert set(selected) == cases
    ticks = None
    for case, row in selected.items():
        data = json.loads((path / f'{case}.phases.json').read_text())
        assert len(data['phases']) == count
        raw = [r for r in data['rows'] if r['tick'] >= 120 and r['tick'] != 720 and r['playing'] == 500]
        ids = [r['tick'] for r in raw]
        ticks = ids if ticks is None else ticks
        assert len(raw) == 853 and ids == ticks
        assert all(r['enabled'] > 0 and r['running'] / r['enabled'] >= .99 for r in raw)
        assert row['minimumScheduledPct'] == min(100 * r['running'] / r['enabled'] for r in raw)
        assert row['instructions'] == statistics.fmean(r['instructions'] for r in raw)
        assert row['phases'] == {name: statistics.fmean(r['phases'][i] for r in raw) for i, name in enumerate(data['phases'])}
        assert row['calls'] == {name: statistics.fmean(r['calls'][i] for r in raw) for i, name in enumerate(data['phases'])}
        assert row['traceEqual'] and row['ticks'] == 853 and row['pins'] == pins
        row['traceIdentity'] = equal_streams(path / f'{case}-profile.trace', path / f'{case}-control.trace', open)
    result['profiles'][label] = selected
checks = root / 'checks'
with tarfile.open(checks / 'source.tar') as stream:
    assert stream.pax_headers['comment'] == validated
assert f'FOOD CHECKS PASS {validated} rust' in (root / 'checks.log').read_text()
assert '12 passed; 0 failed' in (checks / 'focused.log').read_text()
result['checks'] = {'rustPassed': True, 'archiveSha256': sha(checks / 'source.tar'), 'focusedLogSha256': sha(checks / 'focused.log'), 'rustLogSha256': sha(checks / 'rust.log')}
negative = json.loads((root / 'negative-v2/selected.json').read_text())
assert negative['runtime'] == candidate and negative['regressionFailed']
assert negative['logSha256'] == sha(root / 'negative-v2/test.log')
assert negative['originalFoodSha256'] != negative['mutatedFoodSha256']
result['negative'] = negative
assert 'DOCS AND NINE GUARDS PASS' in (root / 'docs-guards.log').read_text()
result['documentationAndGuards'] = {'passed': True, 'logSha256': sha(root / 'docs-guards.log')}
print(json.dumps(result, indent=2))
