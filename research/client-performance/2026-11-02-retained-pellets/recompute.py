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
            assert r['quad_digest'] != 'd1fba762150c532c', 'inactive canonical accumulator'
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


def review_tables():
    identity = json.loads((ROOT / 'identities.json').read_text(encoding='utf-8'))['review_fixes']
    assert hashlib.sha256((ROOT / 'review-fixes-source.patch').read_bytes()).hexdigest() == identity['final_source_delta_sha256']
    for scene, arms in identity['parity'].items():
        for arm, r in arms.items():
            assert (r['callbacks'], r['skips'], r['messages']) == (600, 0, 8 if scene == 'rectangle' else 29044)
            assert r['binary_sha256'] == identity['binaries'][arm]
            assert r['quad_digest'] != 'd1fba762150c532c', 'inactive canonical accumulator'
        for key in ['output_digest', 'quad_digest', 'numeric_sha256']:
            assert arms['baseline'][key] == arms['candidate'][key]
    for r in identity['pixel_parity'].values():
        assert r['callbacks'] == 600 and r['frames_over_2'] == 0 and r['max_rgb'] <= 2
    with (ROOT / 'review-runs.csv').open(newline='', encoding='utf-8') as stream:
        rows = list(csv.DictReader(stream))
    assert len({(r['pair'], r['quality'], r['arm']) for r in rows}) == len(rows)
    for r in rows:
        assert (r['callbacks'], r['messages'], r['skips']) == ('600', '29044', '0')
        assert r['numeric_sha256'] == identity['parity']['join-default']['candidate']['numeric_sha256']
        assert r['binary_sha256'] == identity['binaries'][r['arm']]
        assert (r['status'], r['reason']) == (('excluded', 'observed_compile') if int(r['compile_samples']) else ('accepted', 'none'))
    lines = ['| Review-fix comparison / median of per-run quantiles | Main ms | Before review ms | Final ms | Final vs before review paired range |',
             '| --- | ---: | ---: | ---: | ---: |']
    notes = []
    for quality in ['original', 'native']:
        pairs = {}
        for r in rows:
            if r['quality'] == quality:
                pairs.setdefault(r['pair'], {})[r['arm']] = r
        assert all(set(p) == {'baseline', 'before_review', 'candidate'} for p in pairs.values())
        pairs = [p for p in pairs.values() if all(r['status'] == 'accepted' for r in p.values())]
        assert len(pairs) >= 5
        for key, label in [('art_wall_ns_p50', 'art callback p50'), ('art_wall_ns_p95', 'art callback p95'),
                           ('art_redraw_ns_p50', 'art preparation p50'), ('art_submit_ns_p50', 'art CPU submit p50'),
                           ('art_gpu_ns_p50', 'art GPU p50'), ('art_cpu_ns_p50', 'art CPU stages p50'),
                           ('all_wall_ns_p50', '10 s callback p50'), ('all_wall_ns_p95', '10 s callback p95')]:
            v = {a: statistics.median(float(p[a][key]) for p in pairs)/1e6 for a in ['baseline', 'before_review', 'candidate']}
            d = [100*(float(p['candidate'][key])/float(p['before_review'][key])-1) for p in pairs]
            lines.append(f'| {quality} / {label} | {v["baseline"]:.3f} | {v["before_review"]:.3f} | {v["candidate"]:.3f} | {min(d):+.1f} to {max(d):+.1f}% |')
        selected = [r for p in pairs for r in p.values()]
        notes.append(f'{quality}: {len(pairs)} complete triples; load {min(float(r["load_min"]) for r in selected):.1f} to '
                     f'{max(float(r["load_max"]) for r in selected):.1f}%; cargo/test/archive samples '
                     + '/'.join(str(sum(int(r[k]) for r in selected)) for k in ['cargo_samples', 'test_samples', 'archive_samples']) + '.')
        for key, label in [('art_wall_ns_p50', 'callback'), ('art_submit_ns_p50', 'CPU submit')]:
            delta = [(float(p['candidate'][key])-float(p['before_review'][key]))/1e6 for p in pairs]
            notes.append(f'{quality} {label}: paired final-minus-before-review median {statistics.median(delta):+.3f} ms; '
                         f'range {min(delta):+.3f} to {max(delta):+.3f} ms.')
    return '\n'.join(lines) + '\n\n' + '\n\n'.join(notes) + f'\n\nReview-fix runs retained: {len(rows)}; excluded: {sum(r["status"] == "excluded" for r in rows)}.'


def menu_tables():
    identity = json.loads((ROOT / 'identities.json').read_text(encoding='utf-8'))['review_fixes']
    with (ROOT / 'menu-runs.csv').open(newline='', encoding='utf-8') as f:
        rows = list(csv.DictReader(f))
    assert len(rows) == 4
    lines = ['| Single diagnostic / fade callbacks | Frame median ms | Frame p95 ms | Composition median ms | Spinner-tail median ms |',
             '| --- | ---: | ---: | ---: | ---: |']
    for r in rows:
        assert (r['callbacks'], r['fade_frames'], r['spinner_tail_frames']) == ('600', '31', '11')
        assert (r['first_model_ms'], r['last_model_ms']) == ('1000', '1500')
        assert float(r['first_fully_visible_ms']) == 1516.6666666666667
        assert float(r['first_no_menu_ms']) == 1700
        assert r['binary_sha256'] == identity['binaries']['candidate'] and r['compile_samples'] == '0'
        name = r['quality'] + (' menu composed' if r['composed'] == 'True' else ' direct world')
        lines.append(f'| {name} | {float(r["wall_p50_ms"]):.3f} | {float(r["wall_p95_ms"]):.3f} | '
                     f'{float(r["compose_p50_ms"]):.3f} | {float(r["spinner_tail_wall_p50_ms"]):.3f} |')
    return '\n'.join(lines)


def zoom_tables():
    identity = json.loads((ROOT / 'identities.json').read_text(encoding='utf-8'))['review_fixes']
    with (ROOT / 'zoom-runs.csv').open(newline='', encoding='utf-8') as f:
        rows = list(csv.DictReader(f))
    assert len(rows) == 4
    lines = ['| Single diagnostic / zoom-out notches | Visible pellet median | Art callback median ms | Art callback p95 ms | Retained art callbacks |',
             '| --- | ---: | ---: | ---: | ---: |']
    for r in rows:
        assert (r['callbacks'], r['skips'], r['art_batch_frames'], r['compile_samples']) == ('600', '0', '0', '0')
        assert r['binary_sha256'] == identity['binaries']['candidate']
        lines.append(f'| {r["quality"]} / {abs(int(r["notches"]))} | {float(r["visible_p50"]):.1f} | '
                     f'{float(r["wall_ns_p50_ms"]):.3f} | {float(r["wall_ns_p95_ms"]):.3f} | 0 / 300 |')
    return '\n'.join(lines)


def serial_tables():
    identity = json.loads((ROOT / 'serial-identities.json').read_text(encoding='utf-8'))
    for name, expected in identity['rejected_patches'].items():
        assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected
    assert len(identity['image_parity']) == 10
    for case, arms in identity['image_parity'].items():
        assert set(arms) == {'baseline', 'candidate'}
        for arm, record in arms.items():
            assert (record['callbacks'], record['skips']) == (600, 0)
            assert record['messages'] == (8 if case.startswith('rectangle/') else 29044)
            assert record['binary_sha256'] == identity['image_binaries'][arm]
            assert len(record['image_sequence_sha256']) == 64
        assert arms['baseline']['image_sequence_sha256'] == arms['candidate']['image_sequence_sha256']
    with (ROOT / 'serial-runs.csv').open(newline='', encoding='utf-8') as stream:
        rows = list(csv.DictReader(stream))
    assert len({r['label'] for r in rows}) == len(rows)
    for r in rows:
        assert r['label'] == f'serial-{r["candidate"]}-{r["round"]}-{r["quality"]}-{r["arm"]}-{r["adapter"]}-{r["physical"]}'
        assert (r['callbacks'], r['messages'], r['skips']) == ('600', '29044', '0')
        role = 'always' if r['candidate'] == 'packed-pilot' and r['arm'] == 'baseline' else r['arm']
        assert r['binary_sha256'] == identity['timing_binaries'][role]
        assert r['status'] == ('compile_overlap' if int(r['compile_samples']) else 'accepted')
    lines = ['| Adapter / physical / quality | Complete pairs | Art p50 before -> after ms | Art p95 before -> after ms | CPU stages before -> after ms | Pellet math before -> after ms | GPU before -> after ms |',
             '| --- | ---: | ---: | ---: | ---: | ---: | ---: |']
    notes = []
    configs = [('always', 'rtx', '3840x2160')] + [('always-guard', 'igpu', size) for size in ['3840x2160', '1920x1080']]
    metrics = ['wall_ns_p50_ms', 'wall_ns_p95_ms', 'cpu_ns_p50_ms', 'pellet_math_p50_ms', 'gpu_ns_p50_ms']
    for candidate, adapter, size in configs:
        for quality in ['original', 'native']:
            groups = {}
            for r in rows:
                if (r['candidate'], r['adapter'], r['physical'], r['quality']) == (candidate, adapter, size, quality):
                    pair = groups.setdefault(r['round'], {})
                    assert r['arm'] not in pair
                    pair[r['arm']] = r
            assert all(set(p) == {'baseline', 'always'} for p in groups.values())
            for pair in groups.values():
                for key in ['numeric_state_sha256', 'control_digest']:
                    assert pair['baseline'][key] == pair['always'][key]
            pairs = [p for p in groups.values() if all(r['status'] == 'accepted' for r in p.values())]
            assert len(pairs) >= 5
            values = [f'{statistics.median(float(p["baseline"][key]) for p in pairs):.3f} -> '
                      f'{statistics.median(float(p["always"][key]) for p in pairs):.3f}' for key in metrics]
            name = f'{adapter} / {size} / {quality}'
            lines.append(f'| {name} | {len(pairs)} | ' + ' | '.join(values) + ' |')
            selected = [r for p in pairs for r in p.values()]
            delta = [float(p['always']['wall_ns_p50_ms']) - float(p['baseline']['wall_ns_p50_ms']) for p in pairs]
            spread = '; '.join(f'{arm} p50 range {min(float(p[arm]["wall_ns_p50_ms"]) for p in pairs):.3f} to '
                               f'{max(float(p[arm]["wall_ns_p50_ms"]) for p in pairs):.3f} ms' for arm in ['baseline', 'always'])
            notes.append(f'{name}: paired p50 difference median {statistics.median(delta):+.3f} ms, '
                         f'range {min(delta):+.3f} to {max(delta):+.3f} ms; {spread}; '
                         f'load {min(float(r["load_min"]) for r in selected):.1f} to {max(float(r["load_max"]) for r in selected):.1f}%; '
                         f'observed cargo/test samples {sum(int(r["cargo_samples"]) for r in selected)}/{sum(int(r["test_samples"]) for r in selected)}.')
    return '\n'.join(lines) + '\n\n' + '\n\n'.join(notes) + (
        f'\n\nAll serial-probe runs retained: {len(rows)}; compile-overlap exclusions: '
        f'{sum(r["status"] == "compile_overlap" for r in rows)}. Incomplete accepted pairs are retained but excluded from aggregates.')


if __name__ == '__main__':
    output = tables()
    print(output)
    print(diagnostics())
    print(review_tables())
    print(menu_tables())
    print(zoom_tables())
    print(serial_tables())
    if '--check' in sys.argv:
        report = (ROOT / 'REPORT.md').read_text(encoding='utf-8')
        assert report.split('<!-- BEGIN RESULTS -->\n')[1].split('\n<!-- END RESULTS -->')[0] == output
        assert report.split('<!-- BEGIN DIAGNOSTICS -->\n')[1].split('\n<!-- END DIAGNOSTICS -->')[0] == diagnostics()
        assert report.split('<!-- BEGIN REVIEW RESULTS -->\n')[1].split('\n<!-- END REVIEW RESULTS -->')[0] == review_tables()
        assert report.split('<!-- BEGIN MENU RESULTS -->\n')[1].split('\n<!-- END MENU RESULTS -->')[0] == menu_tables()
        assert report.split('<!-- BEGIN ZOOM RESULTS -->\n')[1].split('\n<!-- END ZOOM RESULTS -->')[0] == zoom_tables()
        assert report.split('<!-- BEGIN SERIAL RESULTS -->\n')[1].split('\n<!-- END SERIAL RESULTS -->')[0] == serial_tables()
