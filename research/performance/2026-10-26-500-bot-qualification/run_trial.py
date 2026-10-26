"""Bounded Linux runner; pin host work to one CPU and each bot worker separately."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time


def snapshot(pid):
    result = {}
    hz = os.sysconf('SC_CLK_TCK')
    for task in Path(f'/proc/{pid}/task').glob('*'):
        try:
            stat = (task / 'stat').read_text().rsplit(')', 1)[1].split()
            result[task.name] = {
                'name': (task / 'comm').read_text().strip(),
                'cpuSeconds': (int(stat[11]) + int(stat[12])) / hz,
                'cpus': sorted(os.sched_getaffinity(int(task.name))),
            }
        except (FileNotFoundError, ProcessLookupError):
            pass
    return {'unix': time.time(), 'threads': result,
            'systemCpu': Path('/proc/stat').read_text().splitlines()[0]}


def run(args):
    os.sched_setaffinity(0, {6})
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, RAYON_NUM_THREADS='1')
    with (out / 'run.log').open('w') as log:
        child = subprocess.Popen(['taskset', '-c', '1', str(args.binary.resolve()), str(out),
                                  str(args.threads), str(args.warmup), str(args.hold)],
                                 env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        rows = []
        deadline = time.monotonic() + args.warmup + args.hold + 90
        try:
            ready = out / 'ready.json'
            pinned = set()
            while not ready.exists():
                if child.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('Host failed before ready; inspect run.log')
                time.sleep(.01)
            # ready.json is small, but a reader can catch its write in progress.
            for attempt in range(100):
                try:
                    timing = json.loads(ready.read_text())
                    break
                except json.JSONDecodeError:
                    time.sleep(.01)
            else:
                raise RuntimeError('Invalid ready.json')
            boundaries = [(timing['startUnix'], 'start'), (timing['endUnix'], 'end')]
            next_sample = time.time()
            while child.poll() is None:
                now = time.time()
                if time.monotonic() > deadline:
                    raise RuntimeError('Trial exceeded bounded deadline')
                # Discover late-created workers too; all pinning must finish before hold.
                for tid, value in snapshot(child.pid)['threads'].items():
                    if tid in pinned:
                        continue
                    name = value['name']
                    if name.startswith('slither-bots-'):
                        cpu = 2 + int(name.removeprefix('slither-bots-'))
                    elif name.startswith('slither-'):
                        cpu = 0
                    else:
                        cpu = 1
                    os.sched_setaffinity(int(tid), {cpu})
                    pinned.add(tid)
                    if now >= timing['startUnix']:
                        raise RuntimeError('New thread affinity set during hold')
                if now >= next_sample or (boundaries and now >= boundaries[0][0]):
                    row = snapshot(child.pid)
                    row['boundary'] = None
                    if boundaries and now >= boundaries[0][0]:
                        row['boundary'] = boundaries.pop(0)[1]
                    rows.append(row)
                    next_sample = now + 1
                time.sleep(.002 if boundaries and boundaries[0][0] - now < .1 else .02)
            if child.returncode:
                raise RuntimeError(f'Host exited {child.returncode}; inspect run.log')
            if boundaries:
                raise RuntimeError('Missing hold CPU boundary')
        finally:
            (out / 'telemetry.json').write_text(json.dumps({'rows': rows}) + '\n')
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                child.wait(timeout=10)
    (out / 'DONE').write_text('complete\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--threads', type=int, default=2)
    parser.add_argument('--warmup', type=float, default=30)
    parser.add_argument('--hold', type=float, default=300)
    run(parser.parse_args())
