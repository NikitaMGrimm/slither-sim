"""Selected post-window host diagnostics; never select/exclude acceptance ticks.

One-second covering intervals retain their whole CPU deltas (no interpolation).
Only matched PID/start identities have known deltas. Exited/short-lived tasks
may be missed; core user residual is a separate check, not an attribution trace.
"""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import tarfile


def cpu_fields(text):
    rows = {line.split()[0]: line.split()[1:] for line in text.splitlines()}
    return [int(x) for x in rows['cpu'][:8]], int(rows['ctxt'][0]), int(rows['procs_running'][0])


def reduce(archive):
    with tarfile.open(archive) as tar:
        read = lambda name: json.load(tar.extractfile('host-run/' + name))
        window = json.load(tar.extractfile('run-summary.json'))['windows'][0]
        start, end = window['start']['unix'], window['end']['unix']
        rows = [json.loads(line) for line in tar.extractfile('host-run/background.jsonl')]
        host = [json.loads(line) for line in tar.extractfile('host-run/telemetry.jsonl')]
        roles = {read('process.json')['pid']: 'server',
                 read('diagnostic-start.json')['pid']: 'diagnostic',
                 host[0]['collector']['process']['pid']: 'observer'}
        hz = read('diagnostic-start.json')['clockTicksPerSecond']
        intervals = []
        names = defaultdict(lambda: [0, 0])
        for a, b in zip(rows, rows[1:]):
            if not (a['unix'] <= end and b['unix'] >= start):
                continue
            previous = {(v['pid'], v['start']): v for v in a['processes']}
            by_role = defaultdict(lambda: [0, 0])
            unmatched = 0
            for v in b['processes']:
                old = previous.get((v['pid'], v['start']))
                if old is None:
                    unmatched += 1
                    continue
                du, ds = v['userTicks'] - old['userTicks'], v['systemTicks'] - old['systemTicks']
                assert du >= 0 and ds >= 0
                role = roles.get(v['pid'], 'other')
                by_role[role][0] += du
                by_role[role][1] += ds
                names[role][0] += du
                names[role][1] += ds
            ca, xa, _ = cpu_fields(a['procStat'])
            cb, xb, queue = cpu_fields(b['procStat'])
            core = [(y - x) / hz for x, y in zip(ca, cb)]
            known_user = sum(by_role[k][0] for k in ('server', 'observer', 'diagnostic')) / hz
            intervals.append({'fromOffsetSeconds': a['unix'] - start,
                'toOffsetSeconds': b['unix'] - start,
                'wallSeconds': b['unix'] - a['unix'], 'userCounterResidualSeconds': core[0] + core[1] - known_user,
                'matchedOtherUserSeconds': by_role['other'][0] / hz,
                'matchedOtherCpuSeconds': sum(by_role['other']) / hz,
                'ctxtDelta': xb - xa, 'procsRunning': queue,
                'coreSeconds': core, 'newOrUnmatchedProcesses': unmatched})
        assert intervals and intervals[0]['fromOffsetSeconds'] <= 0 <= intervals[-1]['toOffsetSeconds']
        assert intervals[-1]['toOffsetSeconds'] >= end - start
        total_wall = sum(v['wallSeconds'] for v in intervals)
        policy, before, after = read('image-policy/policy.json'), read('image-policy/pre-window.json'), read('image-policy/after.json')
        journal = tar.extractfile('host-run/window-journal.log').read().decode()
        journal_times = []
        for line in journal.splitlines():
            try:
                at = float(line.split()[0])
            except (ValueError, IndexError):
                continue
            if start <= at <= end:
                journal_times.append(at)
        out = {
            'scope': 'Whole one-second intervals covering the full requested window; descriptive, no exclusions',
            'intervalCount': len(intervals), 'sampledWallSeconds': total_wall,
            'maxSampleGapSeconds': max(v['wallSeconds'] for v in intervals),
            'maxScanSeconds': max(v['endUnix'] - v['unix'] for v in rows if start <= v['unix'] <= end),
            'meanCpuMillisecondsPerWallSecond': {name: (u + s) / hz / total_wall * 1000
                                                for name, (u, s) in sorted(names.items())},
            'userCounterResidualCpuSeconds': sum(v['userCounterResidualSeconds'] for v in intervals),
            'coreCpuSeconds': dict(zip(('user', 'nice', 'system', 'idle', 'iowait', 'irq', 'softirq', 'steal'),
                                      (sum(v['coreSeconds'][i] for v in intervals) for i in range(8)))),
            'ctxtDelta': sum(v['ctxtDelta'] for v in intervals),
            'procsRunningSampleRange': [min(v['procsRunning'] for v in intervals), max(v['procsRunning'] for v in intervals)],
            'largestMatchedOtherCpuIntervals': sorted(intervals, key=lambda v: v['matchedOtherCpuSeconds'], reverse=True)[:5],
            'timersPresent': policy['timersPresent'], 'timersAbsent': policy['timersAbsent'],
            'servicesStopped': policy['servicesStopped'], 'stopOnlyEtcUnits': policy['stopOnlyEtcUnits'],
            'preWindowStateOffsetSeconds': before['unix'] - start,
            'afterStateOffsetSeconds': after['unix'] - end,
            'beforeWindowAndAfterStates': {name: [[value['units'][name]['ActiveState'], value['units'][name]['UnitFileState']]
                                         for value in (before, after)] for name in policy['recordedUnits']},
            'beforeAndAfterTimerInventoriesRetained': all('host-run/image-policy/' + edge + '-timers.txt' in tar.getnames()
                                                          for edge in ('before', 'pre-window', 'after')),
            'journalBytes': len(journal.encode()), 'journalEntriesWithinWindow': len(journal_times),
            'limits': 'User counter residual includes nice CPU and may be negative: global/task classification and sampling do not reconcile exactly; it is not other-process CPU. Matched-process CPU omits tasks absent at either sample. Sequential scans, 10 ms counters '
                      'and own-sampler runnable state limit attribution; no per-tick off-CPU trace. Raw identities/journal stay private.',
        }
    with archive.open('rb') as stream:
        out['archiveSha256'] = hashlib.file_digest(stream, 'sha256').hexdigest()
    return out


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('archive', type=Path)
    p.add_argument('output', type=Path)
    args = p.parse_args()
    args.output.write_text(json.dumps(reduce(args.archive), indent=2) + '\n', encoding='utf-8')
