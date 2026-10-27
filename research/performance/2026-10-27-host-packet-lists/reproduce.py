"""Reuse the frozen fifteen-trace harness; count the three stock/13 mixes."""
import hashlib, json, subprocess, sys
from pathlib import Path

if not __debug__:
    raise SystemExit('run without -O: evidence checks require asserts')
mode, archives, output = sys.argv[1:4]
wrapper = Path(__file__).resolve().parent.parent / '2026-10-27-host-interest-refresh/reproduce.py'
if mode != 'count':
    subprocess.run([sys.executable, str(wrapper), *sys.argv[1:]], check=True)
else:
    root = Path(output).resolve()
    pins = json.loads((root / 'inputs.json').read_text())
    for arm in ['base', 'candidate']:
        assert hashlib.sha256((Path(archives) / (arm + '.tar')).read_bytes()).hexdigest() == pins[arm]['archiveSha256']
    for name, digest in pins['harnessSha256'].items():
        assert hashlib.sha256((root / 'harness' / name).read_bytes()).hexdigest() == digest
    driver = root / 'harness/reproduce.py'
    text = driver.read_text()
    anchor = "def run(cmd,log,cwd=root,extra=None):"
    assert text.count(anchor) == 1
    text = text.replace(anchor, "cases=cases[:3]\n\n" + anchor)
    sys.argv = [str(driver), mode, '.', str(root / 'run'), '--base', pins['base']['commit'],
                '--candidate', pins['candidate']['commit'], *sys.argv[4:]]
    exec(compile(text, str(driver), 'exec'), {'__file__': str(driver), '__name__': '__main__'})
