"""Nearest-rank frame reduction and predeclared range-based A/B comparison."""
import argparse
from collections import defaultdict
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics


def stats(values):
    values = sorted(values)
    if not values:
        raise ValueError('No hold frames')
    return {'count': len(values), 'p50': values[math.ceil(.5 * len(values)) - 1],
            'p99': values[math.ceil(.99 * len(values)) - 1], 'max': values[-1]}


def boundary(rows, label):
    return next(row for row in rows if row.get('boundary') == label)


def reduce_trial(root):
    ready = json.loads((root / 'ready.json').read_text())
    a, b = [boundary(json.loads((root / 'host.json').read_text())['rows'], s) for s in ['start', 'end']]
    elapsed = b['unix'] - a['unix']
    frames = defaultdict(list)
    with (root / 'frames.csv').open() as handle:
        for row in csv.DictReader(handle):
            frames[row['connection']].append(float(row['receipt_ms']) - float(row['server_ms']))
    excess = []
    for values in frames.values():
        least = min(values)
        excess.extend(v - least for v in values)
    timing = stats(excess)
    telemetry = json.loads((root / 'telemetry.json').read_text())['rows']
    ca, cb = [boundary(telemetry, s) for s in ['start', 'end']]
    cpu_elapsed = cb['unix'] - ca['unix']
    thread_cpu = defaultdict(float)
    affinities = {}
    for tid, value in ca['threads'].items():
        other = cb['threads'].get(tid)
        if other is None or value['name'] != other['name']:
            raise ValueError('Host thread exited/replaced during hold')
        thread_cpu[value['name']] += (other['cpuSeconds'] - value['cpuSeconds']) / cpu_elapsed * 100
        affinities[value['name']] = value['cpus']
    for row in telemetry:
        if ca['unix'] <= row['unix'] <= cb['unix']:
            for value in row['threads'].values():
                if affinities.get(value['name']) != value['cpus']:
                    raise ValueError('Affinity changed during hold')
    expected = {'slither-simulat': [0], 'slither-sockets': [0], 'study-watcher': [1]}
    expected.update({f'slither-bots-{i}': [2 + i] for i in range(ready['config']['bots']['threads'])})
    if any(affinities.get(name) != cpus for name, cpus in expected.items()):
        raise ValueError('Missing host thread or incorrect CPU placement')
    if set(ca['threads']) != set(cb['threads']):
        raise ValueError('Thread membership changed during hold')
    if abs(elapsed - (ready['endUnix'] - ready['startUnix'])) > .1:
        raise ValueError('Host hold window differs from planned duration')
    if any(abs(row['unix'] - ready[key]) > .1 for row, key in [(ca, 'startUnix'), (cb, 'endUnix')]):
        raise ValueError('CPU boundary misses host hold by more than 100 ms')
    simulation = sum(v for n, v in thread_cpu.items() if n.startswith('slither-simulat'))
    bots = sum(v for n, v in thread_cpu.items() if n.startswith('slither-bots'))
    sockets = sum(v for n, v in thread_cpu.items() if n.startswith('slither-sockets'))
    deferred = b['deferred'] - a['deferred']
    watcher = json.loads((root / 'watcher.json').read_text())
    valid = watcher['readOrSendFailures'] == 0 and watcher['playingSeconds'] >= .95 * elapsed
    cpu_a, cpu_b = [[int(v) for v in row['systemCpu'].split()[1:]] for row in [ca, cb]]
    delta = [y - x for x, y in zip(cpu_a, cpu_b)]
    result = {
        'name': root.name, 'holdSeconds': elapsed, 'cpuWindowSeconds': cpu_elapsed,
        'config': ready['config'], 'policy': ready['policy'],
        'parameterManifestSha256': ready['parameterManifestSha256'],
        'ticks': b['ticks'] - a['ticks'], 'ticksPerSecond': (b['ticks'] - a['ticks']) / elapsed,
        'deferredTicks': deferred, 'warmupDeferredTicks': a['deferred'],
        'simulationCpuPercentOneCore': simulation, 'socketCpuPercentOneCore': sockets,
        'hostCpuPercentOneCore': simulation + sockets, 'botCpuCores': bots / 100,
        'simulationBusyWallPercent': (b['simulationSeconds'] - a['simulationSeconds']) / elapsed * 100,
        'threadCpuPercentOneCore': dict(thread_cpu), 'affinities': affinities,
        'systemStealPercent': 100 * delta[7] / sum(delta[:8]),
        'excessMs': timing, 'watcher': watcher, 'valid': valid,
        'realTime': valid and deferred == 0 and timing['p99'] <= 10,
        'officialLike': valid and deferred == 0 and timing['p99'] <= 3,
        'rawFiles': {p.name: {'sha256': hashlib.sha256(p.read_bytes()).hexdigest(), 'bytes': p.stat().st_size}
                     for p in sorted(root.iterdir()) if p.is_file()},
    }
    return result


def comparison(trials):
    a = [t for t in trials if t['name'] in ['A1', 'A2', 'A3'] and t['config']['bots']['threads'] == 2]
    b = [t for t in trials if t['name'] in ['B1', 'B2', 'B3'] and t['config']['bots']['threads'] == 2]
    if len(a) < 3 or len(b) < 3 or not all(t['valid'] for t in a + b):
        raise ValueError('Comparison needs at least three valid holds per revision')
    result = {}
    for metric in ['simulationCpuPercentOneCore', 'excessP99Ms']:
        def values(group):
            return [t['excessMs']['p99'] if metric == 'excessP99Ms' else t[metric] for t in group]
        av, bv = values(a), values(b)
        spread = max(max(av) - min(av), max(bv) - min(bv))
        difference = statistics.median(bv) - statistics.median(av)
        pairs = [y - x for x, y in zip(av, bv)]
        result[metric] = {'a': av, 'b': bv, 'aMedian': statistics.median(av),
                          'bMedian': statistics.median(bv), 'bMinusAMedian': difference,
                          'aRange': max(av) - min(av), 'bRange': max(bv) - min(bv),
                          'spread': spread, 'pairedDifferences': pairs,
                          'pairedDifferenceRange': max(pairs) - min(pairs),
                          'regressionBeyondSpread': difference > spread}
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('raw', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    trials = [reduce_trial(p) for p in sorted(args.raw.iterdir()) if (p / 'DONE').exists()]
    args.output.write_text(json.dumps({'trials': trials, 'comparison': comparison(trials)}, indent=2) + '\n')
