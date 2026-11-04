"""Stdlib reduction of bounded release rows; no Git, game input or current-file gate."""
from pathlib import Path
import argparse
import csv
import json
import math
import re
import statistics

ROOT = Path(__file__).resolve().parent
CONFIGS = {('rtx', v, q) for v in ['far', 'whole'] for q in ['native', 'original']} | {('igpu', 'far', 'native')}
IMAGES = {f'{a}/{q}/{v}' for a in ['rtx', 'igpu'] for q in ['native', 'original'] for v in (['stock', 'far', 'whole'] if a == 'rtx' else ['stock'])}


def digest(value, length=64):
    assert re.fullmatch(f'[0-9a-f]{{{length}}}', str(value)), 'missing or malformed digest'


def load():
    with (ROOT / 'runs.csv').open(newline='', encoding='utf-8') as stream:
        rows = list(csv.DictReader(stream))
    with (ROOT / 'pop.csv').open(newline='', encoding='utf-8') as stream:
        pop = list(csv.DictReader(stream))
    return rows, json.loads((ROOT / 'identities.json').read_text(encoding='utf-8')), json.loads((ROOT / 'image-controls.json').read_text(encoding='utf-8')), pop, json.loads((ROOT / 'capacity.json').read_text(encoding='utf-8'))


def validate(rows, ids, images, pop, capacity):
    assert set(ids) == {'base', 'look', 'candidate'}
    assert len({v['binary_sha256'] for v in ids.values()}) == 3
    assert len({v['driver_sha256'] for v in ids.values()}) == 1
    blocks = {}
    for row in rows:
        config = tuple(row[k] for k in ['adapter', 'view', 'quality'])
        assert config in CONFIGS
        arm = row['arm']; identity = ids[arm]
        for key in ['binary_sha256', 'driver_sha256', 'revision']:
            assert row[key] == identity[key], f'{key} arm mismatch'
        for key, length in [('binary_sha256', 64), ('driver_sha256', 64), ('selected_numeric_sha256', 64), ('command_digest', 16)]:
            digest(row[key], length)
        assert (int(row['callbacks']), int(row['messages']), int(row['skips'])) == (600, 29044, 0)
        assert row['status'] == ('compile_overlap' if int(row['compile_samples']) else 'accepted')
        for key in ['wall_ns_p50_ms', 'wall_ns_p95_ms', 'redraw_ns_p50_ms', 'submit_ns_p50_ms', 'gpu_ns_p50_ms', 'pellet_prepare_p50_ms', 'load_p50']:
            assert math.isfinite(float(row[key])) and float(row[key]) >= 0
        assert float(row['wall_ns_p95_ms']) >= float(row['wall_ns_p50_ms'])
        assert int(row['requested_allocation_bytes']) > 0
        assert int(row['census_callbacks']) == 300
        assert int(row['census_omissions_total']) >= 0
        if arm != 'base':
            assert int(row['census_omissions_total']) == 0
        block = blocks.setdefault((*config, int(row['round'])), {})
        assert arm not in block, 'duplicate run'
        block[arm] = row
    accepted = {}
    for key, block in blocks.items():
        expected = {'look', 'candidate'} if key[0] == 'igpu' else set(ids)
        assert set(block) == expected, 'incomplete arm block'
        for field in ['command_digest', 'selected_numeric_sha256']:
            assert len({r[field] for r in block.values()}) == 1, f'{field} parity failed'
        use = all(r['status'] == 'accepted' for r in block.values())
        assert all(r['block_accepted'] == str(use).lower() for r in block.values())
        if use:
            accepted.setdefault(key[:3], []).append(block)
    assert set(accepted) == CONFIGS
    assert all(len(blocks) == 5 for blocks in accepted.values()), 'need five complete matched blocks'
    assert set(images) == IMAGES, 'wrong image configurations'
    for key, controls in images.items():
        arms = {'base', 'candidate'} if key.endswith('/stock') else {'look', 'candidate'}
        assert set(controls) == arms
        assert len({r['binary_sha256'] for r in controls.values()}) == 2
        assert len({r['image_sequence_sha256'] for r in controls.values()}) == 1
        for arm, record in controls.items():
            assert record['binary_sha256'] == ids[arm]['binary_sha256']
            assert record['driver_sha256'] == ids[arm]['driver_sha256']
            assert record['callbacks'] == 600 and record['skips'] == 0
            assert record['unique_frame_hashes'] > 1, 'inactive image control'
            digest(record['image_sequence_sha256'])
    assert {(r['quality'], r['arm']) for r in pop} == {(q, a) for q in ['native', 'original'] for a in ['base', 'candidate']}
    for row in pop:
        assert abs(int(row['total_rgb']) - int(row['previous_total_rgb'])) == int(row['delta_light'])
        ratio = int(row['delta_light']) / max(1, int(row['previous_total_rgb']))
        assert math.isclose(ratio, float(row['relative_light_jump']), rel_tol=1e-12, abs_tol=1e-15)
    assert set(capacity) == {'native', 'original'}
    for value in capacity.values():
        assert value['callbacks'] == 600 and value['skips'] == 0
        assert value['visible_foods'] == 65535
        assert value['binary_sha256'] == ids['candidate']['binary_sha256']
        assert value['driver_sha256'] == ids['candidate']['driver_sha256']
        assert 0 < value['max_picture_quads'] <= 262144
    return accepted


def tables(rows, ids, images, pop, capacity):
    groups = validate(rows, ids, images, pop, capacity)
    timing = ['| Adapter / view / quality | Main ms p50/p95 | Approved look ms p50/p95 | Trimmed ms p50/p95 | Trim p50 paired range ms |', '| --- | ---: | ---: | ---: | ---: |']
    stages = ['| Adapter / view / quality | Pellet prepare: main / look / trimmed ms | Trimmed redraw / submit / GPU ms |', '| --- | ---: | ---: |']
    for config, blocks in sorted(groups.items()):
        def med(arm, field):
            values = [float(b[arm][field]) for b in blocks if arm in b]
            return statistics.median(values) if values else None
        cells = []
        for arm in ['base', 'look', 'candidate']:
            value = med(arm, 'wall_ns_p50_ms')
            cells.append('n/a' if value is None else f'{value:.3f}/{med(arm, "wall_ns_p95_ms"):.3f}')
        delta = [float(b['candidate']['wall_ns_p50_ms']) - float(b['look']['wall_ns_p50_ms']) for b in blocks]
        label = ' / '.join(config)
        timing.append(f'| {label} | ' + ' | '.join(cells) + f' | {min(delta):+.3f} to {max(delta):+.3f} |')
        pellet = [med(a, 'pellet_prepare_p50_ms') for a in ['base', 'look', 'candidate']]
        stage = [med('candidate', field) for field in ['redraw_ns_p50_ms', 'submit_ns_p50_ms', 'gpu_ns_p50_ms']]
        stages.append(f'| {label} | ' + ' / '.join('n/a' if v is None else f'{v:.3f}' for v in pellet) + ' | ' + ' / '.join(f'{v:.3f}' for v in stage) + ' |')
    pops = ['| Authored quality / arm | Microsteps | Max adjacent RGB byte | Max crop RGB-sum jump | Max relative jump % |', '| --- | ---: | ---: | ---: | ---: |']
    for quality in ['native', 'original']:
        for arm in ['base', 'candidate']:
            selected = [r for r in pop if (r['quality'], r['arm']) == (quality, arm)]
            pops.append(f'| {quality} / {arm} | {len(selected)} | {max(int(r["max_adjacent_rgb"]) for r in selected)} | {max(int(r["delta_light"]) for r in selected)} | {100 * max(float(r["relative_light_jump"]) for r in selected):.7f} |')
    selected = [r for r in rows if r['block_accepted'] == 'true']
    compiled = sum(int(r['compile_samples']) > 0 for r in rows)
    partners = len(rows) - compiled - len(selected)
    summary = f"Retained attempts: {len(rows)}; compile-overlapped: {compiled}; clean partners excluded with their block: {partners}; selected samples: {len(selected)}; complete selected blocks: {sum(map(len, groups.values()))}. Selected sample median machine load: {min(float(r['load_p50']) for r in selected):.1f}-{max(float(r['load_p50']) for r in selected):.1f}%."
    budget = ['| Authored quality | Callbacks / skips | Visible food | Maximum picture charge / limit |', '| --- | ---: | ---: | ---: |']
    for quality, value in sorted(capacity.items()):
        budget.append(f"| {quality} | {value['callbacks']} / {value['skips']} | {value['visible_foods']} | {value['max_picture_quads']} / 262144 |")
    return {'SUMMARY': summary, 'CAPACITY': '\n'.join(budget), 'TIMING': '\n'.join(timing), 'STAGES': '\n'.join(stages), 'POP': '\n'.join(pops)}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--check', action='store_true'); args = parser.parse_args()
    result = tables(*load())
    report = (ROOT / 'REPORT.md').read_text(encoding='utf-8')
    for key, table in result.items():
        if args.check:
            assert f'<!-- {key} -->\n{table}\n<!-- /{key} -->' in report, f'{key} report drift'
        print(table, '\n')
