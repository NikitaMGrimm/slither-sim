"""Normalize only the two report-control tokens; never touch client inputs."""
from pathlib import Path
import json,os,time
r=Path(__file__).resolve().parent;os.sched_setaffinity(0,{7})
root=r/'load-run';path=root/'RESET'
deadline=time.monotonic()+90*60
while time.monotonic()<deadline:
 try:
  raw=path.read_bytes()
  if raw in [b'count\n',b'end\n']:
   token=raw.rstrip(b'\n');temporary=root/'RESET.normalized';temporary.write_bytes(token)
   if path.read_bytes()==raw:
    temporary.replace(path)
    with (root/'marker-normalization.jsonl').open('a') as out:out.write(json.dumps({'unix':time.time(),'marker':token.decode(),'bytesBefore':len(raw),'bytesAfter':len(token)})+'\n')
   else:temporary.unlink(missing_ok=True)
 except FileNotFoundError:pass
 try:
  if json.loads((r/'run-status.json').read_text()).get('status') in ['complete','failed']:break
 except (FileNotFoundError,json.JSONDecodeError):pass
 time.sleep(.005)
