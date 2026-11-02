"""Validate the complete retained dataset and reproduce REPORT.md numeric tables."""
import argparse
from collections import Counter
import csv
import json
import math
from pathlib import Path
import re
from statistics import median

from reduce import STAGES, VARIANTS, require

ROOT = Path(__file__).parent
SERIES = ('main-msaa', 'r8-trial', 'rgb-trial', 'noop-trial', 'combined-trial',
          'selected-native', 'selected-original')


def rows(name, root=ROOT):
    with (root / name).open(encoding='utf-8', newline='') as stream:
        result = list(csv.DictReader(stream))
    require(result, f'empty {name}')
    return result


def finite(value):
    value = float(value)
    require(math.isfinite(value) and value >= 0, 'negative/nonfinite metric')
    return value


def complete(data, labels, expected, id_fn):
    require(Counter(tuple(r[k] for k in labels) for r in data) == Counter(expected),
            'incomplete/duplicate/mislabeled cells')
    ids = [r['run_id'] for r in data]
    require(len(set(ids)) == len(ids), 'duplicate run id')
    for r in data:
        require(r['run_id'] == id_fn(r), 'run id differs from labels')
        for k, v in r.items():
            if k.endswith('_sha256'):
                require(re.fullmatch('[0-9a-f]{64}', v), 'invalid hash')
            if k.endswith(('_ms', '_peak')) or k in ('load_min', 'load_max', 'relative_start_s'):
                finite(v)


def history(data, label):
    expected = [(str(p), z, s) for p in range(1, 6) for z in ('0', '-120')
                for s in (('before', 'after') if p % 2 else ('after', 'before'))]
    complete(data, ('pair', 'zoom', 'side'), expected,
             lambda r: f"{label}-{r['pair']}-{r['zoom']}-{r['side']}")
    require([(r['pair'], r['zoom'], r['side']) for r in data] == expected, 'not alternating')
    for i, r in enumerate(data, 1):
        require(int(r['launch_index']) == i and int(r['callbacks']) == 181
                and int(r['samples']) == 141 and int(r['exit']) == 0, 'incomplete history')
        require(int(r['world_samples']) == (1 if label == 'main-msaa' and r['side'] == 'after' else 4),
                'wrong sample count')
        require(float(r['load_min']) <= float(r['load_max']) <= 100, 'invalid host load')
        for metric in ('gpu', 'sync', 'submit'):
            require(float(r[metric+'_median_ms']) <= float(r[metric+'_p95_ms']), 'quantiles out of order')
    require([float(r['relative_start_s']) for r in data] == sorted(float(r['relative_start_s']) for r in data), 'nonmonotonic launches')
    for side in ('before', 'after'):
        require(len({r['binary_sha256'] for r in data if r['side'] == side}) == 1, 'mixed binaries')
    for z in ('0', '-120'):
        for p in range(1, 6):
            a, b = [r for r in data if r['zoom'] == z and r['pair'] == str(p)]
            for metric in ('draws', 'body_quads', 'outline_quads', 'glow_quads', 'other_quads'):
                require(a[metric + '_peak'] == b[metric + '_peak'], 'CPU picture counts changed')


def profile(data):
    expected = [(z, str(f), str(p), d, side) for z in ('0', '-120') for f in (60, 120, 180)
                for p in range(1, 6) for d in VARIANTS for side in ('before', 'after')]
    complete(data, ('zoom', 'frame', 'pair', 'diagnostic', 'side'), expected,
             lambda r: 'profile-' + '-'.join(r[k] for k in ('zoom', 'frame', 'pair', 'diagnostic', 'side')))
    require(all(int(r['samples']) == 10 for r in data), 'incomplete profile samples')
    require(len({r['binary_sha256'] for r in data}) == 1, 'mixed profile binaries')
    for r in data:
        require(sum(float(r[s + '_mean_ms']) for s in STAGES) > 0, 'empty pass profile')


def subset(data, **labels):
    selected = [r for r in data if all(r[k] == str(v) for k, v in labels.items())]
    require(selected, f'missing {labels}')
    return selected


def m(data, metric):
    return median(float(r[metric]) for r in data)


def timing_table(datasets, labels):
    out = ['| Series / view | Before median / p95 ms | After median / p95 ms | Per-run p95 ranges before / after ms |',
           '| --- | ---: | ---: | --- |']
    for label in labels:
        for zoom, view in (('0', 'default'), ('-120', 'whole-map')):
            a = subset(datasets[label], zoom=zoom, side='before')
            b = subset(datasets[label], zoom=zoom, side='after')
            pair = lambda d: f"{m(d, 'gpu_median_ms'):.4f} / {m(d, 'gpu_p95_ms'):.4f}"
            span = lambda d: f"{min(float(r['gpu_p95_ms']) for r in d):.4f}–{max(float(r['gpu_p95_ms']) for r in d):.4f}"
            out.append(f'| {label} / {view} | {pair(a)} | {pair(b)} | {span(a)} / {span(b)} |')
    return '\n'.join(out)


def tables(root=ROOT):
    p = rows('profile-runs.csv', root)
    profile(p)
    coverage = rows('coverage.csv', root)
    require(Counter((r['zoom'], r['frame']) for r in coverage)
            == Counter((z, str(f)) for z in ('0', '-120') for f in (60, 120, 180)), 'incomplete coverage')
    datasets = {label: rows(label + '-runs.csv', root) for label in SERIES}
    for label, data in datasets.items():
        history(data, label)
    source = json.loads((root/'sources.json').read_text(encoding='utf-8'))
    require({r['binary_sha256'] for r in p} == {source['main-msaa']['before']['binary_sha256']}, 'unbound profile binary')
    for label, data in datasets.items():
        for side in ('before', 'after'):
            require({r['binary_sha256'] for r in subset(data, side=side)} == {source[label][side]['binary_sha256']},
                    'unbound binary')
    stages = subset(p, zoom=0, frame=120, diagnostic='resolve-only', side='before')
    out = ['| Pass | GPU ms |', '| --- | ---: |']
    for stage in sorted(STAGES, key=lambda s: -m(stages, s + '_mean_ms')):
        value = m(stages, stage + '_mean_ms')
        if value:
            out.append(f'| {stage} | {value:.4f} |')
    diagnostics = ['| Diagnostic | Before / after GPU median ms |', '| --- | ---: |']
    for d in VARIANTS:
        a = subset(p, zoom=0, frame=120, diagnostic=d, side='before')
        b = subset(p, zoom=0, frame=120, diagnostic=d, side='after')
        diagnostics.append(f"| {d} | {m(a, 'gpu_median_ms'):.4f} / {m(b, 'gpu_median_ms'):.4f} |")
    resolve = subset(p, zoom=0, frame=120, diagnostic='resolve-only', side='after')
    diagnostics.append(f"| Isolated resolve pass (after arm) | {m(resolve, 'msaa-resolve-only_mean_ms'):.4f} |")
    loads = ['| Series | Host CPU load range % | Concurrent processes range | Compiler overlap runs |', '| --- | ---: | ---: | ---: |']
    for label, data in datasets.items():
        loads.append(f"| {label} | {min(float(r['load_min']) for r in data):.0f}–{max(float(r['load_max']) for r in data):.0f} | {min(int(r['processes_max']) for r in data)}–{max(int(r['processes_max']) for r in data)} | {sum(int(r['compile_overlap']) for r in data)} |")
    encoding = ['| Policy / view | Before / after CPU encode+submit median ms | Before / after GPU draw-call median |', '| --- | ---: | ---: |']
    for label in SERIES[5:]:
        for z in ('0','-120'):
            a,b=[subset(datasets[label],zoom=z,side=s) for s in ('before','after')]
            encoding.append(f"| {label} / {'default' if z=='0' else 'whole-map'} | {m(a,'submit_median_ms'):.4f} / {m(b,'submit_median_ms'):.4f} | {m(a,'gpu_calls_median'):.0f} / {m(b,'gpu_calls_median'):.0f} |")
    sparse = rows('sparse-runs.csv', root)
    expected = [(str(p), side, size, case) for p in range(1, 6) for side in ('before', 'after')
                for size in ('1920-original', '1920-native', '3840-native') for case in ('body', 'low-quality', 'one-glow')]
    complete(sparse, ('pair', 'side', 'size', 'case'), expected,
             lambda r: 'sparse-' + '-'.join(r[k] for k in ('pair', 'side', 'size', 'case')))
    require(all(r['samples'] == '30' and r['passes_min'] == r['passes_max'] == ('2' if r['size']=='1920-original' else '1') for r in sparse), 'sparse extra pass/incomplete samples')
    order = [(str(p), side) for p in range(1,6) for side in (('before','after') if p % 2 else ('after','before'))]
    for row in sparse:
        require(int(row['launch_index']) == order.index((row['pair'],row['side']))+1, 'nonalternating sparse launches')
        require(row['binary_sha256'] == source['sparse_binary_sha256'][row['side']], 'unbound sparse binary')
    small = ['| Sparse size / case | Before median / p95 ms | After median / p95 ms |', '| --- | ---: | ---: |']
    for size in ('1920-original', '1920-native', '3840-native'):
        for case in ('body', 'low-quality', 'one-glow'):
            arms = [subset(sparse, size=size, case=case, side=side) for side in ('before', 'after')]
            value = lambda d: f"{m(d,'gpu_median_ms'):.4f} / {m(d,'gpu_p95_ms'):.4f}"
            small.append(f'| {size} / {case} | {value(arms[0])} | {value(arms[1])} |')
    parity = rows('parity.csv', root)
    require(Counter((r['policy'],r['zoom']) for r in parity) == Counter((p,z) for p in ('native','original') for z in ('0','-120')), 'incomplete parity cells')
    pixels = rows('pixels.csv', root)
    require(Counter((r['policy'],r['zoom'],r['frame']) for r in pixels) == Counter((p,z,str(f)) for p in ('native','original') for z in ('0','-120') for f in (60,120,180)), 'incomplete snapshot cells')
    picture = ['| Policy / view | Frames | Changed frames / RGBA bytes | Max byte difference | Alpha changed bytes |', '| --- | ---: | ---: | ---: | ---: |']
    for r in parity:
        require(r['frames'] == '181' and r['bytes_per_frame'] == str(1920*1080*4), 'incomplete pixel history')
        require(int(r['max_byte_difference']) <= 2 and int(r['alpha_changed_bytes']) == 0, 'pixel budget exceeded')
        require((r['before_rgba_sha256'] == r['after_rgba_sha256']) == (int(r['changed_bytes']) == 0), 'parity digest disagrees')
        for side in ('before','after'):
            require(r[side+'_binary_sha256'] == source['selected-native'][side]['binary_sha256'], 'unbound parity binary')
        picture.append(f"| {r['policy']} / {'default' if r['zoom']=='0' else 'whole-map'} | {r['frames']} | {r['changed_frames']} / {r['changed_bytes']} | {r['max_byte_difference']} | {r['alpha_changed_bytes']} |")
    frozen = rows('frozen-runs.csv', root)
    complete(frozen, ('pair','side','frame'), [(str(p),s,str(f)) for p in range(1,6) for s in ('before','after') for f in (60,120,180)],
             lambda r: 'frozen-'+'-'.join(r[k] for k in ('pair','side','frame')))
    require(all(r['samples']=='10' and r['binary_sha256']==source['selected-native'][r['side']]['binary_sha256'] for r in frozen),'incomplete/unbound frozen profile')
    after = subset(frozen,frame=120,side='after')
    final_profile=['| Selected default picture 120 pass | GPU ms |','| --- | ---: |']
    for stage in sorted(STAGES,key=lambda s:-m(after,s+'_mean_ms')):
        value=m(after,stage+'_mean_ms')
        if value:
            final_profile.append(f'| {stage} | {value:.4f} |')
    return {'profile': '\n'.join(out), 'diagnostics': '\n'.join(diagnostics),
            'experiments': timing_table(datasets, SERIES[:5]),
            'headline': timing_table(datasets, SERIES[5:]), 'load': '\n'.join(loads),
            'sparse': '\n'.join(small), 'parity': '\n'.join(picture), 'encoding': '\n'.join(encoding),
            'remaining': '\n'.join(final_profile)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    report = (ROOT/'REPORT.md').read_text(encoding='utf-8')
    for name, table in tables().items():
        start, end = f'<!-- {name}:start -->', f'<!-- {name}:end -->'
        require(report.count(start) == report.count(end) == 1, 'missing report table markers')
        old = report.split(start)[1].split(end)[0]
        new = '\n' + table + '\n'
        if args.write:
            report = report.replace(start + old + end, start + new + end)
        elif args.check:
            require(old == new, f'{name} table differs')
        else:
            print(table + '\n')
    if args.write:
        (ROOT/'REPORT.md').write_text(report, encoding='utf-8', newline='\n')
    print('Complete cell sets, sample counts, run identities, source bindings and tables validated.')


if __name__ == '__main__':
    main()
