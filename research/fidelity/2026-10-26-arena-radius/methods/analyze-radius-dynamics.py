"""Evaluate simple radius controllers on separate official-server captures.

Input is the JSON produced by analyze-headless-ecology.py. Run3 is used only
for fitting and run4 only for evaluation. No simulated radius is substituted
for an observed radius or a stale population report.
"""

import argparse
import json
import math
from collections import Counter, defaultdict


def traces(report):
    groups = defaultdict(list)
    for file in report['files']:
        for row in file['radius']:
            groups[(file['sha256'], row['conn'])].append(row)
    return [sorted(rows, key=lambda r: r['S']) for rows in groups.values()]


def windows(report, seconds=5, max_population_age_ms=3500):
    """Non-overlapping windows, restricted to uninterrupted connections."""
    out = []
    step = seconds * 1000
    for rows in traces(report):
        if len(rows) < 3:
            continue
        averages = {}
        for tau in (5, 15, 30, 60, 120, 300):
            ema = None
            previous_s = None
            for row in rows:
                if row['players'] is not None:
                    if ema is None:
                        ema = row['players']
                    else:
                        dt = max(0, row['S'] - previous_s) / 1000
                        ema += (1 - math.exp(-dt / tau)) * (row['players'] - ema)
                    previous_s = row['S']
                averages[(tau, id(row))] = ema
        start = rows[0]['S']
        buckets = {}
        for row in rows:
            bucket = (row['S'] - start) // step
            buckets.setdefault(bucket, row)
        for bucket, row in buckets.items():
            next_row = buckets.get(bucket + 1)
            prev_row = buckets.get(bucket - 1)
            if not next_row or not prev_row:
                continue
            # Avoid interpreting a reconnect, long packet gap, or stale count.
            if not 3500 <= next_row['S'] - row['S'] <= 6500:
                continue
            if not 3500 <= row['S'] - prev_row['S'] <= 6500:
                continue
            age = row['population_age_ms']
            if row['players'] is None or age is None or age > max_population_age_ms:
                continue
            delta = next_row['radius'] - row['radius']
            if not delta:
                continue
            out.append({'radius': row['radius'], 'players': row['players'],
                        'wall_ms': row['wall_ms'], 'direction': 1 if delta > 0 else -1,
                        'previous_direction': 1 if row['radius'] > prev_row['radius'] else -1,
                        'ema': {tau: averages[tau, id(row)] for tau in (5, 15, 30, 60, 120, 300)},
                        'conn': rows[0]['conn'], 'S': row['S']})
    return out


def score(rows, predict):
    matrix = Counter((r['direction'], predict(r)) for r in rows)
    positive = sum(v for (actual, _), v in matrix.items() if actual == 1)
    negative = len(rows) - positive
    accuracy = sum(v for (actual, guess), v in matrix.items() if actual == guess) / len(rows)
    balanced = ((matrix[1, 1] / positive + matrix[-1, -1] / negative) / 2
                if positive and negative else None)
    return {'n': len(rows), 'expanding': positive, 'shrinking': negative,
            'accuracy': round(accuracy, 4),
            'balanced_accuracy': round(balanced, 4) if balanced is not None else None}


def choose(rows, candidates):
    # Optimize balanced accuracy to avoid rewarding the majority class alone.
    return max(candidates, key=lambda c: (score(rows, c[1])['balanced_accuracy'] or 0,
                                           score(rows, c[1])['accuracy']))


def models(train, test):
    majority = 1 if sum(r['direction'] for r in train) >= 0 else -1
    result = {'majority': {'direction': majority, 'train': score(train, lambda r: majority),
                           'test': score(test, lambda r: majority)},
              'last_direction': {'train': score(train, lambda r: r['previous_direction']),
                                 'test': score(test, lambda r: r['previous_direction'])}}
    # Memoryless radius target, calibrated independently of the simulator's 670.
    candidates = [(k, lambda r, k=k: 1 if k * math.sqrt(r['players']) > r['radius'] else -1)
                  for k in range(450, 851, 10)]
    k, predictor = choose(train, candidates)
    result['instant_population'] = {'coefficient': k, 'train': score(train, predictor),
                                    'test': score(test, predictor)}
    candidates = [( (tau, k), lambda r, t=tau, k=k:
                    1 if k * math.sqrt(r['ema'][t]) > r['radius'] else -1)
                  for tau in (5, 15, 30, 60, 120, 300) for k in range(450, 851, 10)]
    (tau, k), predictor = choose(train, candidates)
    result['ema_population'] = {'tau_seconds': tau, 'coefficient': k,
                                'train': score(train, predictor), 'test': score(test, predictor)}
    # Fixed-period/phase model uses absolute wall time, so validation cannot
    # reset phase at the start of a new capture.
    candidates = []
    for period in (30, 60, 120, 180, 300, 600):
        for phase in range(24):
            candidates.append(((period, phase), lambda r, p=period, q=phase:
                               1 if math.sin(2 * math.pi * (r['wall_ms'] / 1000 / p + q / 24)) > 0 else -1))
    (period, phase), predictor = choose(train, candidates)
    result['fixed_cycle'] = {'period_seconds': period, 'phase_24ths': phase,
                             'train': score(train, predictor), 'test': score(test, predictor)}
    return result


def update_evidence(report):
    diffs = Counter()
    fast = []
    states = defaultdict(lambda: defaultdict(list))
    for rows in traces(report):
        previous = None
        for row in rows:
            if previous and 100 <= row['S'] - previous['S'] <= 300:
                delta = row['radius'] - previous['radius']
                diffs[delta] += 1
                age = previous['population_age_ms']
                if abs(delta) >= 4:
                    fast.append({'conn': row['conn'], 'S': previous['S'],
                                 'radius': previous['radius'], 'players': previous['players'],
                                 'population_age_ms': age, 'delta': delta,
                                 'interval_ms': row['S'] - previous['S']})
                if previous['players'] is not None and age is not None and age <= 1000 and delta:
                    states[(previous['players'], previous['radius'])][1 if delta > 0 else -1].append(
                        {'conn': row['conn'], 'S': previous['S'], 'delta': delta,
                         'population_age_ms': age})
            previous = row
    opposite = []
    for (players, radius), directions in states.items():
        if 1 in directions and -1 in directions:
            opposite.append({'players': players, 'radius': radius,
                             'expanding': directions[1][0], 'shrinking': directions[-1][0]})
    return {'intervals_100_300ms': sum(diffs.values()),
            'step_counts': dict(sorted(diffs.items())),
            'fastest_expansion': max(fast, key=lambda r: r['delta'] * 1000 / r['interval_ms']) if fast else None,
            'exact_opposite_states': opposite, 'exact_opposite_state_count': len(opposite)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('train', help='earlier, calibration ecology JSON')
    parser.add_argument('test', help='later, validation ecology JSON')
    parser.add_argument('--output')
    args = parser.parse_args()
    with open(args.train, encoding='utf-8') as f:
        train_report = json.load(f)
    with open(args.test, encoding='utf-8') as f:
        test_report = json.load(f)
    a, b = windows(train_report), windows(test_report)
    if not a or not b:
        parser.error('both reports need nonzero eligible radius windows')
    result = {'sources': {'train': [{'path': f['path'], 'sha256': f['sha256']} for f in train_report['files']],
                          'test': [{'path': f['path'], 'sha256': f['sha256']} for f in test_report['files']]},
              'window_seconds': 5, 'models': models(a, b),
              'updates': {'train': update_evidence(train_report), 'test': update_evidence(test_report)}}
    encoded = json.dumps(result, indent=2)
    if args.output:
        with open(args.output, 'w', encoding='utf-8') as f:
            f.write(encoded + '\n')
    else:
        print(encoded)


if __name__ == '__main__':
    main()
