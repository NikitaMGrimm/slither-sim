"""Alternating own-child loopback holds. Kernel units are CPU seconds only."""
import hashlib, json, os, resource, signal, statistics, subprocess, sys, time
from pathlib import Path
from thread_counter import Counter

if not __debug__:
    raise SystemExit("run without -O: the checks are asserts")

root = Path(sys.argv[1]).resolve()
out = root / 'pairs'
out.mkdir(exist_ok=True)
os.sched_setaffinity(0, {2})
soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
resource.setrlimit(resource.RLIMIT_NOFILE, (min(max(soft, 4096), hard), hard))
clock = os.sysconf('SC_CLK_TCK')
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
pins = {a: json.loads((root / a / 'pins.json').read_text()) for a in ['base', 'candidate']}
for a, pin in pins.items():
    assert all(sha(root / a / 'bin' / n) == v for n, v in pin['binaries'].items())
assert Path('/proc/sys/kernel/perf_event_paranoid').read_text().strip() == '2'

def cpu(pid, tid=None):
    path = Path(f'/proc/{pid}/task/{tid}/stat' if tid else f'/proc/{pid}/stat')
    fields = path.read_text().rsplit(')', 1)[1].split()
    return [int(fields[11]) / clock, int(fields[12]) / clock]

def run(mode, arm, pair):
    d = out / f'{mode}-{pair}-{arm}'
    d.mkdir()
    bins = root / arm / 'bin'
    warmup, hold, port = 10, 12, 18777
    (d / 'config.json').write_text(json.dumps({'bind': f'127.0.0.1:{port}', 'seed': 7, 'players': {'max': 500}, 'bots': {'count': 0, 'threads': 0}}) + '\n')
    epoch = time.time() + 2
    env = dict(os.environ, RAYON_NUM_THREADS='1', STUDY_LEGACY=mode, STUDY_SOCKET_COUNTS=str(d / 'counts.json'), STUDY_SERVER_IP='127.0.0.1', STUDY_EPOCH_UNIX_S=str(epoch))
    children, logs, counters = [], [], {}
    def launch(cmd, name):
        log = (d / (name + '.log')).open('w')
        logs.append(log)
        p = subprocess.Popen(cmd, env=env, stdout=log, stderr=subprocess.STDOUT)
        children.append(p)
        return p
    try:
        host = launch(['taskset', '-c', '0', str(bins / 'socket_probe'), str(d / 'config.json'), str(warmup), str(hold), str(d / 'host.json'), str(epoch)], 'host')
        time.sleep(.3)
        assert host.poll() is None
        load = launch(['taskset', '-c', '2', str(bins / 'host_capacity_load'), '500', str(port), str(hold), str(warmup), str(d / 'load.json'), '0'], 'load')
        names = {int(p.name): (p / 'comm').read_text().strip() for p in Path(f'/proc/{host.pid}/task').iterdir()}
        roles = {'main': host.pid}
        for role, prefix in [('simulation', 'slither-simula'), ('socket', 'slither-socket')]:
            ids = [tid for tid, name in names.items() if name.startswith(prefix)]
            assert len(ids) == 1, names
            roles[role] = ids[0]
        counters = {r: Counter(t) for r, t in roles.items()}
        while time.time() < epoch + warmup:
            time.sleep(.005)
        before_cpu = {r: cpu(host.pid, t) for r, t in roles.items()}
        before_cpu['process'] = cpu(host.pid)
        before = {r: c.read() for r, c in counters.items()}
        while time.time() < epoch + warmup + hold:
            time.sleep(.005)
        after = {r: c.read() for r, c in counters.items()}
        after_cpu = {r: cpu(host.pid, t) for r, t in roles.items()}
        after_cpu['process'] = cpu(host.pid)
        for child in children:
            assert child.wait(timeout=60) == 0, child.args
    finally:
        for p in children:
            if p.poll() is None:
                p.terminate()
                p.wait(timeout=10)
        for f in logs:
            f.close()
        for c in counters.values():
            c.close()
    h = json.loads((d / 'host.json').read_text())
    boundaries = [r for r in h['rows'] if r['boundary'] in ['start', 'end']]
    assert len(boundaries) == 2
    ticks = boundaries[1]['ticks'] - boundaries[0]['ticks']
    assert ticks > 0
    load = json.loads((d / 'load.json').read_text())
    assert not load['errors'], load['errors']
    events = json.loads((d / 'counts.json').read_text())
    events = dict(zip(events['names'], events['values']))
    instructions = {}
    for r in counters:
        delta = [b - a for a, b in zip(before[r], after[r])]
        assert delta[1] > 0 and delta[2] / delta[1] >= .99
        instructions[r] = delta[0] / ticks
    instructions['process'] = sum(instructions.values())
    result = {'arm': arm, 'mode': mode, 'pair': pair, 'ticks': ticks, 'holdSeconds': hold,
              'instructionsPerTick': instructions,
              'cpuSeconds': {r: {'user': after_cpu[r][0] - c[0], 'kernel': after_cpu[r][1] - c[1]} for r, c in before_cpu.items()},
              'eventsPerSecond': {k: v / hold for k, v in events.items()},
              'clients': load['clients'], 'receivedFramesPerSecond': load['traffic']['frames'] / hold,
              'generatorBusyPercent': 100 * load['generator']['busySecondsInWindow'] / hold,
              'deferredTicks': boundaries[1]['deferred'] - boundaries[0]['deferred']}
    (d / 'selected.json').write_text(json.dumps(result, indent=2) + '\n')
    print('HOLD', mode, pair, arm, json.dumps(result), flush=True)
    return result

for mode in ['none', 'half', 'all']:
    for pair in range(-1, 5):
        for arm in (['base', 'candidate'] if pair < 0 or pair % 2 == 0 else ['candidate', 'base']):
            path = out / f'{mode}-{pair}-{arm}' / 'selected.json'
            if path.exists():
                print('RETAIN', mode, pair, arm, flush=True)
            else:
                run(mode, arm, pair)
assert Path('/proc/sys/kernel/perf_event_paranoid').read_text().strip() == '2'
print('PASS all alternating loopback pairs', flush=True)
