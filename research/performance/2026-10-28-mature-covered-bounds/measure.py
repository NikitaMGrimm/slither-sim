"""Run one fresh cohort; wait at the warm barrier before sequential windows.

Copy alongside serve.py in an output directory. Run no other active study
while this controller counts. Historical multi-wave attempts remain separate.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

p = argparse.ArgumentParser()
p.add_argument('session')
p.add_argument('arms', nargs='+')
p.add_argument('--diagnostic', action='store_true')
p.add_argument('--preload', action='store_true')
p.add_argument('--legacy', type=int, default=0)
p.add_argument('--warm', type=int, default=193000)
p.add_argument('--ticks', type=int, default=360)
a = p.parse_args()
root = Path(__file__).resolve().parent
session = root / 'sessions' / a.session
assert not session.exists(), 'use a fresh session; preserve existing evidence'
assert a.diagnostic or a.arms == ['base', 'candidate']
command = [sys.executable, str(root / 'serve.py'), a.session, *a.arms,
           '--legacy', str(a.legacy), '--warm', str(a.warm), '--ticks', str(a.ticks)]
if a.diagnostic:
    command.append('--no-trace')
if a.preload:
    command.append('--preload')
with (root / (a.session + '.log')).open('xb') as log:
    proc = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
    try:
        while not (session / 'ready.json').exists():
            assert proc.poll() is None, 'warmup failed; retained log has details'
            time.sleep(.2)
        if not a.diagnostic:
            subprocess.run([sys.executable, str(root / 'queue249.py'), a.session], check=True)
        else:
            for i, arm in enumerate(a.arms):
                request = {'arm': arm, 'label': arm + '-window', 'mode': 'parent', 'trace': True}
                (session / 'requests' / f'{i:03}.request').write_text(json.dumps(request))
        assert proc.wait() == 0, 'window failed; retained log has details'
    except BaseException:
        # A failed server owns and cleans its host process groups. On controller
        # interruption, stop only groups whose current executable is a study arm.
        import os
        import signal
        path = session / 'pids.json'
        if path.exists():
            for arm, pid in json.loads(path.read_text()).items():
                try:
                    if (Path('/proc') / str(pid) / 'exe').resolve() == root / arm / 'bin/study_host':
                        os.killpg(pid, signal.SIGKILL)
                except (FileNotFoundError, ProcessLookupError):
                    pass
        if proc.poll() is None:
            proc.terminate()
            proc.wait()
        raise
if not a.diagnostic:
    subprocess.run([sys.executable, str(root / 'accept249.py'), str(session)], check=True)
print('COMPLETE', a.session)
