"""Bounded summaries only; Python stdlib, no recordings or Git revision reads."""
import csv
import json
import statistics as stats
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
ARMS = ['base', 'serial', 'pool']
EXPECTED_PARITY = {
    f'normal/{scene}-{adapter}-{size}-{quality}'
    for scene in ['replay', 'rectangle']
    for adapter, size in [('rtx', '3840x2160'), ('igpu', '3840x2160'), ('igpu', '1920x1080')]
    for quality in ['original', 'native']
    if scene != 'rectangle' or size != '1920x1080'
} | {
    f'{variant}/replay-rtx-3840x2160-{quality}'
    for variant in ['serial-fallback', 'late-20ms']
    for quality in ['original', 'native']
} | {f'reconnect/{quality}' for quality in ['original', 'native']}


def rows(name):
    with (ROOT / name).open(newline='', encoding='utf-8') as stream:
        return list(csv.DictReader(stream))


def median(records, field):
    return stats.median(float(r[field]) for r in records)


def validate(runs, parity, identities):
    pairs = {}
    for r in runs:
        assert (int(r['callbacks']), int(r['messages']), int(r['skips'])) == (600, 29044, 0)
        assert r['binary_sha256'] == identities[r['arm']]['binary_sha256']
        assert r['canonical_active'] == 'False'
        assert r['inactive_quad_accumulator'] == 'd1fba762150c532c'
        key = (r['condition'], r['round'], r['adapter'], r['physical'], r['quality'])
        pair = pairs.setdefault(key, {})
        assert r['arm'] not in pair
        pair[r['arm']] = r
        assert (r['status'] == 'accepted') == (int(r['compile_samples']) == 0)
    for key, pair in pairs.items():
        assert set(pair) == (set(ARMS) if key[0] == 'adoption' else {'serial', 'pool'})
        use = all(r['status'] == 'accepted' for r in pair.values())
        assert all((r['paired'] == 'True') == use for r in pair.values())
        assert len({r['command_digest'] for r in pair.values()}) == 1
    assert set(parity) == EXPECTED_PARITY
    for key, r in parity.items():
        assert r['callbacks'] == r['baseline_callbacks'] == 600
        assert r['skips'] == r['baseline_skips'] == 0
        expected = identities['base']['binary_sha256'] if key.startswith('reconnect/') else identities['earlier_image_baseline_binary_sha256']
        assert r['baseline_binary_sha256'] == expected
        assert r['binary_sha256'] == identities['pool']['binary_sha256'] != expected
        assert r['image_sequence_sha256'] == r['baseline_image_sequence_sha256']
    return pairs


def tables(runs, idle, density, callbacks):
    selected = [r for r in runs if r['condition'] == 'adoption' and r['paired'] == 'True']
    configs = sorted({(r['adapter'], r['physical'], r['quality']) for r in selected}, key=lambda c: (c[0] != 'rtx', c[1], c[2]))
    text = ['| Adapter / physical size / quality | Pairs | Main art median/p95 ms | Serial split ms | Pool ms |', '| --- | ---: | ---: | ---: | ---: |']
    for config in configs:
        group = [r for r in selected if (r['adapter'], r['physical'], r['quality']) == config]
        count = len(group) // 3
        assert count >= (5 if config[0] == 'rtx' else 2)
        cells = []
        for arm in ARMS:
            part = [r for r in group if r['arm'] == arm]
            cells.append(f"{median(part, 'wall_ns_p50_ms'):.3f}/{median(part, 'wall_ns_p95_ms'):.3f}")
        text.append('| ' + ' / '.join(config) + f' | {count} | ' + ' | '.join(cells) + ' |')

    text += ['', '| RTX 4K / quality / arm | Art-median range ms | Whole-replay median/p95 ms | CPU stages ms | Pellet prepare ms | Submit ms | GPU ms | Process requests / GB |', '| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    for quality in ['original', 'native']:
        for arm in ARMS:
            group = [r for r in selected if r['adapter'] == 'rtx' and r['quality'] == quality and r['arm'] == arm]
            spread = [float(r['wall_ns_p50_ms']) for r in group]
            cells = [f'{min(spread):.3f}-{max(spread):.3f}', f"{median(group, 'whole_p50_ms'):.3f}/{median(group, 'whole_p95_ms'):.3f}"]
            cells += [f'{median(group, k):.3f}' for k in ['cpu_ns_p50_ms', 'pellet_prepare_p50_ms', 'submit_ns_p50_ms', 'gpu_ns_p50_ms']]
            cells += [f"{median(group, 'whole_process_requests'):.0f} / {median(group, 'whole_process_bytes') / 1e9:.3f}"]
            text.append(f'| {quality} / {arm} | ' + ' | '.join(cells) + ' |')

    text += ['', '| Stress / quality | Pairs | Serial median/p95 ms | Pool median/p95 ms | Median host load % |', '| --- | ---: | ---: | ---: | ---: |']
    for condition in ['late20', 'busy8']:
        for quality in ['original', 'native']:
            group = [r for r in runs if r['condition'] == condition and r['quality'] == quality and r['paired'] == 'True']
            assert len(group) >= 4
            cells = []
            for arm in ['serial', 'pool']:
                part = [r for r in group if r['arm'] == arm]
                cells.append(f"{median(part, 'wall_ns_p50_ms'):.3f}/{median(part, 'wall_ns_p95_ms'):.3f}")
            text.append(f"| {condition} / {quality} | {len(group)//2} | " + ' | '.join(cells) + f" | {median(group, 'load_p50'):.1f} |")

    text += ['', '| Long idle control | Serial process CPU % of one core | Pool process CPU % | Serial/pool CPU ms per callback | Serial/pool callback median ms |', '| --- | ---: | ---: | ---: | ---: |']
    for mode in ['menu', 'sparse', 'after-dense']:
        group = [r for r in idle if r['duration_virtual_s'] == '60' and r['mode'] == mode and r['pair'] in ['11', '12'] and r['status'] == 'accepted']
        part = [[r for r in group if r['arm'] == arm] for arm in ['serial', 'pool']]
        assert all(len(p) == 2 for p in part)
        text.append(f'| {mode} | ' + ' | '.join(f"{median(p, 'one_core_percent'):.3f}" for p in part) + ' | ' + '/'.join(f"{median(p, 'cpu_ms_per_callback'):.3f}" for p in part) + ' | ' + '/'.join(f"{median(p, 'callback_p50_ms'):.3f}" for p in part) + ' |')

    text += ['', '| Visible / settled | Serial preparation ms | Two participants ms | Four participants ms |', '| --- | ---: | ---: | ---: |']
    for count in ['1024', '2048', '4096', '8192']:
        for settled in ['false', 'true']:
            group = [r for r in density if r['density'] == count and r['settled'] == settled]
            cells = [f"{median([r for r in group if r['participants'] == n], 'p50_ns')/1e6:.4f}" for n in ['1', '2', '4']]
            text.append(f'| {count} / {settled} | ' + ' | '.join(cells) + ' |')

    text += ['', '| First matched pool run | Rebuild callbacks / 300 | Rebuilds over 1 ms | Slowest-16 median rebuild / ingress / redraw ms |', '| --- | ---: | ---: | ---: |']
    for quality in ['original', 'native']:
        group = [r for r in callbacks if r['quality'] == quality]
        assert len(group) == 300
        slow = sorted(group, key=lambda r: int(r['wall_ns']), reverse=True)[:16]
        cells = '/'.join(f'{median(slow, k)/1e6:.3f}' for k in ['pellet_sources_ns', 'ingress_ns', 'redraw_ns'])
        text.append(f"| {quality} | {sum(int(r['pellet_sources_ns'])>0 for r in group)} | {sum(int(r['pellet_sources_ns'])>1000000 for r in group)} | {cells} |")
    return '\n'.join(text)


def main():
    runs = rows('runs.csv')
    parity = json.loads((ROOT / 'parity.json').read_text(encoding='utf-8'))
    identities = json.loads((ROOT / 'identities.json').read_text(encoding='utf-8'))
    validate(runs, parity, identities)
    text = tables(runs, rows('idle.csv'), rows('density.csv'), rows('callback-stages.csv'))
    if '--check' in sys.argv:
        assert text in (ROOT / 'REPORT.md').read_text(encoding='utf-8'), 'report tables differ from retained rows'
        print('Timing pairs, source-bound binaries, 9600 image pairs and report tables verified')
    else:
        print(text)


if __name__ == '__main__':
    main()
