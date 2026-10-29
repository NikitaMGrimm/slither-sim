"""Recompute diagnostic scopes from the pinned generated-source IP locations."""
from pathlib import Path
import json,statistics

r = Path(__file__).resolve().parent
profiles = json.loads((r / 'attribution.json').read_text())
sites = json.loads((r / 'maintenance-sites.json').read_text())
counters = json.loads((r / 'attribution-counters.json').read_text())
def profile(protocol, event, maintenance):
    return next(v for v in profiles if (v['protocol'], v['event'], v['maintenance'])
                == (protocol, event, maintenance))
def group(row, source):
    return next(v['samples'] for v in row['sourceGroups'] if v['source'] == source)

print('| Protocol | Food overall | Sector advance | Subscription union | Held refresh | Listing build/sort | Sector diff/lookup |')
print('| --- | ---: | ---: | ---: | ---: | ---: | ---: |')
for protocol in ['stock', 'legacy']:
    row = profile(protocol, 'cycles', True)
    locations = next(v['sites'] for v in sites
                     if (v['protocol'], v['event']) == (protocol, 'cycles'))
    def food_ranges(*ranges):
        count = 0
        for v in locations:
            file, line = v['site'].rsplit(':', 1)
            if file == 'src/client/simulation/food.rs' and line.isdigit() and any(lo <= int(line) <= hi for lo, hi in ranges):
                count += v['samples']
        return count
    counts = [group(row, 'src/client/simulation/food.rs'),
              group(row, 'src/client/visibility.rs'),
              # Membership phase; the final retired-listing filter at 108
              # is outside the narrower scope used in the frozen selection.
              food_ranges((58, 107)), food_ranges((534, 574)),
              food_ranges((352, 382)), food_ranges((30, 37), (576, 615))]
    print('| ' + protocol + ' | ' + ' | '.join(f'{n / row["samples"] * 100:.2f}%' for n in counts) + ' |')
    assert row['allProfilesPrimary'] and row['lost'] == 0
    tlb = profile(protocol, 'dTLB-load-misses', True)
    print('Maintenance samples cycles/dTLB:', row['samples'], tlb['samples'],
          'sector dTLB share:', group(tlb, 'src/client/visibility.rs') / tlb['samples'])
    for maintenance in [False, True]:
        ordinary = profile(protocol, 'cycles', maintenance)
        print(protocol, 'maintenance' if maintenance else 'ordinary',
              {name:group(ordinary, name) / ordinary['samples']
               for name in ['src/client/simulation/snakes.rs', 'src/client/simulation/food.rs']})
        print('Leading project source groups:',
              [(v['source'], round(v['share'] * 100, 2))
               for v in ordinary['sourceGroups'] if v['source'].startswith('src/')][:5])
        metrics = {}
        for event in ['cycles', 'L1d-load-misses', 'dTLB-load-misses']:
            values = next(v['runs'] for v in counters
                          if (v['protocol'], v['event'], v['maintenance']) == (protocol, event, maintenance))
            assert len(values) == 3 and all(v['minimumRunningFraction'] == 1 for v in values)
            key = 'eventPerTick' if event == 'cycles' else 'eventPerKInstructions'
            metrics[event] = statistics.median(v[key] for v in values)
        print('Diagnostic counter medians:', metrics)
