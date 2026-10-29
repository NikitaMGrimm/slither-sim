"""Recompute the report's descriptive tables and frozen absolute-cost gate."""
import json
import statistics as st
from pathlib import Path

root = Path(__file__).resolve().parent
data = json.loads((root / 'results.json').read_text())
metric_names = ['cpuMsPerTick', 'instructionsPerTick', 'cyclesPerTick']
distribution_names = ['L1dMPKI', 'dTLBMPKI', 'cpuP99Ms']
reported = lambda row: [p for p in row['pairs'] if not p['discardedAB']]
for row in data['rows']:
    pairs = reported(row)
    assert len(pairs) == 5
    summary = {}
    for tick in ['all', 'ordinary', 'maintenance']:
        summary[tick] = {}
        for metric in metric_names:
            values = [p['candidate'][tick][metric] / p['base'][tick][metric]
                      for p in pairs]
            summary[tick][metric] = {
                'pairedMedianRatio': st.median(values),
                'pairedRatioRange': [min(values), max(values)],
                **{arm + 'Median': st.median(p[arm][tick][metric] for p in pairs)
                   for arm in ['base', 'candidate']},
            }
        for metric in distribution_names:
            summary[tick][metric] = {}
            for arm in ['base', 'candidate']:
                values = [p[arm][tick][metric] for p in pairs]
                summary[tick][metric][arm] = {
                    'median': st.median(values), 'range': [min(values), max(values)],
                }
    assert summary == row['summary'], row['name']

mature = [row for row in data['rows'] if row['name'].startswith('mature-')]
assert len(mature) == 2
lower = all(p['candidate']['all']['cpuTotalNs'] < p['base']['all']['cpuTotalNs']
            for row in mature for p in reported(row))
maintenance, controls = [], []
for row in data['rows']:
    tick = 'maintenance' if row in mature else 'all'
    pairs = reported(row)
    count = pairs[0]['base'][tick]['ticks']
    assert all(p[arm][tick]['ticks'] == count
               for p in pairs for arm in ['base', 'candidate'])
    costs = [p['candidate'][tick]['cpuTotalNs'] - p['base'][tick]['cpuTotalNs']
             for p in pairs]
    median = st.median(costs)
    value = {
        'row': row['name'], 'ticksPerWindow': count,
        'pairedWindowCpuDifferenceNs': costs,
        'medianPairedCostUsPerTick': median / count / 1000,
        'pairedCostRangeUsPerTick': [min(costs) / count / 1000,
                                    max(costs) / count / 1000],
        'qualifies': median <= (0 if row in mature else count * 10000),
    }
    (maintenance if row in mature else controls).append(value)
decision = {
    'allTenMatureWholeWindowPairsLowerCpu': lower,
    'matureMaintenance': maintenance, 'shortAbsoluteCostLimitUs': 10,
    'shortControls': controls,
    'runtimeQualifies': lower and all(v['qualifies'] for v in maintenance + controls),
}
assert decision == data['decision']
print('Frozen gate:', json.dumps(decision, sort_keys=True))


def ratio(value):
    low, high = value['pairedRatioRange']
    return f"{value['pairedMedianRatio']:.4f} [{low:.4f}, {high:.4f}]"


print('\n| Row / tick | CPU ms base → candidate | Paired CPU ratio [range] | Instructions ratio | Cycles ratio |')
print('| --- | ---: | ---: | ---: | ---: |')
for row in data['rows']:
    for tick in ['all', 'ordinary', 'maintenance']:
        v = row['summary'][tick]
        cpu = v['cpuMsPerTick']
        print(f"| {row['name']} / {tick} | {cpu['baseMedian']:.4f} → {cpu['candidateMedian']:.4f} | "
              f"{ratio(cpu)} | {ratio(v['instructionsPerTick'])} | {ratio(v['cyclesPerTick'])} |")

print('\n| Row / tick | M instructions | M cycles | L1d MPKI | dTLB MPKI | CPU p99 ms |')
print('| --- | ---: | ---: | ---: | ---: | ---: |')
for row in data['rows']:
    for tick in ['ordinary', 'maintenance']:
        v = row['summary'][tick]
        values = [f"{v[k]['baseMedian'] / 1e6:.4f} → {v[k]['candidateMedian'] / 1e6:.4f}"
                  for k in ['instructionsPerTick', 'cyclesPerTick']]
        values += [f"{v[k]['base']['median']:.4f} → {v[k]['candidate']['median']:.4f}"
                   for k in distribution_names]
        print(f"| {row['name']} / {tick} | " + ' | '.join(values) + ' |')

print('\n| Row | Median paired cost µs/tick | Range | Qualifies |')
print('| --- | ---: | ---: | --- |')
for v in maintenance + controls:
    low, high = v['pairedCostRangeUsPerTick']
    print(f"| {v['row']} | {v['medianPairedCostUsPerTick']:.4f} | [{low:.4f}, {high:.4f}] | {v['qualifies']} |")
