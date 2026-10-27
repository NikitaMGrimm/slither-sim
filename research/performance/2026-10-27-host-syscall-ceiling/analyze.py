"""Reduce syscall counts and explicit hypothetical boundary-elimination budgets."""
import csv
import json
from pathlib import Path
import re
import statistics

NETWORK = {'sendto', 'sendmsg', 'sendmmsg', 'write', 'writev', 'recvfrom', 'recvmsg',
           'recvmmsg', 'read', 'readv', 'poll', 'ppoll', 'epoll_wait', 'epoll_pwait', 'epoll_pwait2',
           'accept', 'accept4', 'connect', 'shutdown', 'getsockopt', 'setsockopt'}


def summary(values):
    values = list(values)
    return {'count': len(values), 'min': min(values), 'median': statistics.median(values), 'max': max(values)}


def micro(rows):
    valid = [r for r in rows if r['valid']]
    if not valid:
        raise ValueError('No valid microbenchmark repetitions')
    return {'attempts': len(rows), 'valid': len(valid),
            'nullCpuNsPerCall': summary(r['null']['cpuNs']/r['null']['calls'] for r in valid),
            'nullWallNsPerCall': summary(r['null']['wallNs']/r['null']['calls'] for r in valid),
            'emptyLoopCpuNsPerIteration': summary(r['null']['baselineCpuNs']/r['null']['calls'] for r in valid),
            'boundaryProxyCpuNs': summary((r['null']['cpuNs']-r['null']['baselineCpuNs'])/r['null']['calls'] for r in valid),
            'sendCpuNsPerCall': summary(r['send']['cpuNs']/r['send']['calls'] for r in valid),
            'sendWallNsPerCall': summary(r['send']['wallNs']/r['send']['calls'] for r in valid),
            'maxReceiverThreadCpuPercent': max(t['cpuNs']/t['wallNs']*100 for r in valid for t in r['receiver']['threads'])}


def perf_counts(path, names):
    """Parse perf -x ';' per-thread output and remove thread identifiers."""
    counts = {}
    for fields in csv.reader(Path(path).read_text().splitlines(), delimiter=';'):
        events = [(i, s.strip()) for i, s in enumerate(fields) if s.strip().startswith('syscalls:sys_enter_')]
        if not events:
            continue
        i, event = events[0]
        if i < 2:
            raise ValueError('Unexpected perf columns')
        raw = fields[i-2].strip().replace(',', '')
        if '<' in raw:
            raise ValueError('Uncounted/unsupported perf event')
        value = int(float(raw))
        label = fields[0].strip()
        # perf renders comm-TID; map only an exact trailing identifier from the private map.
        matches = [name for tid, name in names.items() if re.search(r'(?:-|\s)'+re.escape(tid)+r'$', label)]
        if len(matches) != 1:
            raise ValueError('Unknown perf thread label')
        thread = matches[0]
        event = event.removeprefix('syscalls:sys_enter_').split(':', 1)[0]
        d = counts.setdefault(thread, {})
        if event in d:
            raise ValueError('Duplicate per-thread syscall event')
        if value:
            d[event] = value
    if not counts or not sum(sum(v.values()) for v in counts.values()):
        raise ValueError('No syscall entries counted')
    return counts


def ceilings(counts, ticks, proxy):
    socket = counts.get('slither-sockets', {})
    groups = {'socketNetwork': sum(v for k, v in socket.items() if k in NETWORK),
              'socketAll': sum(socket.values()),
              'hostAll': sum(sum(v.values()) for v in counts.values())}
    result = {}
    for name, calls in groups.items():
        per_tick = calls/ticks
        result[name] = {'calls': calls, 'callsPerTick': per_tick,
                        'medianProxyMsPerTick': per_tick*proxy['median']/1e6,
                        'maxProxyMsPerTick': per_tick*proxy['max']/1e6,
                        'medianProxyCorePercent': per_tick*proxy['median']/70000,
                        'maxProxyCorePercent': per_tick*proxy['max']/70000}
    return result


def count_window(host):
    host = Path(host)
    window = json.loads((host/'syscall-window.json').read_text())
    origin, rows = json.loads((host/'ticks.json').read_text())
    ticks = sum(window['beforeUnix'] <= origin+r[0]*1e-9 < window['afterUnix'] for r in rows)
    if not ticks:
        raise ValueError('No executed ticks in count window')
    if window['method'] != 'perf':
        raise ValueError('strace fallback requires explicit whole-process parsing; thread attribution unavailable')
    counts = perf_counts(host/'syscalls.csv', window['threads'])
    return {'method': window['method'], 'configuredSeconds': window['configuredSeconds'],
            'wrapperSeconds': window['afterUnix']-window['beforeUnix'], 'executedTicks': ticks,
            'countsByThread': counts, 'denominatorLimit': window['denominator']}
