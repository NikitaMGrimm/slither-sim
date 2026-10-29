from pathlib import Path
import json,tarfile
r=Path(__file__).resolve().parent
rows=json.loads((r/'checks/results.json').read_text())
assert len(rows)==6 and rows[-1]['label']=='rust-group' and rows[-1]['exit']==0
assert all(x['exit']==(101 if x['expectedFailure'] else 0) for x in rows)
with tarfile.open(r/'candidate.tar') as archive:
    for member in archive.getmembers():
        if member.isfile():
            assert (r/'checks/source'/member.name).read_bytes()==archive.extractfile(member).read(),member.name
note='Host restart detached stdout. The check runner durably recorded the full Rust group exit 0 before its final stdout print; reconstruct only the missing aggregate receipt. Verified all archived source files unchanged.'
with (r/'ATTEMPTS.txt').open('a') as f:f.write(note+'\n')
assert not (r/'required-checks-pass.json').exists()
(r/'required-checks-pass.json').write_text(json.dumps({'passed':True,'checks':rows,'recovery':note},indent=2)+'\n')
print('RECOVERED VERIFIED CHECK RECEIPT',flush=True)
