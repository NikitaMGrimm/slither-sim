"""Recompute the bounded summaries without captures, Git or current-source gates."""
import csv
import json
from pathlib import Path
import statistics
import sys

ROOT = Path(__file__).resolve().parent


def records():
    identity = json.loads((ROOT / 'identities.json').read_text(encoding='utf-8'))
    with (ROOT / 'runs.csv').open(newline='', encoding='utf-8') as stream:
        rows = list(csv.DictReader(stream))
    assert len({r['label'] for r in rows}) == len(rows)
    for r in rows:
        assert (r['callbacks'], r['messages'], r['skips']) == ('600', '29044', '0')
        assert r['binary_sha256'] == identity['timing_binaries'][r['arm']]
        assert (r['status'], r['reason']) == (
            ('excluded', 'observed_compile') if int(r['compile_samples']) else ('accepted', 'none'))
        control = identity['controls']['/'.join([r['adapter'], r['physical'], r['quality']])]
        assert r['output_digest'] == control['output_digest']
        # Historical diagnostic metadata only: this is the untouched accumulator,
        # not an arm-to-arm canonical parity gate. Images are checked below.
        assert r['quad_digest'] == 'd1fba762150c532c'
        assert r['frame_numeric_sha256'] == control['frame_numeric_sha256']
    for case, r in identity['image_parity'].items():
        assert (r['callbacks'], r['skips'], r['messages']) == (
            600, 0, 8 if case.startswith('rectangle/') else 29044)
        assert r['binary_sha256'] == identity['image_binaries']['candidate']
        assert r['baseline_binary_sha256'] == identity['image_binaries']['baseline']
        assert r['image_sequence_sha256'] == r['baseline_image_sequence_sha256']
    assert len(identity['image_parity']) == 10
    return rows


def results():
    rows = records()
    lines = ['| Configuration / art metric | Before ms | After ms | Paired reduction range |',
             '| --- | ---: | ---: | ---: |']
    notes = []
    for adapter, size in [('rtx', '3840x2160'), ('igpu', '3840x2160'), ('igpu', '1920x1080')]:
        for quality in ['original', 'native']:
            selected = [r for r in rows if r['candidate'] != 'mirror' and
                        (r['adapter'], r['physical'], r['quality']) == (adapter, size, quality)]
            pairs = {}
            for r in selected:
                pair = pairs.setdefault(r['round'], {})
                assert r['arm'] not in pair
                pair[r['arm']] = r
            assert all(set(p) == {'baseline', 'mirror-inline'} for p in pairs.values())
            pairs = [p for p in pairs.values() if all(r['status'] == 'accepted' for r in p.values())]
            assert len(pairs) >= 5
            metrics = [('wall_ns_p50_ms', 'callback p50'), ('wall_ns_p95_ms', 'callback p95'),
                       ('cpu_ns_p50_ms', 'CPU stages'), ('pellet_prepare_p50_ms', 'pellet preparation'),
                       ('submit_ns_p50_ms', 'CPU submit'), ('gpu_ns_p50_ms', 'GPU'),
                       ('ten_wall_ns_p50_ms', '10 s callback p50'),
                       ('ten_wall_ns_p95_ms', '10 s callback p95')]
            for key, label in metrics:
                before = [float(p['baseline'][key]) for p in pairs]
                after = [float(p['mirror-inline'][key]) for p in pairs]
                delta = [100 * (1 - a / b) for a, b in zip(after, before)]
                lines.append(f'| {adapter} {size} {quality} / {label} | {statistics.median(before):.3f} | '
                             f'{statistics.median(after):.3f} | {min(delta):+.1f} to {max(delta):+.1f}% |')
            allocation = {a: statistics.median(float(p[a]['art_requested_alloc_p50']) for p in pairs)
                          for a in ['baseline', 'mirror-inline']}
            notes.append(f'{adapter} {size} {quality}: median requested allocation per art callback '
                         f'{allocation["baseline"]:.0f} to {allocation["mirror-inline"]:.0f} bytes.')
            for arm in ['baseline', 'mirror-inline']:
                runs = [p[arm] for p in pairs]
                notes.append(f'{adapter} {size} {quality} {arm}: {len(runs)} runs; callback p50 range '
                             f'{min(float(r["wall_ns_p50_ms"]) for r in runs):.3f} to '
                             f'{max(float(r["wall_ns_p50_ms"]) for r in runs):.3f} ms; load '
                             f'{min(float(r["load_min"]) for r in runs):.1f} to '
                             f'{max(float(r["load_max"]) for r in runs):.1f}%; cargo/test samples '
                             + '/'.join(str(sum(int(r[k]) for r in runs)) for k in ['cargo_samples', 'test_samples']) + '.')
    return '\n'.join(lines) + '\n\n' + '\n\n'.join(notes) + (
        f'\n\nAll retained runs: {len(rows)}; accepted {sum(r["status"] == "accepted" for r in rows)}; '
        f'excluded {sum(r["status"] == "excluded" for r in rows)}.')


if __name__ == '__main__':
    output = results()
    if '--check' in sys.argv:
        report = (ROOT / 'REPORT.md').read_text(encoding='utf-8')
        assert report.split('<!-- BEGIN RESULTS -->\n')[1].split('\n<!-- END RESULTS -->')[0] == output
        print('Retained tables and parity metadata match.')
    else:
        print(output)
