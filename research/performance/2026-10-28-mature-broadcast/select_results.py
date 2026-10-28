"""Select bounded public results from the retained task artifact root.

Read-only on inputs; full checkpoints, census logs, process inventories and
binary/source trees remain outside Git. Output goes to stdout.
"""
import hashlib
import json
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
read = lambda p: json.loads(p.read_text())
mature = root / 'mature'
remaining = root / 'remaining'
out = {
    'environment': 'a 4-vCPU Zen 5 EPYC KVM guest',
    'matureHostStatus': 'unavailable: missing food-presentation birth provenance',
    'mature249Saving': None,
    'rawAccess': 'local-only; checkpoints, traces, inventories, sources and binaries are not downloadable',
    'construction': {},
    'phaseScope': {
        1: 'outer fixture', 2: 'host tick', 3: 'world physics',
        4: 'food-interest advance', 5: 'broadcast inclusive',
        6: 'snake coverage', 7: 'snake encoding/history', 8: 'recipient food',
        9: 'recipient prey', 10: 'whole compact_food',
        11: 'collision bounds/index build', 12: 'collision query/exact tests'},
    'shortBodyPhaseCounts': read(remaining / 'results.json'),
    'pins': {},
}
for label, folder, checkpoint in [
    ('uniform60k', 'warm-pilot-60k', 'unqualified-checkpoint.json'),
    ('churn180k', 'warm-churn-180k', 'unqualified-checkpoint.json'),
    ('continued193k', 'warm', 'checkpoint.json'),
]:
    path = mature / folder
    meta = read(path / 'meta.json')
    assert meta['roundtripExact'] is True
    assert meta['qualified'] == (label == 'continued193k')
    out['construction'][label] = dict(meta, checkpointSha256=sha(path / checkpoint))
assert out['construction']['continued193k']['checkpointSha256'] == (
    mature / 'checkpoint-sha256.txt').read_text().strip()
for label, path in [
    ('uniform', mature / 'warm-pilot-60k-source/pins.json'),
    ('matureBase', mature / 'base/pins.json'),
    ('continuation', mature / 'generator/pins.json'),
    ('remainingCandidateProfile', remaining / 'candidate-profile/pins.json'),
]:
    pin = read(path)
    # Full original receipts stay local. Retain every executable source hash,
    # excluding unrelated tests/examples/research copied in the archive.
    pin['sources'] = {k: v for k, v in pin['sources'].items()
                      if k.startswith('src/') or k in (
                          'build.rs', 'examples/study_mature.rs',
                          'examples/study_extend.rs', 'examples/study_protocols.rs',
                          'examples/perf_baseline.rs')}
    out['pins'][label] = pin
for case, counts in out['shortBodyPhaseCounts'].items():
    assert counts['calls'][2] == 1080
    assert counts['calls'][6:10] == [1080 * (2281 if case == 'official2281' else 500)] * 4
    expected = root / f'pair-{case}-0-candidate.stderr'
    actual = remaining / f'remaining-{case}.stderr'
    assert actual.read_bytes() == expected.read_bytes()
out['hostFailures'] = []
for path, message in [
    (mature / 'presentation-failure/mature19-base.stderr',
     'Picked-up food is missing presentation provenance'),
    (mature / 'mature19-base.stderr', 'Snake food is missing birth provenance'),
]:
    assert message in path.read_text()
    out['hostFailures'].append({'error': message, 'stderrSha256': sha(path)})
log = remaining / 'provenance-guard.log'
assert 'test result: ok. 1 passed; 0 failed;' in log.read_text()
out['guardRegression'] = {
    'command': ['cargo', 'test', '--locked', '--lib', 'missing_or_unknown_source_is_rejected'],
    'passed': 1, 'failed': 0, 'logSha256': sha(log)}
print(json.dumps(out, indent=2))
