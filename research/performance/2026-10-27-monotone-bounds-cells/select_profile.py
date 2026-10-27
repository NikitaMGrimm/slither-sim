"""Select the prespecified steady core-phase window."""
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
if len(rows) != 853 or len({r['tick'] for r in rows}) != 853 or any(r['tick'] < 120 or r['tick'] == 720 for r in rows):
    raise SystemExit('Wrong steady window')
names = ['pre_physics', 'setup', 'movement_boost_body', 'collision_records', 'collision_order_index', 'collision_tests', 'death_drops', 'pickup_cache', 'prey', 'ambient', 'finish']
result = {'ticks': len(rows), 'meanInstructions': {}, 'nestedMeanInstructions': {}, 'nestedMeanCalls': {}}
scheduled = []
for i, name in enumerate(names):
    samples = []
    for row in rows:
        values = [v for v in row['phases'] if v[0] == i]
        if len(values) != 1:
            raise SystemExit('Wrong phase structure')
        _, count, enabled, running = values[0]
        if not running or running / enabled < .99:
            raise SystemExit('Under-scheduled phase')
        scheduled.append(running / enabled)
        samples.append(count * enabled / running)
    result['meanInstructions'][name] = statistics.mean(samples)
for i, name in enumerate(['bounds_index_build', 'bounds_index_query']):
    result['nestedMeanInstructions'][name] = statistics.mean(r['sub'][i] for r in rows)
    result['nestedMeanCalls'][name] = statistics.mean(r['calls'][i] for r in rows)
result['physicsTotal'] = sum(result['meanInstructions'][name] for name in names[1:])
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
result.update(minScheduledPct=min(scheduled) * 100, rawSha256=sha(root / 'phases.json'), binarySha256=sha(root / 'binary'), sourcePins=json.loads((root / 'source-pins.json').read_text()))
(root / 'selected.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
