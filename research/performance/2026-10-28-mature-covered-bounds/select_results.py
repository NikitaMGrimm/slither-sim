"""Select bounded public evidence; raw data and process inventories stay local."""
import hashlib
import json
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
opt = root / 'opt'
output = Path(sys.argv[2])
read = lambda p: json.loads(p.read_text())


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def cohort(folder):
    accepted = read(folder / 'accepted.json')
    assert accepted['forkCompleteWindowBytesEqual']
    assert len(accepted['pairs']) == 6
    assert [v['pair'] for v in accepted['pairs']] == [-1, 0, 1, 2, 3, 4]
    return accepted


result = {
    'environment': 'a 4-vCPU Zen 5 EPYC KVM guest',
    'access': 'local-only raw archives, sources, binaries, window traces and logs; warm streams compared live and not stored',
    'phaseScope': {
        2: 'complete host advance', 3: 'world physics', 4: 'food-interest advance',
        5: 'broadcast inclusive', 6: 'snake coverage', 7: 'snake encoding/history',
        8: 'recipient food', 9: 'recipient prey', 10: 'whole food compaction',
        11: 'collision bounds/index construction', 12: 'collision query/exact tests',
        13: 'new body block-bounds construction'},
    'cohorts': {}, 'pins': {}, 'phases': {}, 'libm': {},
}
for label, base, name in [
    ('distance-mature-stock', root, 'saving249-stock'),
    ('distance-mature-legacy', root, 'saving249-legacy'),
    ('distance-short', root, 'matched-short249'),
    ('index-mature-stock', opt, 'index-stock'),
    ('index-mature-legacy', opt, 'index-legacy-retry'),
    ('index-short', opt, 'matched-short-index'),
]:
    result['cohorts'][label] = cohort(base / 'sessions' / name)
for label, base in [('distance', root), ('index', opt)]:
    for arm in ['base', 'candidate', 'base-profile', 'candidate-profile', 'mutation']:
        path = base / arm / 'pins.json'
        if not path.exists():
            continue
        pin = read(path)
        for name, digest in pin['binaries'].items():
            assert sha(base / arm / 'bin' / name) == digest
        pin['sources'] = {k: v for k, v in pin['sources'].items()
                          if k.startswith('src/') or k in (
                              'build.rs', 'examples/study_host.rs',
                              'examples/study_protocols.rs', 'examples/perf_baseline.rs')}
        pin['manifestSha256'] = sha(base / arm / 'source/Cargo.toml')
        pin['lockfileSha256'] = sha(base / arm / 'source/Cargo.lock')
        result['pins'][label + '/' + arm] = pin
    result[label + 'SourceReconstruction'] = read(base / 'reconstruction.json')
    assert all(v['allRustSourceHashesMatch'] for v in result[label + 'SourceReconstruction'])
for scene, ticks in [('short', 1080), ('stock', 360)]:
    for arm in ['base-profile', 'candidate-profile']:
        value = read(root / 'sessions' / ('phases-' + scene) / (arm + '-window.json'))
        assert value['counts']['calls'][2] == ticks
        result['phases'][scene + '/' + arm] = value
    result['libm'][scene] = read(root / 'sessions' / ('libm-' + scene) / 'callers-selected.json')
result['phases']['index-stock'] = read(opt / 'sessions/index-profile/profile-window.json')
result['libm']['plainMatureCrosscheck'] = read(root / 'libm-crosscheck.json')
assert all(v['callsMatch'] and abs(v['relativeInstructionDifference']) < .00002
           for v in result['libm']['plainMatureCrosscheck'])
result['libm']['calibrationBuild'] = read(root / 'libm-build.json')
result['libm']['importCoverage'] = read(root / 'libm-import-coverage.json')
result['libm']['targetOnlyLauncherProbes'] = read(root / 'preload-launcher-probes/results.json')
result['catalogParity'] = read(opt / 'parity.json')
assert len(result['catalogParity']) == 27
result['shortMutation'] = read(opt / 'mutation.json')
assert all(v['detected'] for v in result['shortMutation'])
result['matureMutation'] = {}
for case in ['mutation-stock', 'mutation-legacy-retry']:
    value = read(opt / 'sessions' / case / 'mutation.json')
    assert value['detected'] and value['physicsStateUnchanged']
    result['matureMutation'][case] = value
result['streamComparerControls'] = {}
for case in ['stream-control-0', 'stream-control-1']:
    value = read(opt / 'sessions' / case / 'mismatch.json')
    assert value['offset'] == 0
    result['streamComparerControls'][case] = value
result['requiredChecks'] = read(opt / 'required-checks-pass.json')
assert result['requiredChecks']['passed']
result['originalShortPairs'] = read(opt / 'pairs.json')
for rows in result['originalShortPairs'].values():
    assert len(rows) == 6 and [v['pair'] for v in rows] == [-1, 0, 1, 2, 3, 4]
result['windowTraceHashes'] = {}
for folder in [root / 'sessions', opt / 'sessions']:
    group = 'attribution' if folder.parent == root else 'index'
    for path in folder.glob('*/*.trace'):
        result['windowTraceHashes'][group + '/' + path.parent.name + '/' + path.name] = sha(path)
result['matureWarmCpuSeconds'] = 0.
result['warmCpuByCohort'] = {}
for folder in [root / 'sessions', opt / 'sessions']:
    group = 'attribution' if folder.parent == root else 'index'
    for path in folder.glob('*/ready.json'):
        value = read(path)
        if value['warmTicks'] >= 193000:
            seconds = sum(v['cpuSeconds'] for v in value['ready'].values())
            result['matureWarmCpuSeconds'] += seconds
            result['warmCpuByCohort'][group + '/' + path.parent.name] = seconds
result['retainedFailure'] = {
    'stage': 'initial supplemental plain-libm startup; no accepted warm/window counts',
    'error': 'base process terminated by SIGABRT before warmup census',
    'logSha256': sha(root / 'libm-plain-stock.log'),
    'legacyIndexFirstAttempt': 'full warm/window parity passed; no plain pairs before pipeline cleanup; retry retains all five pairs',
    'initialLegacyWarmCpuSeconds': sum(v['cpuSeconds'] for v in read(opt / 'sessions/index-legacy/ready.json')['ready'].values()),
    'correction': 'LD_PRELOAD applies only to the simulator, not taskset; 12 small launch/calibration probes pass before retry',
}
result['cpuAccounting'] = read(root / 'final-cpu-receipts.json')
result['cleanup'] = read(root / 'target-cleanup.json')
output.write_text(json.dumps(result, indent=2) + '\n')
print('SELECTED', output.name, output.stat().st_size, 'bytes')
