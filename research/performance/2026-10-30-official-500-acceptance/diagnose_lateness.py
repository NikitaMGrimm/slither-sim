"""Post-result reduction of retained v1 reports; never infer unrecorded timings."""
from pathlib import Path
import argparse, collections, hashlib, json, math, tarfile

p = argparse.ArgumentParser()
p.add_argument('archive', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
assert hashlib.file_digest(a.archive.open('rb'), 'sha256').hexdigest() == '4002cf60512b63558e163e252db84f9f33782fb17efe415c35d70dda3a6323f9'
with tarfile.open(a.archive) as archive:
    def read(name):
        return archive.extractfile(name).read().decode()
    summary = json.loads(read('run-summary.json'))
    window = summary['windows'][0]
    start, end = window['start']['unix'], window['end']['unix']
    canonical_start, canonical_end = window['result']['windowUnix']
    # Identical overlapping-report selection to the frozen ladder gate.
    client_lines = read('load-run/client.log').splitlines()
    all_rows = [json.loads(s[7:]) for s in client_lines if s.startswith('CLIENT ')]
    publications = []
    for index, line in enumerate(client_lines):
        if line.startswith('CLIENT_WINDOW '):
            value = json.loads(line[14:])
            publications.append({'worker': value['worker'], 'bytes': len(line.encode()) + 1,
                                 'metricEndOffsetSeconds': value['endUnix'] - start,
                                 'nextClientReportWorker': next(json.loads(s[7:])['worker'] for s in client_lines[index + 1:] if s.startswith('CLIENT '))})
    markers = [json.loads(s) for s in read('load-run/marker-normalization.jsonl').splitlines()]
    rows = []
    for worker in range(1, 7):
        previous = None
        for row in (v for v in all_rows if v['worker'] == worker):
            if canonical_start - 1.1 <= row['unix'] <= canonical_end + 1.1:
                assert previous is not None
                rows.append({
                    'worker': worker,
                    'offsetSeconds': row['unix'] - start,
                    'intervalSeconds': row['unix'] - previous['unix'],
                    **{k: row[k] for k in ['lateP99Nanos', 'lateMaxNanos', 'maxPassNanos', 'cpuFraction']},
                    'joins': row['joins'] - previous['joins'],
                    'deaths': row['deaths'] - previous['deaths'],
                })
            previous = row
    host = read('host-run/host.log').splitlines()
    prefixes = collections.Counter(s.split(' ', 1)[0] for s in host if s)
    census = [json.loads(s[7:]) for s in host if s.startswith('CENSUS ')]
    sockets = [json.loads(s[7:]) for s in host if s.startswith('SOCKET ')]

bad = [v for v in rows if v['lateP99Nanos'] >= 42_000_000]
def public(row):
    out = dict(row)
    out['reconnectDurations'] = None
    out['reconnectTimingStatus'] = 'Not recorded; aggregate joins are completions, not start/end timings.'
    return out
worst = []
for worker in range(1, 7):
    chosen = sorted((v for v in rows if v['worker'] == worker), key=lambda v: v['lateP99Nanos'], reverse=True)[:1]
    for row in chosen:
        out = public(row)
        publication = next(v for v in publications if v['worker'] == worker)
        out['metricPublication'] = publication
        out['reportAfterMetricEndSeconds'] = row['offsetSeconds'] - publication['metricEndOffsetSeconds']
        at = start + row['offsetSeconds']
        before = max((v for v in census if v['unix'] <= at - row['intervalSeconds']), key=lambda v: v['unix'])
        after = min((v for v in census if v['unix'] >= at), key=lambda v: v['unix'])
        clean = lambda v: {k: v[k] for k in ['tick', 'living', 'occupied', 'deaths', 'deferred']}
        near = [v for v in sockets if at - row['intervalSeconds'] - 1 <= v['unix'] <= at + 1]
        bracket = {'beforeOffsetSeconds': before['unix'] - start, 'afterOffsetSeconds': after['unix'] - start, 'before': clean(before), 'after': clean(after), 'socketSamples': [{'offsetSeconds': v['unix'] - start, **{k: v[k] for k in ['players', 'writeBlocks', 'bufferFull', 'rejects', 'queueBytes', 'kernelSendBytes']}} for v in near]}
        if worst:
            assert bracket == host_bracket, 'Publish separate brackets if episodes differ'
        host_bracket = bracket
        worst.append(out)

per_worker = {}
for worker in range(1, 7):
    selected = [v for v in bad if v['worker'] == worker]
    per_worker[str(worker)] = {
        'reports': sum(v['worker'] == worker for v in rows),
        'violatingReports': len(selected),
        'violationsWithJoins': sum(v['joins'] > 0 for v in selected),
        'violationsWithDeaths': sum(v['deaths'] > 0 for v in selected),
        'firstOffsetSeconds': min((v['offsetSeconds'] for v in selected), default=None),
        'lastOffsetSeconds': max((v['offsetSeconds'] for v in selected), default=None),
    }
out = {
    'label': 'Post-result diagnosis; unchanged v1 gates and outcome',
    'selection': 'Frozen ladder overlapping one-second reports; interval boundaries and offsets retained.',
    'thresholdNanos': 42_000_000,
    'reports': len(rows), 'violatingWorkerReports': len(bad),
    'violatingWallSecondBins': len({math.floor(v['offsetSeconds']) for v in bad}),
    'violatingWorkerReportsByMinute': dict(sorted(collections.Counter(str(math.floor(v['offsetSeconds'] / 60)) for v in bad).items(), key=lambda x: int(x[0]))),
    'workers': per_worker, 'worstPerWorker': worst,
    'requestedEndOffsetSeconds': end - start,
    'metricMarkerOffsets': [{'marker': v['marker'], 'offsetSeconds': v['unix'] - start} for v in markers],
    'publicationOrder': publications,
    'hostLateEpisode': host_bracket,
    'beforeRequestedEnd': {'reports': sum(v['offsetSeconds'] <= end - start for v in rows), 'maxLateP99Nanos': max(v['lateP99Nanos'] for v in rows if v['offsetSeconds'] <= end - start), 'maxPassNanos': max(v['maxPassNanos'] for v in rows if v['offsetSeconds'] <= end - start)},
    'hostLogPrefixes': dict(prefixes),
    'missingEvidence': ['Individual reconnect start/end and mutex-wait timings', 'TCP accept, HTTP-upgrade completion and per-snake admission event timestamps', 'Per-call blocking/scheduler traces'],
}
assert len(bad) == 6 and all(v['offsetSeconds'] > end - start for v in bad)
assert out['beforeRequestedEnd']['maxLateP99Nanos'] < 42_000_000
assert max(v['lateP99Nanos'] for v in rows) / 1e6 == window['result']['client']['worstInputP99Ms']
a.output.write_text(json.dumps(out, indent=2) + '\n')
print(json.dumps({k: out[k] for k in ['reports', 'violatingWorkerReports', 'violatingWallSecondBins', 'violatingWorkerReportsByMinute', 'workers', 'hostLogPrefixes']}))
for row in sorted(worst, key=lambda v: v['lateP99Nanos'], reverse=True):
    print(json.dumps({k: row[k] for k in ['worker', 'offsetSeconds', 'intervalSeconds', 'lateP99Nanos', 'lateMaxNanos', 'maxPassNanos', 'cpuFraction', 'joins', 'deaths']}))
