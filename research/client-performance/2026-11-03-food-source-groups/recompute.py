"""Recompute bounded tables with stdlib Python; no recording or GPU required."""
import csv
import json
from pathlib import Path
import re
import statistics as st
import sys

ROOT = Path(__file__).resolve().parent
ARMS = ('base', 'candidate')
CONFIGS = [('rtx', '3840x2160'), ('igpu', '1920x1080'), ('igpu', '3840x2160')]
PARITY_KEYS = {
    f'{scene}-{adapter}-{size}-{quality}'
    for scene in ['replay', 'rectangle']
    for adapter, size in CONFIGS
    if scene != 'rectangle' or size == '3840x2160'
    for quality in ['original', 'native']
}


def rows(name):
    with (ROOT/name).open(newline='', encoding='utf-8') as f:
        return list(csv.DictReader(f))


def median(rs, key):
    return st.median(float(r[key]) for r in rs)


def shape(value, length):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{'+str(length)+'}', value) is not None


def validate(runs, parity, ids):
    assert ids['base']['binary_sha256'] != ids['candidate']['binary_sha256']
    assert ids['base']['driver_sha256'] == ids['candidate']['driver_sha256']
    assert shape(ids['replay_input']['sha256'], 64)
    pairs = {}
    for r in runs:
        assert r['arm'] in ARMS
        assert r['binary_sha256'] == ids[r['arm']]['binary_sha256']
        assert (int(r['callbacks']), int(r['messages']), int(r['skips'])) == (600, 29044, 0)
        assert r['canonical_digest'] == 'absent'
        assert shape(r['sent_digest'], 16)
        assert all(shape(r[k], 64) for k in ['binary_sha256', 'frames_sha256', 'load_sha256'])
        assert r['status'] in ['accepted', 'compile_overlap']
        assert (r['status'] == 'accepted') == (int(r['compile_samples']) == 0)
        assert r['reason'] == ('none' if r['status'] == 'accepted' else 'observed rustc/link overlap')
        assert 0 <= float(r['load_min']) <= float(r['load_p50']) <= float(r['load_max']) <= 100
        assert int(r['source_rebuild_events']) == 65
        key = tuple(r[k] for k in ['round', 'adapter', 'physical', 'quality'])
        pair = pairs.setdefault(key, {})
        assert r['arm'] not in pair
        pair[r['arm']] = r
    for pair in pairs.values():
        assert set(pair) == set(ARMS)
        accepted = all(r['status'] == 'accepted' for r in pair.values())
        assert all((r['paired'] == 'True') == accepted for r in pair.values())
        assert len({r['sent_digest'] for r in pair.values()}) == 1
    assert set(parity) == PARITY_KEYS
    for arms in parity.values():
        assert set(arms) == set(ARMS)
        for arm, r in arms.items():
            assert (r['callbacks'], r['skips']) == (600, 0)
            assert r['binary_sha256'] == ids[arm]['binary_sha256']
            assert shape(r['image_sequence_sha256'], 64)
        assert arms['base']['image_sequence_sha256'] == arms['candidate']['image_sequence_sha256']
    return pairs


def tables(runs, tails):
    selected = [r for r in runs if r['paired'] == 'True']
    def group(adapter, size, quality):
        return [r for r in selected if (r['adapter'], r['physical'], r['quality']) == (adapter, size, quality)]
    def cell(rs, a='wall_ns_p50_ms', b='wall_ns_p95_ms'):
        return f'{median(rs,a):.3f}/{median(rs,b):.3f}'
    text = ['| Adapter / size / quality | Pairs | Base art median/p95 ms | Cache art median/p95 ms | Paired p95 reduction median [range] ms |', '| --- | ---: | ---: | ---: | ---: |']
    for adapter, size in CONFIGS:
        for quality in ['original', 'native']:
            rs = group(adapter, size, quality)
            parts = [[r for r in rs if r['arm'] == arm] for arm in ARMS]
            assert len(parts[0]) == len(parts[1]) >= (5 if adapter == 'rtx' else 3)
            base = {r['round']: r for r in parts[0]}
            delta = [float(base[r['round']]['wall_ns_p95_ms'])-float(r['wall_ns_p95_ms']) for r in parts[1]]
            text.append(f'| {adapter} / {size} / {quality} | {len(parts[0])} | '+ ' | '.join(map(cell, parts)) + f' | {st.median(delta):+.3f} [{min(delta):+.3f}, {max(delta):+.3f}] |')
    text += ['', '| RTX 4K / quality / arm | Art median range ms | Rebuild median/p95 ms | Whole median/p95 ms | Art GPU median ms | Process Rust requests / GB | Load median [range] % |', '| --- | ---: | ---: | ---: | ---: | ---: | ---: |']
    for quality in ['original', 'native']:
        for arm in ARMS:
            rs = [r for r in group('rtx', '3840x2160', quality) if r['arm'] == arm]
            values = [float(r['wall_ns_p50_ms']) for r in rs]
            text.append(f'| {quality} / {arm} | {min(values):.3f}-{max(values):.3f} | '+cell(rs,'source_rebuild_p50_ms','source_rebuild_p95_ms')+' | '+cell(rs,'whole_p50_ms','whole_p95_ms')+f" | {median(rs,'gpu_ns_p50_ms'):.3f} | {median(rs,'whole_process_requests'):.0f} / {median(rs,'whole_process_bytes')/1e9:.3f} | {median(rs,'load_p50'):.1f} [{min(float(r['load_min']) for r in rs):.1f}, {max(float(r['load_max']) for r in rs):.1f}] |")
    text += ['', '| Candidate slowest 16 art callbacks, median across runs | Original ms | Native ms |', '| --- | ---: | ---: |']
    for key in ['wall_ns','ingress_ns','food_advance_ns','food_clone_ns','pellet_sources_ns','pellet_prepare_ns','pellet_math_ns','submit_ns','submit_without_prepare_ns','snapshot_ns','prey_ns','minimap_ns','resources_ns','egui_cpu_ns','compose_ns','gpu_ns']:
        text.append('| '+key+' | '+' | '.join(f"{median([r for r in tails if r['arm']=='candidate' and r['quality']==q],key):.4f}" for q in ['original','native'])+' |')
    text += ['', '| RTX 4K / quality / arm | Measured art p95 ms | Arithmetic zero-source art p95 ms |', '| --- | ---: | ---: |']
    for quality in ['original','native']:
        for arm in ARMS:
            rs=[r for r in group('rtx','3840x2160',quality) if r['arm']==arm]
            text.append(f"| {quality} / {arm} | {median(rs,'wall_ns_p95_ms'):.3f} | {median(rs,'zero_source_art_p95_ms'):.3f} |")
    return '\n'.join(text)


def main():
    runs=rows('runs.csv')
    validate(runs,json.loads((ROOT/'parity.json').read_text(encoding='utf-8')),json.loads((ROOT/'identities.json').read_text(encoding='utf-8')))
    text=tables(runs,rows('tail-stages.csv'))
    if '--check' in sys.argv:
        assert text in (ROOT/'REPORT.md').read_text(encoding='utf-8'), 'report tables differ'
        print('Run selection, binary/driver identities, 6000 exact image pairs and tables verified')
    else:
        print(text)


if __name__=='__main__':
    main()
