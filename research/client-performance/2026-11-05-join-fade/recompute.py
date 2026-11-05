"""Check selected image identities and recompute paired callback summaries.

usage: python -B recompute.py [--check]
Raw journals, recordings, archives and executables remain outside Git.
"""
from pathlib import Path
import csv
import hashlib
import json
import re
import statistics
import sys

sys.stdout.reconfigure(encoding='utf-8')

HERE = Path(__file__).resolve().parent
METRICS = {
    'fade_wall_median_ms': 'Tail callback p50',
    'fade_wall_p95_ms': 'Tail callback p95',
    'warm_tail_wall_p95_ms': 'Post-cold tail p95',
    'gameplay_wall_p95_ms': 'Gameplay callback p95',
}


def verify_images():
    ids = json.loads((HERE / 'identities.json').read_text(encoding='utf-8'))
    images = json.loads((HERE / 'image-controls.json').read_text(encoding='utf-8'))
    assert set(images) == {f'pixels/{a}/{q}/{v}' for a in ['rtx','igpu']
                           for q in ['native','original'] for v in ['stock','far','whole']} | {
                               f'canonical/{v}' for v in ['stock','far','whole']}
    for key, control in images.items():
        arms = control['arms']
        assert set(arms) == {'baseline', 'balanced2'}
        for arm, values in arms.items():
            assert values['binary_sha256'] == ids['arms'][arm]['binary_sha256']
            assert values['driver_sha256'] == ids['driver_sha256']
        fields = ['digest', 'census_sha256']
        if key.startswith('pixels/'):
            assert control['callbacks'] == 600 and control['skips'] == 0
            fields += ['image_sequence_sha256', 'join_tail_rgba_sha256',
                       'join_tail_sequence_sha256', 'crossfade_callbacks']
            for values in arms.values():
                hashes = values['join_tail_rgba_sha256']
                assert len(hashes) == 42
                assert hashlib.sha256('\n'.join(hashes).encode()).hexdigest() == values['join_tail_sequence_sha256']
                for frame, row in enumerate(hashes, 60):
                    index, digest = row.split(',')
                    assert int(index) == frame and re.fullmatch('[0-9a-f]{64}', digest)
                assert values['crossfade_callbacks']
                assert set(values['crossfade_callbacks']) <= set(range(60, 102))
                assert values['distinct_images'] > 1
        else:
            fields += ['quad_digest', 'canonical_quads']
            assert all(v['canonical_active'] == 'true' and v['canonical_quads'] > 0 for v in arms.values())
        for field in fields:
            assert arms['baseline'][field] == arms['balanced2'][field], (key, field)


def tables():
    ids = json.loads((HERE / 'identities.json').read_text(encoding='utf-8'))
    with (HERE / 'runs.csv').open(newline='', encoding='utf-8') as f:
        rows = list(csv.DictReader(f))
    pairs = {}
    for r in rows:
        assert int(r['callbacks']) == 600 and int(r['skips']) == 0
        assert r['binary_sha256'] == ids['arms'][r['arm']]['binary_sha256']
        assert r['status'] in {'accepted', 'compile_overlap'}
        assert (r['status'] == 'accepted') == (int(r['compile_samples']) == 0)
        key = (r['adapter'], r['view'], r['quality'], int(r['pair']))
        arms = pairs.setdefault(key, {})
        assert r['arm'] not in arms, key
        arms[r['arm']] = r
    groups, excluded = {}, 0
    for (adapter, view, quality, _), arms in sorted(pairs.items()):
        assert set(arms) == {'baseline', 'balanced2'}
        if all(r['status'] == 'accepted' for r in arms.values()):
            groups.setdefault((adapter, view, quality), []).append(arms)
        else:
            excluded += 1
    assert set(groups) == {('rtx','stock','native'), ('rtx','far','native'),
                           ('rtx','far','original'), ('rtx','whole','native'),
                           ('igpu','far','native')}
    assert all(len(group) >= 5 for group in groups.values())
    lines = ['| Adapter / view / quality | Pairs | ' + ' | '.join(f'{v} before → after (ms)' for v in METRICS.values()) + ' |',
             '| --- | ---: | ' + ' | '.join('---:' for _ in METRICS) + ' |']
    for config, group in groups.items():
        cells = []
        for metric in METRICS:
            arms = [[float(pair[arm][metric]) for pair in group] for arm in ['baseline', 'balanced2']]
            cells.append(' → '.join(f'{statistics.median(v):.1f} [{min(v):.1f}, {max(v):.1f}]' for v in arms))
        lines.append(f'| {" / ".join(config)} | {len(group)} | ' + ' | '.join(cells) + ' |')
    loads = [float(r['load_p50']) for group in groups.values() for pair in group for r in pair.values()]
    summary = (f'Runs: {len(rows)}; pairs: {len(pairs)}; pairs excluded for observed compiler overlap: {excluded}. '
               f'Accepted-pair run median machine load: {min(loads):.1f}-{max(loads):.1f}%.')
    group = groups[('rtx', 'far', 'native')]
    def median(arm, metric):
        return statistics.median(float(pair[arm][metric]) for pair in group)
    result = (f'RTX/Vulkan at factor 0.065, Native 4K: tail callback p95 falls from '
              f'{median("baseline", "fade_wall_p95_ms"):.1f} to {median("balanced2", "fade_wall_p95_ms"):.1f} ms '
              f'(medians across {len(group)} accepted pairs). After the first two cold callbacks, '
              f'tail p95 is {median("balanced2", "warm_tail_wall_p95_ms"):.1f} ms; '
              f'ordinary gameplay p95 is {median("balanced2", "gameplay_wall_p95_ms"):.1f} ms. '
              'The long expansion plateau is removed; the tail still has measurable submission overhead.')
    return {'RESULT': result, 'SUMMARY': summary, 'TIMING': '\n'.join(lines)}


def main():
    verify_images()
    report_path = HERE / 'REPORT.md'
    report = report_path.read_text(encoding='utf-8')
    for marker, text in tables().items():
        begin, end = f'<!-- {marker} -->\n', f'\n<!-- /{marker} -->'
        start = report.index(begin) + len(begin)
        stop = report.index(end, start)
        if '--check' in sys.argv:
            assert report[start:stop] == text, f'{marker} differs'
        else:
            report = report[:start] + text + report[stop:]
    if '--check' in sys.argv:
        print('Selected image identities and REPORT.md match the retained run summaries')
    else:
        report_path.write_text(report, encoding='utf-8', newline='\n')
        print(tables()['TIMING'])


if __name__ == '__main__':
    main()
