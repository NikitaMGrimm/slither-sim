"""Independent confirmation check; reuse the unchanged first-series table format."""
from pathlib import Path
from fractions import Fraction
import json,statistics

r = Path(__file__).resolve().parent
original = (r / 'tables.py').read_text()
# Only reuse metric-summary checks and rendering, never the original gate.
prefix = original[:original.index('mature = [row')]
prefix = prefix.replace("root / 'results.json'", "root / 'confirmation-results.json'")
prefix = prefix.replace('assert len(pairs) == 5', 'assert len(pairs) == 10')
scope = {'__file__': str(r / 'tables.py')}
exec(compile(prefix, 'original tables metric checks', 'exec'), scope)
data = scope['data']
assert data['series'] == 'maintainer-post-result-confirmation'
assert len(data['rows']) == 4
mature, young = [], []
for row in data['rows']:
    assert [p['pair'] for p in row['pairs']] == list(range(-1, 10))
    pairs = [p for p in row['pairs'] if not p['discardedAB']]
    assert len(pairs) == 10
    assert all(not p[arm]['background']['exclusionApplied']
               for p in row['pairs'] for arm in ['base', 'candidate'])
    if row['name'].startswith('mature-'):
        def metric(kind):
            return [Fraction(p['candidate'][kind]['cpuTotalNs'], p['base'][kind]['cpuTotalNs'])
                    for p in pairs]
        whole, upkeep = metric('all'), metric('maintenance')
        wins = sum(value < 1 for value in whole)
        mature.append({
            'row': row['name'], 'improvingWholeWindowPairs': wins, 'requiredImprovingPairs': 8,
            'medianPairedWholeCpuRatio': float(statistics.median(whole)),
            'wholeCpuPairedRatioRange': [float(min(whole)), float(max(whole))],
            'medianPairedMaintenanceCpuRatio': float(statistics.median(upkeep)),
            'maintenanceCpuPairedRatioRange': [float(min(upkeep)), float(max(upkeep))],
            'qualifies': wins >= 8 and statistics.median(whole) < 1 and statistics.median(upkeep) < 1,
        })
    else:
        costs = [p['candidate']['all']['cpuTotalNs'] - p['base']['all']['cpuTotalNs'] for p in pairs]
        ordered = sorted(costs)
        twice = ordered[4] + ordered[5]
        young.append({'row': row['name'], 'ticksPerWindow': 1080,
                      'medianPairedWindowCostNsTimesTwo': twice,
                      'medianPairedCostUsPerTick': float(Fraction(twice, 2160000)),
                      'pairedCostRangeUsPerTick': [min(costs) / 1080 / 1000, max(costs) / 1080 / 1000],
                      'qualifies': twice <= 21600000})
expected = {'reportedPairsPerRow': 10, 'originalSeriesPooled': False,
            'mature': mature, 'youngAbsoluteCostLimitUs': 10, 'young': young,
            'runtimeQualifies': all(v['qualifies'] for v in mature + young)}
assert expected == data['decision']
print('Confirmation-only frozen gate:', json.dumps(expected, sort_keys=True))
start = original.index('def ratio(value):')
end = original.index("print('\\n| Row | Median paired cost")
exec(compile(original[start:end], 'original table rendering', 'exec'), scope)
print('\n| Row | Whole-window improving pairs | Median whole CPU ratio | Median maintenance ratio |')
print('| --- | ---: | ---: | ---: |')
for value in mature:
    print(f"| {value['row']} | {value['improvingWholeWindowPairs']}/10 | {value['medianPairedWholeCpuRatio']:.6f} | {value['medianPairedMaintenanceCpuRatio']:.6f} |")
print('\n| Young row | Median paired CPU cost µs/tick | Range | Qualifies |')
print('| --- | ---: | ---: | --- |')
for value in young:
    low, high = value['pairedCostRangeUsPerTick']
    print(f"| {value['row']} | {value['medianPairedCostUsPerTick']:.6f} | [{low:.6f}, {high:.6f}] | {value['qualifies']} |")
print('\n| Row | Busy-minus-study CPU ms/window, median [range] | Visible non-study CPU ms/window, median [range] |')
print('| --- | ---: | ---: |')
for row in data['rows']:
    windows = [p[arm]['background'] for p in row['pairs'][1:] for arm in ['base', 'candidate']]
    values = []
    for key in ['guestBusyMinusStudyCpuMsRaw', 'visibleNonStudyCpuMs']:
        numbers = [w[key] for w in windows]
        values.append(f'{statistics.median(numbers):.1f} [{min(numbers):.1f}, {max(numbers):.1f}]')
    print('| ' + row['name'] + ' | ' + ' | '.join(values) + ' |')
