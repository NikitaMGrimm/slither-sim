"""Recompute bounded run summaries; raw replay data remains restricted and local."""
import csv
import json
from pathlib import Path
import statistics
import sys

ROOT = Path(__file__).resolve().parent


def aggregate():
    rows = list(csv.DictReader((ROOT / 'runs.csv').open(newline='', encoding='utf-8')))
    identities = json.loads((ROOT / 'identities.json').read_text(encoding='utf-8'))
    for row in rows:
        arm = identities['arms'][row['arm']]
        parity = identities['parity'][row['view']][row['arm']]
        assert row['binary_sha256'] == arm['binary_sha256']
        assert row['frame_source_sha256'] == arm['measured_frame_source_sha256']
        assert identities['base'].startswith(row['source_base'])
        assert row['numeric_sha256'] == parity['numeric_sha256']
        assert (int(row['callbacks']), int(row['messages']), int(row['skips'])) == (600, 29044, 0)
    lines = ['| View / median across complete pairs | Before | After | Paired reduction range |',
             '| --- | ---: | ---: | ---: |']
    notes = []
    for view in ['default', 'whole_map']:
        pairs = {}
        for row in rows:
            if row['view'] == view and row['status'] == 'accepted':
                pair = pairs.setdefault(row['pair'], {})
                assert row['arm'] not in pair
                pair[row['arm']] = row
        pairs = [p for p in pairs.values() if set(p) == {'control', 'candidate'}]
        assert len(pairs) >= 5, (view, len(pairs))
        for key, label, divisor, unit in [
            ('art_p50_ns', 'art callback p50', 1e6, 'ms'),
            ('art_p95_ns', 'art callback p95', 1e6, 'ms'),
            ('wall_ns_p95', 'all callbacks p95', 1e6, 'ms'),
            ('wall_ns_max', 'worst callback', 1e6, 'ms'),
            ('submit_ns_p50', 'CPU submission p50', 1e6, 'ms'),
            ('redraw_ns_p50', 'advance/preparation p50', 1e6, 'ms'),
            ('gpu_ns_p50', 'GPU passes p50', 1e6, 'ms'),
            ('elapsed_s', 'process elapsed', 1, 's'),
            ('alloc_bytes', 'requested allocations', 1e9, 'GB'),
        ]:
            values = {a: [float(p[a][key]) for p in pairs] for a in ['control', 'candidate']}
            delta = [(1 - float(p['candidate'][key]) / float(p['control'][key])) * 100 for p in pairs]
            med = {a: statistics.median(v) / divisor for a, v in values.items()}
            lines.append(f'| {view} / {label} | {med["control"]:.3f} {unit} | {med["candidate"]:.3f} {unit} | {min(delta):.1f} to {max(delta):.1f}% |')
        for arm in ['control', 'candidate']:
            selected = [p[arm] for p in pairs]
            notes.append(f'\n{view} {arm}: {len(selected)} runs, art p50 range '
                         f'{min(float(r["art_p50_ns"]) for r in selected)/1e6:.3f}–'
                         f'{max(float(r["art_p50_ns"]) for r in selected)/1e6:.3f} ms; '
                         f'load {min(float(r["load_min"]) for r in selected):.1f}–'
                         f'{max(float(r["load_max"]) for r in selected):.1f}%; '
                         f'art callbacks >16.7 ms '
                         f'{min(int(r["art_over_budget_callbacks"]) for r in selected)}–'
                         f'{max(int(r["art_over_budget_callbacks"]) for r in selected)} / 300; '
                         f'longest over-budget virtual interval '
                         f'{statistics.median(float(r["longest_over_budget_virtual_ms"]) for r in selected):.3f} ms.\n')
    lines.extend(notes)
    lines.append(f'\nAll runs: {len(rows)}; accepted: {sum(r["status"] == "accepted" for r in rows)}; '
                 f'excluded: {sum(r["status"] != "accepted" for r in rows)}.\n')
    return '\n'.join(lines)


def diagnostics():
    rows = list(csv.DictReader((ROOT / 'diagnostics.csv').open(newline='', encoding='utf-8')))
    lines = ['| Art-period stage / one untimed diagnostic | Before p50 / p95 ms | After p50 / p95 ms |',
             '| --- | ---: | ---: |']
    for stage in ['redraw', 'gpu_pack', 'section_food', 'advance', 'section_resources',
                  'gpu_sort', 'gpu_upload', 'gpu_encode', 'gpu_actual']:
        values = {r['arm']: r for r in rows if r['stage'] == stage}
        def value(arm):
            row = values[arm]
            assert int(row['callbacks']) == 300
            return f'{float(row["p50_ns"])/1e6:.3f} / {float(row["p95_ns"])/1e6:.3f}'
        lines.append(f'| {stage} | {value("control")} | {value("candidate")} |')
    return '\n'.join(lines)


def review_summary():
    with (ROOT / 'runs.csv').open(newline='', encoding='utf-8') as stream:
        runs = list(csv.DictReader(stream))
    pairs = {}
    for row in runs:
        if row['view'] == 'default' and row['status'] == 'accepted':
            pairs.setdefault(row['pair'], {})[row['arm']] = row
    changes = [100 * (1 - float(p['candidate']['art_p50_ns']) / float(p['control']['art_p50_ns']))
               for p in pairs.values() if set(p) == {'control', 'candidate'}]
    lines = [f'{sum(c > 0 for c in changes)} of {len(changes)} paired default-art medians improve; '
             f'median paired reduction {statistics.median(changes):.3f}%.', '',
             '| Independent converter-only scene | Main ns/quad | Candidate ns/quad | Change | Extra per traversal |',
             '| --- | ---: | ---: | ---: | ---: |']
    with (ROOT / 'review-converter.csv').open(newline='', encoding='utf-8') as stream:
        rows = list(csv.DictReader(stream))
    for scene, count in [('sparse-256', 256), ('miss-32768', 32768), ('paired-32768', 32768)]:
        selected = [r for r in rows if r['scene'] == scene]
        assert len({(r['pair'], r['arm']) for r in selected}) == len(selected) == 20
        values = {arm: [float(r['ns_per_quad']) for r in selected if r['arm'] == arm]
                  for arm in ['base', 'candidate']}
        assert all(len(v) == 10 for v in values.values())
        before, after = (statistics.median(values[a]) for a in ['base', 'candidate'])
        lines.append(f'| {scene} | {before:.3f} | {after:.3f} | {100*(after/before-1):+.3f}% | '
                     f'{(after-before)*count/1e6:+.6f} ms |')
    return '\n'.join(lines)


if __name__ == '__main__':
    result = aggregate()
    print(result)
    print(diagnostics())
    print(review_summary())
    if '--check' in sys.argv:
        report = (ROOT / 'REPORT.md').read_text(encoding='utf-8')
        assert report.split('<!-- BEGIN RESULTS -->\n', 1)[1].split('\n<!-- END RESULTS -->', 1)[0] == result
        assert report.split('<!-- BEGIN DIAGNOSTICS -->\n', 1)[1].split('\n<!-- END DIAGNOSTICS -->', 1)[0] == diagnostics()
        assert report.split('<!-- BEGIN REVIEW -->\n', 1)[1].split('\n<!-- END REVIEW -->', 1)[0] == review_summary()
