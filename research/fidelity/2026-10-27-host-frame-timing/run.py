"""Sequential local-only diagnostic, standard library; one stock headless client."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

p = argparse.ArgumentParser()
p.add_argument('binary', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--seconds', type=int, default=40)
p.add_argument('--port', type=int, default=8790)
p.add_argument('--bots', type=int, default=64)
p.add_argument('--metronome', action='store_true')
a = p.parse_args()
if (a.output.parent / 'STOP').exists():
    raise SystemExit('series stopped: candidate timeout regressed input polling')
a.output.mkdir(parents=True, exist_ok=False)
if os.name == 'nt':
    ctypes.windll.kernel32.SetProcessAffinityMask(ctypes.windll.kernel32.GetCurrentProcess(), 3)
else:
    os.sched_setaffinity(0, {0, 1})
config = {'bind': f'127.0.0.1:{a.port}', 'seed': 7,
          'players': {'max': 4}, 'bots': {'count': a.bots, 'threads': 2},
          'simulation': {'world': {'preset': 'battledome'}, 'delivery': 'responsive'}}
cfg = a.output / 'host.json'
cfg.write_text(json.dumps(config), encoding='utf-8')
env = dict(os.environ, SLITHER_TIMING=str((a.output / 'trace').resolve()), RAYON_NUM_THREADS='2')
command = [str(a.binary.resolve()), str(cfg), str(a.seconds + 8)]
if os.name != 'nt':
    command = ['/usr/bin/time', '-f', '%U %S %e', '-o', str(a.output / 'cpu.txt'), *command]
started = time.perf_counter()
with (a.output / 'host.log').open('w') as log:
    host = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        time.sleep(1)
        collect_command = ['node', 'tools/fidelity/slither-headless.mjs',
            '--out', str(a.output / 'capture'), '--server', f'127.0.0.1:{a.port}',
            '--seconds', str(a.seconds), '--bots', '1', '--nick', 'timing']
        if a.metronome:
            collect_command = ['node', 'research/fidelity/2026-10-27-host-frame-timing/metronome.mjs',
                               str(a.output / 'capture'), str(a.port), str(a.seconds)]
        collector = subprocess.run(collect_command,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=a.seconds + 15)
        (a.output / 'collector.log').write_bytes(collector.stdout)
        code = host.wait(timeout=15)
        cpu = None
        if os.name == 'nt':
            from ctypes import wintypes
            values = [wintypes.FILETIME() for _ in range(4)]
            if not ctypes.windll.kernel32.GetProcessTimes(wintypes.HANDLE(int(host._handle)),
                    *(ctypes.byref(v) for v in values)):
                raise ctypes.WinError()
            cpu = sum((v.dwHighDateTime << 32) + v.dwLowDateTime for v in values[2:]) / 1e7
        else:
            u, s, _ = (a.output / 'cpu.txt').read_text().split()
            cpu = float(u) + float(s)
        result = dict(platform=platform.platform(), binarySha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),
            hostExit=code, collectorExit=collector.returncode, cpuSeconds=cpu,
            wallSeconds=time.perf_counter()-started, seconds=a.seconds, logicalCpus=[0,1])
        (a.output / 'run.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps(result), flush=True)
        if code or collector.returncode:
            raise RuntimeError('failed run retained')
    finally:
        if host.poll() is None:
            host.terminate()
            host.wait(timeout=10)
