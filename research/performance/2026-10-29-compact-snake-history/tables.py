"""Recompute descriptive paired tables from the selected grouped observations."""
import json
import statistics
from pathlib import Path

root = Path(__file__).resolve().parent
data = json.loads((root / 'results.json').read_text())
for row in data['rows']:
    reported = [v for v in row['pairs'] if not v['discardedAB']]
    assert len(reported) == 5
    computed = {}
    for tick in ['all', 'ordinary', 'maintenance']:
        computed[tick] = {}
        for key in ['cpuMsPerTick', 'instructionsPerTick', 'cyclesPerTick']:
            ratios = [v['candidate'][tick][key] / v['base'][tick][key] for v in reported]
            computed[tick][key] = {
                'pairedMedianRatio': statistics.median(ratios),
                'pairedRatioRange': [min(ratios), max(ratios)],
                'baseMedian': statistics.median(v['base'][tick][key] for v in reported),
                'candidateMedian': statistics.median(v['candidate'][tick][key] for v in reported),
            }
        for key in ['L1dMPKI', 'dTLBMPKI', 'cpuP99Ms']:
            computed[tick][key] = {}
            for arm in ['base', 'candidate']:
                values = [v[arm][tick][key] for v in reported]
                computed[tick][key][arm] = {
                    'median': statistics.median(values), 'range': [min(values), max(values)],
                }
    assert computed == row['summary'], row['name']
    row['summary'] = computed

mature = data['rows'][:2]
lower = all(v['candidate']['all']['cpuMsPerTick'] < v['base']['all']['cpuMsPerTick']
            for row in mature for v in row['pairs'] if not v['discardedAB'])
maintenance = all(row['summary']['maintenance']['cpuMsPerTick']['candidateMedian'] <
                  row['summary']['maintenance']['cpuMsPerTick']['baseMedian'] for row in mature)
regressions = [row['name'] for row in data['rows'][2:]
               if all(v['candidate']['all']['cpuMsPerTick'] / v['base']['all']['cpuMsPerTick'] > 1.02
                      for v in row['pairs'] if not v['discardedAB'])]
assert data['decision'] == {
    'allTenMatureWholeWindowPairsLowerCpu': lower,
    'bothMatureMaintenanceCpuMediansLower': maintenance,
    'shortRowsWithAllFiveCpuRatiosAbove1_02': regressions,
    'runtimeQualifies': lower and maintenance and not regressions,
}
print('Frozen adoption gate:', json.dumps(data['decision'], sort_keys=True))
pilot = json.loads((root / 'pilot-results.json').read_text())
for design in pilot['pilotResults']:
    pairs = [v for v in design['pairs'] if not v['discarded']]
    ratios = [v['cpuRatio'] for v in pairs]
    print('Pilot', design['design'], 'CPU median/range', statistics.median(ratios),
          min(ratios), max(ratios), 'instruction median',
          statistics.median(v['instructionRatio'] for v in pairs))


def pair(value):
    return f"{value['baseMedian']:.4f} → {value['candidateMedian']:.4f}"


def ratio(value):
    low, high = value['pairedRatioRange']
    return f"{value['pairedMedianRatio']:.4f} [{low:.4f}, {high:.4f}]"


def misses(value):
    return f"{value['base']['median']:.3f} → {value['candidate']['median']:.3f}"


print('\n| Row / tick | CPU ms | Paired CPU ratio [range] | Instructions ratio | Cycles ratio |')
print('| --- | ---: | ---: | ---: | ---: |')
for row in data['rows']:
    for tick in ['all', 'ordinary', 'maintenance']:
        v = row['summary'][tick]
        print(f"| {row['name']} / {tick} | {pair(v['cpuMsPerTick'])} | "
              f"{ratio(v['cpuMsPerTick'])} | {ratio(v['instructionsPerTick'])} | "
              f"{ratio(v['cyclesPerTick'])} |")

print('\n| Row / tick | M instructions | M cycles | L1d misses/k instructions | dTLB misses/k instructions | CPU p99 ms |')
print('| --- | ---: | ---: | ---: | ---: | ---: |')
for row in data['rows']:
    for tick in ['ordinary', 'maintenance']:
        v = row['summary'][tick]
        instructions = {k: v['instructionsPerTick'][k] / 1e6 for k in ['baseMedian', 'candidateMedian']}
        cycles = {k: v['cyclesPerTick'][k] / 1e6 for k in ['baseMedian', 'candidateMedian']}
        print(f"| {row['name']} / {tick} | {pair(instructions)} | {pair(cycles)} | "
              f"{misses(v['L1dMPKI'])} | {misses(v['dTLBMPKI'])} | {misses(v['cpuP99Ms'])} |")

print('\n| Mature row | Base record capacity bytes | Compact record capacity bytes | Occupied |')
print('| --- | ---: | ---: | ---: |')
for row in data['rows'][:2]:
    storage = row['storage']
    print(f"| {row['name']} | {storage['base']['recordCapacityBytes']} | "
          f"{storage['candidate']['recordCapacityBytes']} | {storage['base']['occupied']} |")
