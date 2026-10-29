"""Reuse original grouped metrics; keep the new series and gate separate."""
from pathlib import Path
import subprocess,sys

r = Path(__file__).resolve().parent
# Reuse only the original adapter's metric/storage transformations. Its gate
# and executable footer are deliberately not evaluated for the confirmation.
adapter = (r / 'retained-select-primary.py').read_text()
assert adapter.count('start=text.index(') == 1
prefix = adapter.split('start=text.index(', 1)[0]
scope = {'__file__': str(r / 'retained-select-primary.py')}
exec(compile(prefix, 'retained-select-primary.py prefix', 'exec'), scope)
text = scope['text']
assert text.count('for pair in range(-1,5):') == 1
text = text.replace('for pair in range(-1,5):', 'for pair in range(-1,10):')
needle = "   summaries[arm]=data['summary']"
assert text.count(needle) == 1
text = text.replace(needle, "   arms[arm]['background']=json.loads((directory/f'pair-{pair}-{arm}-background.json').read_text())\n" + needle)
needle = " assert accepted['allSixCompleteContinuationStreamsEqual']"
assert text.count(needle) == 1
text = text.replace(needle, " assert accepted['timedPairs']==10 and accepted['discardedAB']==1\n" + needle)
text = text[:text.index("mature=[x for x in results")]
text += '''
from confirmation_gate import evaluate
out={'series':'maintainer-post-result-confirmation',
 'planCommit':'a6dfc764d509e53f5811a21a39d068708cd2244b',
 'baseline':json.loads((r/'base/pins.json').read_text())['archiveCommit'],
 'candidate':json.loads((r/'candidate/pins.json').read_text())['archiveCommit'],
 'decision':evaluate(results),'rows':results}
(r/'confirmation-results.json').write_text(json.dumps(out,indent=2)+'\\n')
print(json.dumps(out['decision']),flush=True)
for row in results:
 print(row['name'],json.dumps({kind:row['summary'][kind]['cpuMsPerTick'] for kind in ['all','ordinary','maintenance']}),flush=True)
'''
(r / 'select-confirmation-generated.py').write_text(text)
if '--prepare-only' not in sys.argv:
    subprocess.run([sys.executable, str(r / 'select-confirmation-generated.py')], check=True)
