"""Select the prespecified steady window and slot-refresh metrics."""
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
names = ['cache_remove', 'position_refresh', 'physics', 'index_push', 'lookup_tests_sort', 'pellet_access']
result = {'ticks': len(rows), 'meanInstructions': {}, 'meanCalls': {}}
for i, name in enumerate(names):
    result['meanInstructions'][name] = statistics.mean(r['sub'][i] for r in rows)
    result['meanCalls'][name] = statistics.mean(r['calls'][i] for r in rows)
result['meanMetrics'] = {name: statistics.mean(r['metrics'][i] for r in rows) for i, name in enumerate(['oldFoodOnRemovalTicks', 'unchangedPrefix', 'newFoodOnRemovalTicks'])}
result['weightedUnchangedFraction'] = sum(r['metrics'][1] for r in rows) / sum(r['metrics'][2] for r in rows)
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
result.update(rawSha256=sha(root / 'phases.json'), binarySha256=sha(root / 'binary'), sourcePins=json.loads((root / 'source-pins.json').read_text()))
(root / 'selected.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
