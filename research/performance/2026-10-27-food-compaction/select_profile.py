"""Select the prespecified 853 steady-tick diagnostic window."""
import argparse
import hashlib
import json
import statistics
from pathlib import Path

if not __debug__:
    raise SystemExit('Run without Python optimization')
p = argparse.ArgumentParser()
p.add_argument('root', type=Path)
a = p.parse_args()
root = a.root.resolve()
rows = json.loads((root / 'phases.json').read_text())
if len(rows) != 853 or len({r['tick'] for r in rows}) != 853:
    raise SystemExit('Unexpected steady window')
if any(r['tick'] < 120 or r['tick'] == 720 for r in rows):
    raise SystemExit('Wrong tick selection')
names = ['pre_pickup', 'eater_circles', 'grid_and_fallback_setup', 'lookup_exact_sort', 'event_mass_application', 'removal_mask', 'cache_remove', 'food_retain', 'prey', 'ambient']
sub_names = ['cache_push', 'index_push', 'cache_remove', 'slot_refresh', 'index_remove', 'has_room', 'sync', 'pellets_access', 'find_eater', 'unused9', 'hit_sort', 'unused11', 'ambient_count', 'eater_grid_build', 'empty_probe', 'unused15']
result = {'ticks': len(rows), 'meanInstructions': {}, 'nestedMeanInstructions': {}, 'nestedMeanCalls': {}}
scheduled = []
for i, name in enumerate(names):
    samples = []
    for row in rows:
        values = [v for v in row['phases'] if v[0] == i]
        if not values and i in {5, 6}:
            samples.append(0)
            continue
        if len(values) != 1:
            raise SystemExit('Unexpected phase structure')
        _, instructions, enabled, running = values[0]
        if not running or running / enabled < .99:
            raise SystemExit('Under-scheduled profile phase')
        scheduled.append(running / enabled)
        samples.append(instructions * enabled / running)
    result['meanInstructions'][name] = statistics.mean(samples)
for i, name in enumerate(sub_names):
    if name.startswith('unused'):
        continue
    result['nestedMeanInstructions'][name] = statistics.mean(r['sub'][i] for r in rows)
    result['nestedMeanCalls'][name] = statistics.mean(r['calls'][i] for r in rows)
result['pickupMean'] = sum(result['meanInstructions'][name] for name in names[1:8])
result['minScheduledPct'] = min(scheduled) * 100
result['rawSha256'] = hashlib.sha256((root / 'phases.json').read_bytes()).hexdigest()
result['binarySha256'] = hashlib.sha256((root / 'binary').read_bytes()).hexdigest()
result['sourcePins'] = json.loads((root / 'source-pins.json').read_text())
(root / 'selected.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
