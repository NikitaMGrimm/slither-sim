"""Bind completed checks, native parity and immutable binaries before counting.

Usage: python write_gates.py OUTPUT CORRECTNESS_JSON VERIFICATION_JSON
"""
from pathlib import Path
import hashlib,json,sys
root=Path(sys.argv[1]);correct=json.loads(Path(sys.argv[2]).read_text());verified=json.loads(Path(sys.argv[3]).read_text())
assert len(verified['fullRustCommands'])==15 and all(r['exit']==0 for r in verified['fullRustCommands'])
for name,count in [('catalog-parity.json',23),('host-parity.json',6),('fixed-parity.json',2)]:
    rows=json.loads((root/name).read_text());assert len(rows)==count
    assert all(r['base']==r['candidate'] for r in rows)
    if name=='fixed-parity.json':assert all(r['wireMutationDetected'] for r in rows)
for arm in ['base','candidate','mutation']:
    pins=json.loads((root/arm/'pins.json').read_text());assert pins['releaseBuild']
    artifact=correct['artifacts'].get(arm,correct['artifacts'].get('traces/'+arm))
    assert artifact and pins['archiveSha256']==artifact['archiveSha256']
    for name,digest in pins['binaries'].items():
        with (root/arm/'bin'/name).open('rb') as stream:
            assert hashlib.file_digest(stream,'sha256').hexdigest()==digest
(root/'required-checks-pass.json').write_text(json.dumps({'passed':True,'checks':verified['fullRustCommands']},indent=2)+'\n')
(root/'parity-pass.json').write_text(json.dumps({'passed':True,'catalog':23,'host':6,'fixed':2,'wireMutations':2},indent=2)+'\n')
print('Native parity, archived runtime pins, copied binaries and check receipts verified')
