"""Real producer-token regression, isolated from the running world and inputs."""
from pathlib import Path
import json,shutil,subprocess,sys,tempfile,time
with tempfile.TemporaryDirectory() as name:
 root=Path(name);load=root/'load-run';load.mkdir()
 shutil.copyfile(Path(__file__).with_name('marker_tokens.py'),root/'marker_tokens.py')
 (root/'run-status.json').write_text('{"status":"running"}')
 (load/'tick').write_text('123\n');(load/'target').write_text('500\n')
 proc=subprocess.Popen([sys.executable,str(root/'marker_tokens.py')])
 try:
  for value in ['count','end']:
   (load/'RESET').write_text(value+'\n');deadline=time.monotonic()+3
   while (load/'RESET').read_bytes()!=value.encode():
    assert proc.poll() is None and time.monotonic()<deadline;time.sleep(.005)
  assert (load/'tick').read_bytes()==b'123\n' and (load/'target').read_bytes()==b'500\n'
  (root/'run-status.json').write_text('{"status":"complete"}');assert proc.wait(timeout=3)==0
  rows=[json.loads(x) for x in (load/'marker-normalization.jsonl').read_text().splitlines()];assert [x['marker'] for x in rows]==['count','end']
 finally:
  if proc.poll() is None:proc.kill();proc.wait()
print('PASS: newline producer tokens normalize to exact client tokens; input files unchanged',flush=True)
