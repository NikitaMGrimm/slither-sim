"""Conditional integrated checks, reusing #274's driver; no timing requests."""
from pathlib import Path
import subprocess,sys

r = Path(__file__).resolve().parent
subprocess.run([sys.executable, str(r / 'retained-setup-primary.py')], check=True)
text = (r / 'prior-adoption.py').read_text()
assert text.count('/tmp/slither-compact-history-target') == 1
text = text.replace('/tmp/slither-compact-history-target', '/tmp/slither-retained-layout-target')
needle = "run('pipeline',[sys.executable,str(r/'pipeline.py')])"
assert text.count(needle) == 1
text = text.replace(needle, needle + "\nrun('stream-byte-control',[sys.executable,str(r/'compact-stream-control.py')])")
(r / 'integrated-adoption-generated.py').write_text(text)
subprocess.run([sys.executable, str(r / 'integrated-adoption-generated.py')], check=True)
