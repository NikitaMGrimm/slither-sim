"""Select actual four-cohort confirmation receipts without touching series one."""
from pathlib import Path
import json

r = Path(__file__).resolve().parent
text = (r / 'select-proofs.py').read_text()
for before, after in [
    ("'selected-results.json'", "'confirmation-results.json'"),
    ("len(results['rows']) == 10", "len(results['rows']) == 4"),
    ('allTenCohortsHaveEqualCompleteWarmAndSixContinuationStreams',
     'allFourCohortsHaveEqualCompleteWarmAndSixContinuationStreams'),
    ('results.json rows[].proofs', 'confirmation-results.json rows[].proofs'),
    ("'proofs.json'", "'confirmation-proofs.json'"),
    ('ten-cohort stream receipts selected.', 'four-cohort confirmation receipts selected.'),
]:
    assert text.count(before) == 1, before
    text = text.replace(before, after)
exec(compile(text, 'original proof receipt selector, four-cohort adaptation', 'exec'))
log = (r / 'confirmation-controls.log').read_text()
assert 'Ran 9 tests' in log and log.rstrip().endswith('OK')
receipt = json.loads((r / 'confirmation-proofs.json').read_text())
receipt['methodControls'] = {'command': ['python3', 'confirmation-controls.py'],
                             'exit': 0, 'tests': 9, 'synthetic': True}
(r / 'confirmation-proofs.json').write_text(json.dumps(receipt, indent=2) + '\n')
