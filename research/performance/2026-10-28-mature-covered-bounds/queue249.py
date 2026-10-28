"""Queue the frozen #249 mature comparison; no sample replacement."""
import json,sys
from pathlib import Path
root=Path(__file__).resolve().parent;session=root/'sessions'/sys.argv[1];requests=session/'requests'
assert (session/'ready.json').exists() and requests.is_dir(), 'wait for serve.py READY before queueing'
seq=0
for arm in ['base','candidate']:
 for repetition in range(2):
  row={'arm':arm,'label':f'proof-{arm}-{repetition}','trace':True}
  (requests/f'{seq:03}.request').write_text(json.dumps(row));seq+=1
for pair in range(-1,5):
 for arm in (['base','candidate'] if pair<0 or pair%2==0 else ['candidate','base']):
  row={'arm':arm,'label':f'pair-{pair}-{arm}'}
  (requests/f'{seq:03}.request').write_text(json.dumps(row));seq+=1
for arm in ['base','candidate']:
 row={'arm':arm,'label':f'proof-{arm}-parent','mode':'parent','trace':True}
 (requests/f'{seq:03}.request').write_text(json.dumps(row));seq+=1
print('queued',seq)
