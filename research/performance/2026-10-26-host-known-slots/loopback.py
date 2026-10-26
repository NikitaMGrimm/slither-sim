"""Bounded Linux loopback host comparison, with one CPU for all host threads."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import time


def cpu(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK')


def run(binary, load, output, hold, profile, count=500, preset=None):
    output.mkdir(parents=True, exist_ok=False)
    config = {'bind': '127.0.0.1:18766', 'seed': 7,
              'players': {'max': count}, 'bots': {'count': 0, 'threads': 0}}
    if preset:
        config['simulation'] = {'world': {'preset': preset}}
    (output / 'config.json').write_text(json.dumps(config) + '\n')
    epoch = time.time() + 1
    warmup = 15
    children, logs = [], []

    def launch(command, name, env=None):
        log = (output / f'{name}.log').open('w')
        logs.append(log)
        p = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=env)
        children.append(p)
        return p

    try:
        host = launch(['taskset', '-c', '0', str(binary), str(output / 'config.json'),
                       str(warmup), str(hold), str(output / 'host.json'), str(epoch)], 'host')
        time.sleep(.2)
        offset = 0
        for shard in range(3):
            shard_count = count // 3 + (shard < count % 3)
            env = dict(os.environ, STUDY_SERVER_IP='127.0.0.1', STUDY_EPOCH_UNIX_S=str(epoch),
                       RAYON_NUM_THREADS='2')
            launch(['taskset', '-c', str(2 + 2 * shard), str(load), str(shard_count), '18766',
                    str(hold), str(warmup), str(output / f'load-{shard}.json'), str(offset)],
                   f'load-{shard}', env)
            offset += shard_count
        samples = []
        profiler = None
        if profile:
            while time.time() < epoch + warmup:
                time.sleep(.02)
            profiler = launch([profile, 'record', '-e', 'cpu-clock:u', '-F', '199',
                               '--call-graph', 'dwarf,16384', '-p', str(host.pid),
                               '-o', str(output / 'perf.data'), '--', 'sleep', str(hold)], 'perf')
        for boundary in [epoch + warmup, epoch + warmup + hold]:
            while time.time() < boundary:
                time.sleep(min(.02, max(0, boundary - time.time())))
            samples.append({'unix': time.time(), 'cpuSeconds': cpu(host.pid)})
        for p in children:
            if p.wait(timeout=30):
                raise RuntimeError(f'child failed: {p.args}; inspect retained logs')
        report = json.loads((output / 'host.json').read_text())
        bounds = [r for r in report['rows'] if r['boundary'] in ['start', 'end']]
        assert len(bounds) == 2
        ticks = bounds[1]['ticks'] - bounds[0]['ticks']
        result = {'ticks': ticks, 'cpuMsPerTick': 1000 * (samples[1]['cpuSeconds'] - samples[0]['cpuSeconds']) / ticks,
                  'deferredTicks': bounds[1]['deferred'] - bounds[0]['deferred'],
                  'samples': samples, 'tickBoundaries': bounds,
                  'binarySha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                  'hostCpu': [0], 'loadCpus': [2, 4, 6], 'warmupSeconds': warmup, 'holdSeconds': hold,
                  'requested': count, 'preset': preset or 'official',
                  'loads': [json.loads((output / f'load-{i}.json').read_text()) for i in range(3)]}
        (output / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
        if profiler:
            with (output / 'perf-period.stacks').open('w') as f:
                subprocess.run([profile, 'script', '-i', str(output / 'perf.data'),
                                '-F', 'comm,pid,tid,time,event,period,ip,sym,dso'], stdout=f, check=True)
        print(json.dumps({k: result[k] for k in ['ticks', 'cpuMsPerTick', 'deferredTicks']}), flush=True)
    finally:
        for p in children:
            if p.poll() is None:
                p.terminate()
                p.wait(timeout=10)
        for log in logs:
            log.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('load', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--hold', type=float, default=45)
    parser.add_argument('--profile', default='')
    parser.add_argument('--count', type=int, default=500)
    parser.add_argument('--preset', choices=['battledome'])
    args = parser.parse_args()
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    resource.setrlimit(resource.RLIMIT_NOFILE, (min(max(soft, 4096), hard), hard))
    run(args.binary.resolve(), args.load.resolve(), args.output.resolve(), args.hold, args.profile,
        args.count, args.preset)
