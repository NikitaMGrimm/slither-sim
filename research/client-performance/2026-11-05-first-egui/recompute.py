"""Recompute selected first-egui results; --check verifies the retained report.

Uses only the selected, valid runs.csv and image-controls.json. Raw journals,
rejected builds and invalid timing attempts remain in the study's local scratch.
"""
import csv
import json
from pathlib import Path
import statistics
import sys

HERE = Path(__file__).resolve().parent


def cell(values):
    return f'{statistics.median(values):.2f} [{min(values):.2f}, {max(values):.2f}]'


def tables():
    with (HERE / 'runs.csv').open(newline='', encoding='utf-8') as file:
        rows = list(csv.DictReader(file))
    pairs = {}
    for row in rows:
        key = row['adapter'], row['screen'], int(row['pair'])
        arms = pairs.setdefault(key, {})
        assert row['arm'] not in arms, f'duplicate {key}'
        arms[row['arm']] = row
    assert len(rows) == 80 and len(pairs) == 40
    assert all(set(arms) == {'baseline', 'candidate'} for arms in pairs.values())
    timing = [
        '| Adapter / menu | Pairs | First paint before (ms) | First paint after (ms) | Warm paint after (ms) |',
        '| --- | ---: | ---: | ---: | ---: |',
    ]
    startup = [
        '| Adapter | Runs per arm | GPU initialization before → after (ms) | First complete picture before → after (ms) |',
        '| --- | ---: | ---: | ---: |',
    ]
    for adapter in ['rtx-vulkan', 'amd-dx12']:
        for screen in ['login', 'settings', 'ingame', 'picker']:
            group = [arms for (a, s, _), arms in pairs.items() if (a, s) == (adapter, screen)]
            assert len(group) == 5
            before = [float(p['baseline']['paint_0_ms']) for p in group]
            after = [float(p['candidate']['paint_0_ms']) for p in group]
            warm = [statistics.median(float(p['candidate'][f'paint_{i}_ms']) for i in range(2, 6)) for p in group]
            timing.append(f'| {adapter} / {screen} | 5 | {cell(before)} | {cell(after)} | {cell(warm)} |')
        fields = []
        for field in ['gpu_ms', 'first_picture_ms']:
            values = [[float(r[field]) for r in rows if (r['adapter'], r['arm']) == (adapter, arm)] for arm in ['baseline', 'candidate']]
            assert all(len(v) == 20 for v in values)
            fields.append(f'{cell(values[0])} → {cell(values[1])}')
        startup.append(f'| {adapter} | 20 | ' + ' | '.join(fields) + ' |')
    controls = json.loads((HERE / 'image-controls.json').read_text(encoding='utf-8'))
    assert len(controls) == 138
    assert len({c['specimen'] for c in controls}) == len(controls)
    for control in controls:
        assert control['before_png_sha256'] == control['after_png_sha256']
        assert len(bytes.fromhex(control['before_png_sha256'])) == 32
        assert control['rgba_bytes'] > 0
    loads = [float(r['load_p50']) for r in rows]
    summary = (f'80 valid runs; 40 pairs; no load-based exclusions. '
               f'Run median machine CPU load: {min(loads):.1f}–{max(loads):.1f}%. '
               f'Compiler/linker observed in {sum(int(r["compiler_seen"]) for r in rows)} runs. '
               '138 image pairs are byte-identical.')
    return {'SUMMARY': summary, 'TIMING': '\n'.join(timing), 'STARTUP': '\n'.join(startup)}


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
        print('REPORT.md matches selected timing and image controls')
    else:
        (HERE / 'REPORT.md').write_text(report, encoding='utf-8', newline='\n')


if __name__ == '__main__':
    main()
