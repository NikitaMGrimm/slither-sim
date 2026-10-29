"""Select cited aggregates; raw per-tick rows and IPs stay outside Git."""
import json, sys, hashlib
from pathlib import Path

raw = Path(sys.argv[1]).resolve()
out = raw / 'selected'
out.mkdir(exist_ok=True)
for name in ['counter-results.json', 'footprint-results.json', 'events.json']:
    (out / name).write_bytes((raw / name).read_bytes())
profiles = json.loads((raw / 'profile-results.json').read_text())
selected = []
for p in profiles:
    p = dict(p)
    if p['primary']:
        p['leafFunctions'] = p['leafFunctions'][:6]
        p['sourceSites'] = p['sourceSites'][:8]
    else:
        for field in ['leafFunctions', 'sourceSites', 'sourceGroups']:
            p.pop(field)
    selected.append(p)
(out / 'profile-results.json').write_text(json.dumps(selected, indent=2) + '\n')
build = json.loads((raw / 'build.json').read_text())
repeat = json.loads((raw / 'profile/build.json').read_text())
if (raw / 'reconstruction.json').exists():
    reconstruction = json.loads((raw / 'reconstruction.json').read_text())
else:
    # A fresh reproduction can verify the published whole-generator identity
    # without access to this run's local-only original source directory.
    files = sorted((raw / 'source').rglob('*.rs'),
                   key=lambda f: f.relative_to(raw / 'source').as_posix())
    digest = hashlib.sha256()
    for f in files:
        name = f.relative_to(raw / 'source').as_posix()
        digest.update(name.encode() + b'\0' + f.read_text().encode() + b'\0')
    c_digest = hashlib.sha256((raw / 'source/src/study_events.c').read_text().encode()).hexdigest()
    assert digest.hexdigest() == '72121b58959b3da7f081ab5d98cf884c47e63800aa3c1d92cb25b659ddf34164'
    assert c_digest == 'b851cf9a88bbee6a4b56517a67f1f8bfeaa2572744d8d1dd9307416e0bf77b66'
    reconstruction = {'matchesPublishedGeneratorIdentity': True, 'rustFiles': len(files),
                      'normalizedAggregateRustSha256': digest.hexdigest(),
                      'normalizedCollectorSha256': c_digest}
rows = json.loads((raw / 'counter-results.json').read_text())
assert len(rows) == 24
assert all(len(r['runs']) == 3 and
           all(v['ticks'] == (15 if r['maintenance'] else 1065) for v in r['runs'])
           for r in rows)
meta = {
    'runtime': build['runtime'], 'archiveSha256': build['archiveSha256'],
    'binarySha256': build['binarySha256'], 'compiler': build['compiler'],
    'buildCommand': build['command'],
    'codegen': 'portable release, thin LTO, one codegen unit, debug symbols; no affinity',
    'lowerRateBinarySha256': repeat['binarySha256'],
    'frozenPlanCommit': '7260e8d3', 'samplingCorrectionCommit': '9c060bdb',
    'reconstruction': reconstruction,
    'counterRuns': 36, 'counterTicksPerRun': 1080,
    'maintenanceTicksPerRun': 15, 'ordinaryTicksPerRun': 1065,
    'minimumCounterRunningFraction': min(v['minimumRunningFraction'] for r in rows for v in r['runs']),
    'sameProtocolForkSummariesEqual': True,
    'allPrimaryProfilesUnthrottledAndLossless': all(p['lost'] == 0 and not p['recordTypes'].get('5')
                                                and not p['recordTypes'].get('6')
                                                for p in profiles if p['primary']),
    'rawAccess': 'local-only',
    'unavailable': {'perfExecutable': True, 'L2Alias': True,
                    'genericLLCLoadMissErrno': 2, 'preciseIP': True,
                    'zenTopDownMetrics': True},
}
meta['census'] = {}
for protocol in ['stock', 'legacy']:
    ready = [json.loads(line) for line in (raw / (protocol + '-parent.log')).read_text().splitlines()
             if line.startswith('{') and '"ready":true' in line][0]
    repeated = [json.loads(line) for line in (raw / ('profile-' + protocol + '-parent.log')).read_text().splitlines()
                if line.startswith('{') and '"ready":true' in line][0]
    assert ready['summary'] == repeated['summary'], 'different frozen backend in repeat'
    end = json.loads((raw / (protocol + '-e1-r0.json')).read_text())['summary']
    meta['census'][protocol] = {'startTick': ready['summary']['tick'],
                              'start': ready['summary']['census'],
                              'endTick': end['tick'], 'end': end['census']}
assert meta['minimumCounterRunningFraction'] >= .99
assert len([p for p in profiles if p['primary']]) == 16
assert meta['allPrimaryProfilesUnthrottledAndLossless']
(out / 'verification.json').write_text(json.dumps(meta, indent=2) + '\n')
print('Selected aggregate bytes:', sum(p.stat().st_size for p in out.iterdir()))
