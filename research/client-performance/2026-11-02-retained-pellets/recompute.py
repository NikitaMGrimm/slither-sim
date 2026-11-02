"""Stdlib recomputation of bounded run summaries, with no Git/current-file gates."""
import csv
import hashlib
import json
from pathlib import Path
import statistics
import sys

ROOT = Path(__file__).resolve().parent


def records():
    identity = json.loads((ROOT / 'identities.json').read_text(encoding='utf-8'))
    assert hashlib.sha256((ROOT / 'reviewed-source.patch').read_bytes()).hexdigest() == identity['reviewed_source_patch_sha256']
    for scene, arms in identity['parity'].items():
        for arm, r in arms.items():
            assert (r['callbacks'], r['skips']) == (600, 0)
            assert r['messages'] == (8 if scene == 'rectangle' else 29044)
            assert r['binary_sha256'] == identity['binaries'][arm]
        for key in ['output_digest', 'quad_digest', 'numeric_sha256']:
            assert arms['baseline'][key] == arms['candidate'][key]
    for r in identity['pixel_parity'].values():
        assert r['callbacks'] == 600 and r['frames_over_2'] == 0 and r['max_rgb'] <= 2
    with (ROOT / 'runs.csv').open(newline='', encoding='utf-8') as f:
        rows = list(csv.DictReader(f))
    assert len({(r['pair'], r['quality'], r['arm']) for r in rows}) == len(rows)
    for r in rows:
        assert r['quality'] in ['original', 'native']
        assert (r['callbacks'], r['skips'], r['messages']) == ('600', '0', '29044')
        assert r['binary_sha256'] == identity['binaries'][r['arm']]
        assert r['base'] == identity['base']
        assert r['numeric_sha256'] == identity['parity']['join-default'][r['arm']]['numeric_sha256']
        assert r['link_samples'] == '0'
        assert (r['status'], r['reason']) == (
            ('excluded', 'observed_rustc') if int(r['rustc_samples']) else ('accepted', 'none'))
    return rows


def tables():
    rows = records()
    lines = ['| Quality / median of per-run quantiles | Before ms | After ms | Paired reduction range |',
             '| --- | ---: | ---: | ---: |']
    notes = []
    metrics = [('all_wall_ns_p50', 'first 10 s callback p50'),
               ('all_wall_ns_p95', 'first 10 s callback p95'),
               ('all_redraw_ns_p50', 'first 10 s preparation p50'),
               ('all_submit_ns_p50', 'first 10 s CPU submit p50'),
               ('all_gpu_ns_p50', 'first 10 s GPU p50'),
               ('art_wall_ns_p50', 'art callback p50'),
               ('art_wall_ns_p95', 'art callback p95'),
               ('art_redraw_ns_p50', 'art preparation p50'),
               ('art_submit_ns_p50', 'art CPU submit p50'),
               ('art_gpu_ns_p50', 'art GPU p50'),
               ('art_cpu_ns_p50', 'art CPU stages p50'),
               ('all_wall_ns_max', 'first 10 s worst callback')]
    for quality in ['original', 'native']:
        pairs = {}
        for r in rows:
            if r['quality'] == quality:
                pair = pairs.setdefault(r['pair'], {})
                assert r['arm'] not in pair
                pair[r['arm']] = r
        assert all(set(p) == {'baseline', 'candidate'} for p in pairs.values())
        pairs = [p for p in pairs.values() if all(r['status'] == 'accepted' for r in p.values())]
        assert len(pairs) >= 5
        for key, label in metrics:
            values = {a: [float(p[a][key]) for p in pairs] for a in ['baseline', 'candidate']}
            delta = [100 * (1 - float(p['candidate'][key]) / float(p['baseline'][key])) for p in pairs]
            lines.append(f'| {quality} / {label} | {statistics.median(values["baseline"])/1e6:.3f} | '
                         f'{statistics.median(values["candidate"])/1e6:.3f} | {min(delta):.1f} to {max(delta):.1f}% |')
        for arm in ['baseline', 'candidate']:
            runs = [p[arm] for p in pairs]
            notes.append(f'{quality} {arm}: {len(runs)} runs; art p50 range '
                         f'{min(float(r["art_wall_ns_p50"]) for r in runs)/1e6:.3f} to '
                         f'{max(float(r["art_wall_ns_p50"]) for r in runs)/1e6:.3f} ms; load '
                         f'{min(float(r["load_min"]) for r in runs):.1f} to '
                         f'{max(float(r["load_max"]) for r in runs):.1f}%; observed cargo/test/archive samples '
                         + '/'.join(str(sum(int(r[k]) for r in runs)) for k in ['cargo_samples', 'test_samples', 'archive_samples'])
                         + f'; worst callback range {min(float(r["all_wall_ns_max"]) for r in runs)/1e6:.3f} to '
                         f'{max(float(r["all_wall_ns_max"]) for r in runs)/1e6:.3f} ms.')
    return '\n'.join(lines) + '\n\n' + '\n\n'.join(notes) + (
        f'\n\nAll runs: {len(rows)}; accepted {sum(r["status"] == "accepted" for r in rows)}; '
        f'excluded {sum(r["status"] == "excluded" for r in rows)}.')


def diagnostics():
    with (ROOT / 'diagnostics.csv').open(newline='', encoding='utf-8') as f:
        rows = list(csv.DictReader(f))
    lines = ['| Diagnostic / candidate art p50 ms | Original | Native |', '| --- | ---: | ---: |']
    for case in ['join', 'rectangle']:
        for stage in ['redraw_ns', 'submit_ns', 'gpu_ns', 'pellet_prepare', 'food_advance', 'glow_passes', 'other_gpu']:
            values = {r['quality']: r for r in rows if r['case'] == case and r['arm'] == 'candidate' and r['stage'] == stage}
            assert set(values) == {'original', 'native'}
            assert all(r['callbacks'] == '300' for r in values.values())
            lines.append(f'| {case} / {stage} | {float(values["original"]["p50_ms"]):.3f} | {float(values["native"]["p50_ms"]):.3f} |')
    return '\n'.join(lines)


if __name__ == '__main__':
    output = tables()
    print(output)
    print(diagnostics())
    if '--check' in sys.argv:
        report = (ROOT / 'REPORT.md').read_text(encoding='utf-8')
        assert report.split('<!-- BEGIN RESULTS -->\n')[1].split('\n<!-- END RESULTS -->')[0] == output
        assert report.split('<!-- BEGIN DIAGNOSTICS -->\n')[1].split('\n<!-- END DIAGNOSTICS -->')[0] == diagnostics()
