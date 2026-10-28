"""Select bounded, anonymous counters, source owners, hashes and paired results."""
from collections import Counter
import csv
import json
from pathlib import Path
import statistics
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2])
result = {'description': 'a 4-vCPU Zen 5 EPYC KVM guest', 'phaseScope': {
    '1': 'outer fixture call; excludes process startup/final counter write', '2': 'host advance tick', '3': 'physics/world step',
    '4': 'food-interest advance', '5': 'broadcast inclusive', '6': 'snake coverage',
    '7': 'snake encoding and recipient history', '8': 'recipient food', '9': 'recipient prey'},
    'pins': {}, 'phases': {}, 'libmCalls': {}, 'libmCalibration': {}, 'samples': {}, 'pairs': {}}


def owner(stack):
    functions = [x['function'] for x in stack]
    sources = [x['source'] for x in stack]
    text = '\n'.join(functions)
    if '[unresolved caller]' in text:
        return 'unresolved libm caller'
    if 'snakes::covered' in text:
        return 'covered segments' if 'segment_distance' in text else 'covered head'
    if 'snakes::changes' in text:
        return 'incremental anchors'
    if 'encode::appearance' in text:
        return 'appearance'
    if 'SectorInterest' in text:
        return 'food-interest heading'
    if any('broadcast.rs:' in s for s in sources) and any('Point>::distance' in f for f in functions):
        return 'prey visibility'
    if 'compiler_builtins::math::libm_math::fmod' in text:
        return 'statically linked fmod'
    if 'compact_food' in text or ('slice::rotate' in text and 'model::Food' in text):
        return 'food compaction'
    if 'physics::advance_directed' in text:
        return 'physics advance_directed'
    if 'simulator' in text and 'advance_prey' in text:
        return 'physics prey'
    return next((f for f in functions if f.startswith('slither_sim') or f.startswith('<slither_sim')),
                functions[0] if functions else '[unresolved]')


for arm in ['base', 'candidate', 'mutation', 'base-profile', 'candidate-profile']:
    path = root / arm / 'pins.json'
    if path.exists():
        pin = json.loads(path.read_text())
        result['pins'][arm] = {k: pin[k] for k in ['archiveCommit', 'archiveSha256', 'command',
                                                 'toolchain', 'profile', 'binaries', 'driverSha256', 'counterSha256']}
        result['pins'][arm]['runtimeSourceSha256'] = {k: v for k, v in pin['sources'].items()
                                                   if k in ['src/physics.rs', 'src/model.rs', 'src/simulator.rs', 'src/client/simulation/snakes.rs', 'src/client/simulation/broadcast.rs']}
        result['pins'][arm]['harnessSourceSha256'] = {k: v for k, v in pin['sources'].items()
            if k in ['src/client/simulation/study_protocols.rs', 'src/client/simulation/study_protocols/tick_counter.rs',
                     'examples/study_protocols.rs', 'examples/perf_baseline.rs', 'src/study_counter.rs']}
for path in root.glob('*-phases-counts.json'):
    result['phases'][path.name.removesuffix('-phases-counts.json')] = json.loads(path.read_text())
for path in root.glob('*-libm.csv'):
    if path.name.startswith(('baseplain-', 'candidateplain-')):
        arm = path.name.split('-')[0].removesuffix('plain')
    else:
        arm = 'candidate-profile' if path.name.startswith('candidate-') else 'base-profile'
    with path.open() as f:
        first = f.readline()
        if not first.startswith('#'):
            continue  # Earlier uncalibrated diagnostics remain raw/private.
        rows = list(csv.DictReader(f))
    if first.startswith('#'):
        result['libmCalibration'][path.name.removesuffix('-libm.csv')] = first.strip()
    addresses = [hex(int(r['offset'], 16) - 1) for r in rows]
    symbols = subprocess.check_output(['addr2line', '-a', '-f', '-C', '-i', '-e',
                                       str(root / arm / 'bin/study_protocols') if 'client-official' not in path.name
                                       else str(root / arm / 'bin/perf_baseline'), *addresses], text=True)
    stacks = []
    current = None
    lines = iter(symbols.splitlines())
    for line in lines:
        if line.startswith('0x'):
            current = []
            stacks.append(current)
        else:
            source = next(lines)
            # Retain just repository owners; no machine/build path in publication.
            if 'slither_sim' in line:
                current.append({'function': line, 'source': 'src/' + source.rsplit('/src/', 1)[-1]})
    selected = []
    for row, stack in zip(rows, stacks):
        selected.append({'phase': int(row['scope']), 'function': row['function'],
                         'calls': int(row['calls']), 'instructionsIncludingReadBoundaries': int(row['instructions']),
                         'calleeInstructions': int(row['callee_instructions']) if 'callee_instructions' in row else None,
                         'owner': owner(stack), 'stack': stack})
    result['libmCalls'][path.name.removesuffix('-libm.csv')] = selected
for path in root.glob('sample-*.selected.json'):
    data = json.loads(path.read_text())
    grouped = Counter()
    for row in data['libmCallers']:
        grouped[owner(row['stack'])] += row['instructionWeight']
    executable = Counter()
    for row in data['executable']:
        executable[owner(row['stack'])] += row['instructionWeight']
    result['samples'][path.name.removesuffix('.selected.json')] = {
        k: data[k] for k in ['case', 'period', 'lost', 'coverage', 'totalInstructionWeight', 'modules', 'limits',
                            'outerFixtureInstructions', 'windowToOuterRatio'] if k in data}
    result['samples'][path.name.removesuffix('.selected.json')]['libmOwners'] = dict(grouped.most_common())
    result['samples'][path.name.removesuffix('.selected.json')]['executableOwners'] = dict(executable.most_common(25))
if (root / 'pairs.json').exists():
    for case, rows in json.loads((root / 'pairs.json').read_text()).items():
        counted = rows[1:]
        result['pairs'][case] = {'pairs': rows,
            'baseMedian': statistics.median(r['base'] for r in counted),
            'candidateMedian': statistics.median(r['candidate'] for r in counted),
            'pairedMedian': statistics.median(r['ratio'] for r in counted),
            'pairedRange': [min(r['ratio'] for r in counted), max(r['ratio'] for r in counted)]}
        if 'tickRatio' in counted[0]:
            result['pairs'][case].update({'baseTickMedian': statistics.median(r['baseTicks'] for r in counted),
                'candidateTickMedian': statistics.median(r['candidateTicks'] for r in counted),
                'tickPairedMedian': statistics.median(r['tickRatio'] for r in counted),
                'tickPairedRange': [min(r['tickRatio'] for r in counted), max(r['tickRatio'] for r in counted)]})
for name in ['parity.json', 'mutation.json', 'checks/results.json', 'reconstruction.json']:
    if (root / name).exists():
        result[name] = json.loads((root / name).read_text())
for path in root.glob('libm-*-build.json'):
    result['libmCalibration'][path.name] = json.loads(path.read_text())
result['hostSummaries'] = {}
for case in ['official500', 'official500-half', 'official500-legacy', 'official2281']:
    base = (root / ('trace-' + case + '-base.stderr')).read_bytes()
    candidate = (root / ('trace-' + case + '-candidate.stderr')).read_bytes()
    assert base == candidate
    result['hostSummaries'][case] = json.loads(base.decode().splitlines()[-1])
result['callerCrosscheck'] = {}
for case in ['official500', 'official2281']:
    if 'base-' + case not in result['libmCalls']:
        continue
    a = result['libmCalls']['base-' + case]
    b = result['libmCalls']['baseplain-' + case]
    if any(r['calleeInstructions'] is None for r in a + b):
        continue
    checks = []
    for function in sorted({r['function'] for r in a + b}):
        x = [r for r in a if r['function'] == function]
        y = [r for r in b if r['function'] == function]
        calls_x, calls_y = sum(r['calls'] for r in x), sum(r['calls'] for r in y)
        assert calls_x == calls_y, (case, function)
        ix, iy = sum(r['calleeInstructions'] for r in x), sum(r['calleeInstructions'] for r in y)
        checks.append({'function': function, 'callsMatch': True, 'calls': calls_x,
                       'taggedInstructions': ix, 'plainInstructions': iy,
                       'relativeInstructionDifference': (ix - iy) / iy})
    result['callerCrosscheck'][case] = checks
result['limits'] = 'Phase and per-libm brackets include instrumentation overhead. Sample weights are estimates and unresolved callers remain explicit. Counts, samples and elapsed time are distinct. Short synthetic bodies are not the mature capacity worlds.'
output.write_text(json.dumps(result, indent=2) + '\n')
