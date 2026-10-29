"""Request-boundary diagnostics, never an exclusion rule or hot-loop wrapper."""
from pathlib import Path
import os,time


def snapshot(root):
    started = time.monotonic()
    aggregate = list(map(int, Path('/proc/stat').read_text().splitlines()[0].split()[1:]))
    processes, roots = {}, set()
    for path in Path('/proc').iterdir():
        if not path.name.isdigit():
            continue
        try:
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
            args = (path / 'cmdline').read_bytes().split(b'\0')
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        pid = int(path.name)
        processes[pid] = {'ppid': int(fields[1]), 'start': int(fields[19]),
                          'own': int(fields[11]) + int(fields[12]),
                          'waited': int(fields[13]) + int(fields[14])}
        if any(str(root).encode() in arg for arg in args):
            roots.add(pid)
    while True:
        expanded = roots | {pid for pid, v in processes.items() if v['ppid'] in roots}
        if expanded == roots:
            break
        roots = expanded
    for pid, value in processes.items():
        value['study'] = pid in roots
    return {'monotonic': started, 'hz': os.sysconf('SC_CLK_TCK'),
            'busy': sum(aggregate[i] for i in [0, 1, 2, 5, 6]),
            'steal': aggregate[7], 'processes': processes}


def finish(before, after):
    assert before['hz'] == after['hz'] and after['monotonic'] >= before['monotonic']
    hz = before['hz']
    def records(s):
        return {(pid, v['start']): v for pid, v in s['processes'].items()}
    left, right = records(before), records(after)
    study_before = sum(v['own'] + v['waited'] for v in left.values() if v['study'])
    study_after = sum(v['own'] + v['waited'] for v in right.values() if v['study'])
    study_delta = study_after - study_before
    busy_delta = after['busy'] - before['busy']
    visible = 0
    for key, value in right.items():
        previous = left.get(key)
        if value['study'] or (previous and previous['study']):
            continue
        visible += value['own'] - (previous['own'] if previous else 0)
    return {
        'wallSeconds': after['monotonic'] - before['monotonic'],
        'counterQuantumMs': 1000 / hz,
        'guestBusyCpuMs': busy_delta * 1000 / hz,
        'studyTreeCpuMs': study_delta * 1000 / hz,
        'guestBusyMinusStudyCpuMsRaw': (busy_delta - study_delta) * 1000 / hz,
        'visibleNonStudyCpuMs': visible * 1000 / hz,
        'guestStealMs': (after['steal'] - before['steal']) * 1000 / hz,
        'newNonStudyProcesses': sum(not v['study'] and key not in left for key, v in right.items()),
        'exitedNonStudyProcesses': sum(not v['study'] and key not in right for key, v in left.items()),
        'exclusionApplied': False,
    }
