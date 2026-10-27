"""Five alternating loopback repetitions, new output and distinct ports."""
import argparse
from pathlib import Path
import subprocess
import sys

p = argparse.ArgumentParser()
p.add_argument('baseline', type=Path)
p.add_argument('candidate', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--port', type=int, required=True)
p.add_argument('--seconds', type=int, default=40)
p.add_argument('--start', type=int, default=1)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=a.start != 1)
for i in range(a.start-1,5):
    for side, binary in [('baseline', a.baseline), ('candidate', a.candidate)]:
        out = a.output / f'{i+1}-{side}'
        print(f'START {out}', flush=True)
        subprocess.run([sys.executable, str(Path(__file__).with_name('run.py')),
            str(binary), str(out), '--port', str(a.port), '--seconds', str(a.seconds),
            '--metronome'], check=True)
        a.port += 1
