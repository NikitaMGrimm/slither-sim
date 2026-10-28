"""Ephemeral NIC controls and pre-warmup verification; raw identities stay private."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time


def parse_coalesce(text):
    adaptive = re.search(r'Adaptive RX:\s*(on|off|n/a)\s+TX:', text)
    values = {'adaptiveRx': adaptive[1] if adaptive else 'n/a'}
    for key, label in [('rxUsecs', 'rx-usecs'), ('txUsecs', 'tx-usecs')]:
        match = re.search(r'(?m)^'+label+r':\s*(\d+)\s*$', text)
        if not match:
            raise ValueError('NIC moderation field unavailable: '+label)
        values[key] = int(match[1])
    return values


def nic():
    names = [p.name for p in Path('/sys/class/net').iterdir() if p.name != 'lo']
    if len(names) != 1:
        raise RuntimeError('Expected one network interface')
    return names[0]


def readback():
    name = nic()
    result = subprocess.run(['sudo', 'ethtool', '-c', name], capture_output=True, text=True, check=True)
    return {'unix': time.time(), 'device': name, 'text': result.stdout, 'values': parse_coalesce(result.stdout)}


def command(args):
    result = subprocess.run(['sudo', 'ethtool', '-C', nic(), *args], capture_output=True, text=True)
    return {'arguments': args, 'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}


def restore(values):
    args = ['rx-usecs', str(values['rxUsecs']), 'tx-usecs', str(values['txUsecs'])]
    if values['adaptiveRx'] in ['on', 'off']:
        args += ['adaptive-rx', values['adaptiveRx']]
    result = command(args)
    if result['exit'] or readback()['values'] != values:
        raise RuntimeError('Default NIC restoration failed')
    return result


def configure(arm, baseline):
    baseline = baseline['values']
    result = {'arm': arm, 'before': readback(), 'resetCommand': restore(baseline)}
    expected = dict(baseline)
    if arm in ['C', 'D']:
        expected['rxUsecs'] = 64
        result['armCommand'] = command(['rx-usecs', '64'])
    elif arm == 'E':
        expected['adaptiveRx'] = 'on'
        result['armCommand'] = command(['adaptive-rx', 'on'])
    elif arm not in ['A', 'B']:
        raise ValueError('Unknown arm')
    result['applied'] = readback()
    result['expected'] = expected
    result['socketNice'] = 5 if arm in ['B', 'D'] else 0
    if result.get('armCommand', {}).get('exit', 0) or result['applied']['values'] != expected:
        raise RuntimeError('Requested NIC settings not verified')
    return result


def network_irq(text=None):
    text = Path('/proc/interrupts').read_text() if text is None else text
    total, queues = 0, 0
    for line in text.splitlines():
        if 'Tx-Rx' not in line:
            continue
        queues += 1
        for token in line.split(':', 1)[1].split():
            if not token.isdigit():
                break
            total += int(token)
    if not queues:
        raise ValueError('Network Tx-Rx interrupt labels unavailable')
    return total


def total_irq():
    return int(next(line.split()[1] for line in Path('/proc/stat').read_text().splitlines() if line.startswith('intr ')))


def verify(pid, hold_start, config, out):
    deadline = hold_start - 1
    while time.time() < deadline:
        threads = {}
        for path in Path(f'/proc/{pid}/task').glob('*'):
            try:
                stat = (path/'stat').read_text().rsplit(')', 1)[1].split()
                name = (path/'comm').read_text().strip()
                threads[name] = {'nice': int(stat[16]), 'policy': os.sched_getscheduler(int(path.name)),
                                 'cpus': sorted(os.sched_getaffinity(int(path.name)))}
            except (FileNotFoundError, ProcessLookupError):
                pass
        if {'slither-sockets', 'slither-simulat'}.issubset(threads) and all(
            t['nice'] == (config['socketNice'] if name == 'slither-sockets' else 0)
            and t['policy'] == os.SCHED_OTHER and t['cpus'] == [0] for name, t in threads.items()):
            actual = readback()
            if actual['values'] != config['expected'] or time.time() >= hold_start:
                raise RuntimeError('NIC verification missed or disagreed before hold')
            network_irq()
            Path(out).write_text(json.dumps({'unix': time.time(), 'holdStartUnix': hold_start,
                                            'threads': threads, 'coalesce': actual})+'\n')
            return
        time.sleep(.05)
    raise RuntimeError('Thread settings not verified before warmup ended')


if __name__ == '__main__':
    mode = sys.argv[1]
    if mode == 'capture':
        Path(sys.argv[2]).write_text(json.dumps(readback())+'\n')
    elif mode == 'configure':
        Path(sys.argv[4]).write_text(json.dumps(configure(sys.argv[2], json.loads(Path(sys.argv[3]).read_text())))+'\n')
    elif mode == 'capability':
        baseline = json.loads(Path(sys.argv[2]).read_text())
        before = readback(); attempt = command(['adaptive-rx', 'on']); after = readback()
        restored = restore(baseline['values'])
        Path(sys.argv[3]).write_text(json.dumps({'before': before, 'command': attempt, 'after': after,
            'supported': attempt['exit'] == 0 and after['values']['adaptiveRx'] == 'on',
            'restoreCommand': restored, 'restored': readback()})+'\n')
    elif mode == 'verify':
        verify(int(sys.argv[2]), float(sys.argv[3]), json.loads(Path(sys.argv[4]).read_text()), sys.argv[5])
    else:
        raise ValueError('Unknown mode')
