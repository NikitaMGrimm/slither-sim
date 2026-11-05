"""Recompute the study tables from runs.csv and check REPORT.md.

usage: python -B recompute.py [--check]
A pair counts only when both of its runs are `accepted` (no rustc/link
process observed); every excluded attempt stays in runs.csv. Across runs, p95
is the sorted value at index floor((n-1)*0.95).
"""
from pathlib import Path
import csv, json, statistics, sys

sys.stdout.reconfigure(encoding='utf-8')
HERE = Path(__file__).resolve().parent
IDS = json.loads((HERE / 'identities.json').read_text(encoding='utf-8'))
BASE, CAND = IDS['arms']['baseline']['name'], IDS['arms']['candidate']['name']
METRICS = IDS['metrics']


def q95(values):
    values = sorted(values)
    return values[(len(values) - 1) * 95 // 100]


def tables():
    with (HERE / 'runs.csv').open(newline='', encoding='utf-8') as f:
        rows = list(csv.DictReader(f))
    pairs = {}
    for r in rows:
        key = (r['adapter'], r['view'], r['quality'], r['pair'])
        pairs.setdefault(key, {})[r['arm']] = r
    groups = {}
    excluded = 0
    for (adapter, view, quality, _), arms in sorted(pairs.items()):
        assert set(arms) == {BASE, CAND}, arms
        if all(arms[a]['status'] == 'accepted' for a in arms):
            groups.setdefault((adapter, view, quality), []).append(arms)
        else:
            excluded += 1
    lines = ['| Adapter / view / quality | Pairs | ' + ' | '.join(f'{label} before → after (median / p95 ms)' for label in METRICS.values()) + ' |',
             '| --- | ---: | ' + ' | '.join('---:' for _ in METRICS) + ' |']
    for (adapter, view, quality), group in groups.items():
        cells = []
        for field in METRICS:
            before = [float(p[BASE][field]) for p in group]
            after = [float(p[CAND][field]) for p in group]
            cells.append(f'{statistics.median(before):.1f} / {q95(before):.1f} → {statistics.median(after):.1f} / {q95(after):.1f}')
        lines.append(f'| {adapter} / {view} / {quality} | {len(group)} | ' + ' | '.join(cells) + ' |')
    loads = [float(r['load_p50']) for r in rows if r['status'] == 'accepted']
    summary = (f'Runs: {len(rows)}; pairs: {len(pairs)}; pairs excluded for observed compiler overlap: {excluded}. '
               f'Accepted-run median machine load: {min(loads):.1f}-{max(loads):.1f}%.')
    return {'TIMING': '\n'.join(lines), 'SUMMARY': summary}


def main():
    report = (HERE / 'REPORT.md').read_text(encoding='utf-8')
    for marker, text in tables().items():
        begin, end = f'<!-- {marker} -->\n', f'\n<!-- /{marker} -->'
        start = report.index(begin) + len(begin)
        stop = report.index(end, start)
        if '--check' in sys.argv:
            assert report[start:stop] == text, f'{marker} differs:\n{text}'
        else:
            report = report[:start] + text + report[stop:]
    if '--check' in sys.argv:
        print('REPORT.md tables match runs.csv')
    else:
        (HERE / 'REPORT.md').write_text(report, encoding='utf-8', newline='\n')
        print(tables()['TIMING'])


main()
