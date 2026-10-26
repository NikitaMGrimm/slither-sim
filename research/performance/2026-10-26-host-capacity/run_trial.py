"""Native Linux trial worker. All children are bounded; raw telemetry stays local."""
import argparse
import json
import os
from pathlib import Path
import signal
import resource
import subprocess
import time

HZ = os.sysconf('SC_CLK_TCK') if os.name == 'posix' else 100

def proc(pid, task=None):
    prefix = f'/proc/{pid}' if task is None else f'/proc/{pid}/task/{task}'
    try:
        stat = Path(prefix+'/stat').read_text().rsplit(')', 1)[1].split()
        try:
            fds = len(list(Path(prefix+'/fd').iterdir()))
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            fds = None  # Linux can revoke /proc/fd access during exit.
        return {'cpuSeconds': (int(stat[11]) + int(stat[12])) / HZ,
                'rssBytes': int(stat[21]) * os.sysconf('SC_PAGE_SIZE'),
                'fds': fds}
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        return None

def threads(pid):
    result = {}
    for tid in Path(f'/proc/{pid}/task').glob('*'):
        try:
            value = proc(pid, int(tid.name))
            if value is not None:
                result[tid.name] = {'name':(tid/'comm').read_text().strip(), 'cpus':sorted(os.sched_getaffinity(int(tid.name))), **value}
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            pass  # The thread exited between the directory listing and read.
    return result

def snapshot(pids):
    net = {}
    for line in Path('/proc/net/dev').read_text().splitlines()[2:]:
        name, values = line.split(':')
        if name.strip() == 'lo':
            continue
        v = [int(x) for x in values.split()]
        net[name.strip()] = {'rxBytes': v[0], 'txBytes': v[8], 'rxPackets': v[1], 'txPackets': v[9], 'rxDrops': v[3], 'txDrops': v[11]}
    return {'unix': time.time(), 'processes': {str(p): proc(p) for p in pids},
            'cpu': Path('/proc/stat').read_text().splitlines()[0], 'nic': net,
            'threads': {str(p): threads(p) for p in pids}}

def main(args):
    # Keep the process soft limit clear of the requested 1,000 sockets and churn.
    # This stays within the inherited hard limit and changes no host code.
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    resource.setrlimit(resource.RLIMIT_NOFILE, (min(max(soft, 4096), hard), hard))
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=False)
    children, handles, perf = [], [], None
    def launch(command, name, env=None):
        log = open(out / (name + '.log'), 'w')
        handles.append(log)
        p = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                             start_new_session=True, env=env)
        children.append(p)
        return p
    try:
        if args.role == 'host':
            config = {'bind': '0.0.0.0:8766', 'seed': 7, 'players': {'max': args.count},
                      'bots': {'count': args.bots, 'threads': 7 if args.bots else 0}}
            if args.bots:
                latency = {'roundTripSeconds': [0.04]}
                config['players']['latency'] = latency
                config['bots']['latency'] = latency
            (out / 'config.json').write_text(json.dumps(config) + '\n')
            p = launch([str(Path(args.bin).resolve()/'host_probe'), str(out/'config.json'), str(args.warmup), str(args.hold),
                        str(out/'host.json'), str(args.epoch)], 'host')
        else:
            shards = min(args.shards, args.count)
            offset = 0
            for i in range(shards):
                count = args.count // shards + (i < args.count % shards)
                env = dict(os.environ, STUDY_SERVER_IP=args.server, STUDY_EPOCH_UNIX_S=str(args.epoch))
                if args.watcher:
                    env['STUDY_WATCHER'] = '1'
                command = [str(Path(args.bin).resolve()/'host_capacity_load'), str(count), '8766', str(args.hold),
                           str(args.warmup), str(out/f'load-{i}.json'), str(offset)]
                launch(['taskset', '-c', str(i + (1 if args.watcher else 0)), *command], f'load-{i}', env)
                offset += count
        pids = [p.pid for p in children]
        (out/'pids.json').write_text(json.dumps(pids))
        rows = []
        start, end = args.epoch + args.warmup, args.epoch + args.warmup + args.hold
        boundaries = [start, end]
        next_sample = time.time()
        pinned = False
        while time.time() < end + 45:
            now = time.time()
            if args.bots and not pinned and now > args.epoch + 1:
                # Linux truncates comm to 15 bytes; pin each existing host thread explicitly.
                for tid in Path(f'/proc/{pids[0]}/task').glob('*'):
                    name = (tid/'comm').read_text().strip()
                    cpu = '0' if name.startswith('slither-simulat') else ('2-15' if name.startswith('slither-bots') else '1')
                    subprocess.run(['taskset', '-pc', cpu, tid.name], check=True, capture_output=True)
                pinned = True
            if now >= next_sample or (boundaries and now >= boundaries[0]):
                row = snapshot(pids)
                row['boundary'] = None
                if boundaries and now >= boundaries[0]:
                    row['boundary'] = 'start' if len(boundaries) == 2 else 'end'
                    boundaries.pop(0)
                rows.append(row)
                next_sample = now + 1
            if args.profile and perf is None and now >= start + 15:
                log = open(out/'perf.log', 'w'); handles.append(log)
                perf = subprocess.Popen(['sudo', args.profile, 'record', '-e', args.event, '-F', '99',
                                         '--call-graph', args.call_graph, '-p', str(pids[0]), '-o', str(out/'perf.data'),
                                         '--', 'sleep', '60'], stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
            if all(p.poll() is not None for p in children):
                break
            time.sleep(0.005 if boundaries and boundaries[0] - now < 0.1 else 0.05)
        for p in children:
            if p.poll() is None:
                raise RuntimeError('trial worker exceeded bounded exit margin')
            if p.returncode:
                raise RuntimeError(f'child exited {p.returncode}; inspect logs')
        if perf is not None and perf.wait(timeout=20):
            raise RuntimeError('perf failed; inspect perf.log')
        if perf is not None:
            with (out/'perf-period.stacks').open('w') as stacks, (out/'perf-script.log').open('w') as errors:
                subprocess.run(['sudo',args.profile,'script','-i',str(out/'perf.data'),
                                '-F','comm,pid,tid,time,event,period,ip,sym,dso'],stdout=stacks,stderr=errors,check=True,timeout=120)
            with (out/'perf-noinline-period.stacks').open('w') as stacks:
                subprocess.run(['sudo',args.profile,'script','--no-inline','-i',str(out/'perf.data'),
                                '-F','comm,pid,tid,time,event,period,ip,sym,dso'],stdout=stacks,check=True,timeout=120)
        (out/'telemetry.json').write_text(json.dumps({'epoch':args.epoch, 'warmup':args.warmup, 'hold':args.hold,
                                                    'nofile':resource.getrlimit(resource.RLIMIT_NOFILE), 'rows':rows})+'\n')
        (out/'DONE').write_text('complete\n')
    finally:
        for p in children:
            if p.poll() is None:
                os.killpg(p.pid, signal.SIGTERM)
                p.wait(timeout=10)
        if perf is not None and perf.poll() is None:
            perf.terminate(); perf.wait(timeout=10)
        for handle in handles:
            handle.close()

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('role', choices=['host', 'load'])
    p.add_argument('output')
    p.add_argument('--bin', default='./bin')
    p.add_argument('--count', type=int, required=True)
    p.add_argument('--bots', type=int, default=0)
    p.add_argument('--shards', type=int, default=8)
    p.add_argument('--warmup', type=float, default=15)
    p.add_argument('--hold', type=float, default=120)
    p.add_argument('--epoch', type=float, required=True)
    p.add_argument('--server', default='127.0.0.1')
    p.add_argument('--watcher', action='store_true')
    p.add_argument('--profile', default='')
    p.add_argument('--call-graph', default='dwarf,16384')
    p.add_argument('--event', default='cycles')
    main(p.parse_args())
