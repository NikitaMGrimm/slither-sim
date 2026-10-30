"""Post-result host diagnosis from a retained archive; changes no selection/gate.

The fixed +744..751 s context covers the run-2 episode and the same offsets in
run 1. CPU counters have 10 ms granularity; snapshots are sequential, not atomic.
Only safe role labels and selected daemon names leave the private inventories.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import tarfile


CPU_FIELDS = "user nice system idle iowait irq softirq steal guest guestNice".split()
DAEMONS = (
    "systemd-journald", "snapd", "amazon-ssm-agent", "cron", "rsyslogd",
    "unattended-upgrade-shutdown", "fwupd", "apt-daily", "fstrim",
    "man-db", "cloud-init",
)


def delta(before, after):
    assert (before['pid'], before['start']) == (after['pid'], after['start'])
    return {key: round(after[key] - before[key], 6) for key in ('user', 'system')}


def total(cpu):
    return round(cpu['user'] + cpu['system'], 6)


def interval(before, after, start):
    core = {key: round((int(y) - int(x)) / 100, 6)
            for key, x, y in zip(CPU_FIELDS, before['core'].split()[1:],
                                 after['core'].split()[1:])}
    server = delta(before['server']['process'], after['server']['process'])
    observer = delta(before['collector']['process'], after['collector']['process'])
    threads = {}
    for role in ('server', 'collector'):
        old = {t['pid']: t for t in before[role]['threads']}
        values = {}
        for t in after[role]['threads']:
            if t['pid'] not in old or t['start'] != old[t['pid']]['start']:
                continue
            label = t['comm'] if role == 'server' else (
                'main' if t['pid'] == after[role]['process']['pid'] else 'helpers')
            values[label] = round(values.get(label, 0) + total(delta(old[t['pid']], t)), 6)
        threads[role] = values
    return {
        'fromOffsetSeconds': round(before['server']['unix'] - start, 6),
        'toOffsetSeconds': round(after['server']['unix'] - start, 6),
        'elapsedSeconds': round(after['server']['unix'] - before['server']['unix'], 6),
        'serverCpuSeconds': server, 'observerCpuSeconds': observer,
        'matchedThreadCpuSeconds': threads, 'coreSeconds': core,
        # Use user only: task system accounting can also include softirq work.
        'otherUserCpuSeconds': round(core['user'] - server['user'] - observer['user'], 6),
    }


def reduce(archive):
    with archive.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    with tarfile.open(archive, 'r:gz') as tar:
        window = json.load(tar.extractfile('run-summary.json'))['windows'][0]
        start = window['start']['unix']
        samples = [json.loads(line) for line in tar.extractfile('host-run/telemetry.jsonl')]
        samples = [s for s in samples if 'core' in s]
        rows = [interval(a, b, start) for a, b in zip(samples, samples[1:])
                if 744 < b['server']['unix'] - start < 751]
        episode = [r for r in rows if 746 <= r['toOffsetSeconds'] < 750]
        assert len(episode) == 4
        summary = {
            'fromOffsetSeconds': episode[0]['fromOffsetSeconds'],
            'toOffsetSeconds': episode[-1]['toOffsetSeconds'],
            'elapsedSeconds': round(sum(r['elapsedSeconds'] for r in episode), 6),
            'coreSeconds': {k: round(sum(r['coreSeconds'][k] for r in episode), 6)
                            for k in CPU_FIELDS},
            'serverCpuSeconds': round(sum(total(r['serverCpuSeconds']) for r in episode), 6),
            'observerCpuSeconds': round(sum(total(r['observerCpuSeconds']) for r in episode), 6),
            'otherUserCpuSeconds': round(sum(r['otherUserCpuSeconds'] for r in episode), 6),
        }
        inventories = {}
        for edge in ('start', 'end'):
            text = tar.extractfile(f'host-run/acceptance-500-{edge}-processes.txt').read().decode()
            found = {}
            for line in text.splitlines()[1:]:
                columns = line.split(None, 4)
                if len(columns) != 5:
                    continue
                for name in DAEMONS:
                    if name in columns[4]:
                        found[name] = {'state': columns[2], 'lifetimeCpuPercent': float(columns[3])}
            inventories[edge] = found
        blocks = []
        log = tar.extractfile('load-run/host-stream.log').read().decode()
        for line in log.splitlines():
            if not line.startswith('CAPACITY '):
                continue
            cap = json.loads(line[9:])
            if not 300900 <= cap['tick'] <= 301500:
                continue
            values = cap['rows']
            assert len(values) == 100
            blocks.append({
                'afterTick': cap['tick'], 'offsetSeconds': round(cap['unix'] - start, 6),
                'worldCpuSumMs': sum(r[0] + r[1] for r in values) / 1e6,
                'bracketedWallSumMs': sum(r[2] for r in values) / 1e6,
                'bracketedWallOver10MsTicks': sum(r[2] > 10_000_000 for r in values),
                'bracketedWallMaxMs': max(r[2] for r in values) / 1e6,
            })
        return {
            'archiveSha256': digest,
            'episodeReferenceUtc': datetime.fromtimestamp(start + 746, timezone.utc).isoformat(),
            'scope': 'Post-result context only; no gate, tail, window or outcome changes',
            'cpuCounterQuantumSeconds': 0.01,
            'intervals': rows, 'fourSampleSummary': summary,
            'boundaryDaemonPresence': inventories, 'tickBlocks': blocks,
            'limits': 'Only server and observer have interval task counters. Matched threads omit '
                      'threads absent at either endpoint. Boundary ps CPU is lifetime, not interval. '
                      'No journal, timer inventory, context switches, run queue, per-task I/O or '
                      'scheduler trace was retained. Sequential samples and counter rounding limit attribution.',
        }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    result = reduce(args.archive)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps(result['fourSampleSummary']))
