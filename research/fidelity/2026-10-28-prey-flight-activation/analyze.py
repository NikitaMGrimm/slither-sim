"""Offline, serial analysis of the cohort frozen in PLAN.md; no network code."""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT / 'tools/fidelity'))
import slither_protocol as sp

PLAN_COMMIT = 'f228e6c632290b53e85fd1383ebad66a16767f6e'
BASE = 'f9154e92c76aaf904e1031ef44196ec52bb783d2'
ARENA = 'c2fb7285febb4273034e30699dc880715a334423'
HISTORICAL_PATHS = (
    'docs/research/prey-state-audit.md', 'docs/research/prey-flight-speed-audit.md',
    'docs/research/prey-decision-cadence.md', 'docs/research/prey-activation-shape.md',
    'docs/research/prey-width-refinement.md', 'docs/research/prey-flee-jitter-audit.md',
    'docs/research/prey-radial-mode-audit.md', 'docs/research/prey-pulse-onset-audit.md',
    'tools/fidelity/analyze-prey-flight-speed.py',
    'tools/fidelity/analyze-prey-activation-shape.py',
    'tools/fidelity/slither_protocol.py', 'tests/fixtures/prey-activation-shape.json')
SOURCES = {
    'fidelity-prey-transitions-20260927': '282834351bce3a5e9c5bdf342b179a6ea1634eacca450b98c7924a52c7cf669a',
    'prey-controlled-20260927-first': 'c65f2d0b248f851743963b067dc53767028eb63d2709c72f35766b4d3c45ba7e',
    'prey-boundary-validation-20260927': '264b132056f5b4527691955b08770e24daf36a86bb651860236a3ecf0d9d799f',
    'prey-width-confirmation-20260927': 'ff0aebb9b7b66f9e9773416e14a9328d7edbd7af7dfc785f890bd5dbe51cdcb9',
    'prey-steering-validation-20260927': 'b7630b155d374914bb7e9497f5af602184e08339f1334c2e755f801970bf4248',
}
SLOPE = 4.844 / 8.21
LIPSCHITZ = max(SLOPE, .59025)
REGIONS = {'circle300': ('circle', 300), 'circle415': ('circle', 415),
           'square315': ('square', 315), 'square320': ('square', 320)}
JITTERS = (0, 7, 14, 42)


def one_cpu():
    if os.name == 'nt':
        import ctypes
        kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.GetProcessAffinityMask.argtypes = [ctypes.c_void_p,
                                                  ctypes.POINTER(ctypes.c_size_t),
                                                  ctypes.POINTER(ctypes.c_size_t)]
        kernel.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        proc = kernel.GetCurrentProcess()
        mask, system = ctypes.c_size_t(), ctypes.c_size_t()
        if not kernel.GetProcessAffinityMask(proc, mask, system):
            raise ctypes.WinError(ctypes.get_last_error())
        if not kernel.SetProcessAffinityMask(proc, mask.value & -mask.value):
            raise ctypes.WinError(ctypes.get_last_error())
    elif hasattr(os, 'sched_getaffinity'):
        os.sched_setaffinity(0, {min(os.sched_getaffinity(0))})
    else:
        raise RuntimeError('Set one-CPU affinity before using this platform')


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b''):
            digest.update(chunk)
    return digest.hexdigest()


def laws(x):
    return {'A': 6.153 + min(1, max(0, (x - 5.79) / 8.21)) * 4.844,
            'F': 2.74227 + .59025 * x,
            'B': 6.153 if x < 13.5 else 10.997}


def ordinary(speed):
    return speed == 3.8 or 6.14 <= speed < 11


def speed_budget(j=None):
    return .001 + LIPSCHITZ * (1 / 18 + (0 if j is None else 16 * (.001 + 2 * j / 1000)))


def region_bounds(snake, prey_speed, j, decision_age=False):
    age = max(0, snake['age']) / 1000
    u = (6 + age * (snake['speed'] / .032 * math.tau / 256 + (1 / 18) / .032)
         + (snake['speed'] + prey_speed) / .032 * (.001 + 2 * j / 1000))
    if decision_age:
        u += (snake['speed'] + prey_speed) / .032 * .336 + math.ceil(.336 / .007)
    lower = [max(0, abs(snake[k]) - u) for k in ('dx', 'dy')]
    upper = [abs(snake[k]) + u for k in ('dx', 'dy')]
    return {'square': (max(lower), max(upper)),
            'circle': (math.hypot(*lower), math.hypot(*upper))}


def prediction(row, name):
    shape, width = REGIONS[name]
    field = 'box' if shape == 'square' else 'd'
    return any(s[field] < width for s in row['snakes'])


def robust_prediction(row, name, j, decision_age=False):
    shape, width = REGIONS[name]
    bounds = [region_bounds(s, row['speed'], j, decision_age)[shape] for s in row['snakes']]
    if any(high < width for low, high in bounds):
        return True
    if all(low >= width for low, high in bounds):
        return False
    return None


def history_range(history, now):
    """Carry the last causal value at the left edge, never use a future report."""
    before = [item for item in history if item[0] <= now - 336]
    if not before:
        return None
    values = [before[-1][1]] + [v for t, v in history if now - 336 < t <= now]
    return min(values), max(values)


def inspect(path):
    # Body contraction never feeds the stored head base or heading/speed history.
    # Skipping the unused body smoothing avoids quadratic work on long snakes.
    world, connection = sp.World(contract=False), None
    episodes, history, speed_source, prior_mode = {}, {}, {}, {}
    flights, activations = [], []
    census, layouts = Counter(), Counter()
    complete = False
    for frame in sp.read(path):
        if isinstance(frame, dict):
            complete |= frame.get('event') == 'capture-end'
            continue
        if not isinstance(frame, sp.Frame):
            continue
        census['frames'] += 1
        for packet in frame.packets:
            if frame.conn != connection or packet[:1] == b'a':
                world, connection = sp.World(contract=False), frame.conn
                episodes, history, speed_source, prior_mode = {}, {}, {}, {}
            kind = packet[:1]
            if kind == b'y':
                pid = sp.u16(packet, 1)
                # Preserve the original analyzer's episode numbering (all y).
                episodes[pid] = episodes.get(pid, 0) + 1
                prior_mode.pop(pid, None)
            world.events.clear()
            world.feed(packet, frame.server)
            for event in world.events:
                if event[0] == 'spawn':
                    sid = event[2]
                    history[sid] = [(frame.server, world.snakes[sid].sp)]
                    speed_source[sid] = (frame.server, packet.hex())
                elif event[0] == 'rot' and event[3][2] != event[4][2]:
                    history.setdefault(event[2], []).append((frame.server, event[4][2]))
                    speed_source[event[2]] = (frame.server, packet.hex())
                elif event[0] == 'remove':
                    history.pop(event[2], None)
                    speed_source.pop(event[2], None)
            if kind != b'j':
                continue
            layouts[len(packet)] += 1
            census['j'] += 1
            census['present_speed'] += len(packet) in (9, 12, 13, 16)
            census['present_heading'] += len(packet) in (10, 12, 14, 16)
            census['present_target'] += len(packet) in (11, 13, 14, 16)
            prey = world.prey.get(sp.u16(packet, 1))
            if not prey:
                census['unknown_prey_j'] += 1
                continue
            if len(packet) != 16:
                continue
            census['full_j'] += 1
            pid = sp.u16(packet, 1)
            previous = prior_mode.get(pid)
            if ordinary(prey['sp']):
                prior_mode[pid] = (frame.server, prey['sp'] != 3.8)
            else:
                prior_mode.pop(pid, None)
            snakes = []
            for snake in world.snakes.values():
                if not snake.pts:
                    continue
                head = snake.head(frame.server)
                dx, dy = prey['x'] - head[0], prey['y'] - head[1]
                snakes.append({'sid': snake.id, 'dx': dx, 'dy': dy,
                               'box': max(abs(dx), abs(dy)), 'd': math.hypot(dx, dy),
                               'speed': snake.sp, 'age': frame.server - snake.anchor_server})
            snakes.sort(key=lambda s: s['d'])
            if not snakes:
                continue
            nearest = snakes[0]
            isolated = len(snakes) == 1 or snakes[1]['d'] > 700
            key = [connection, pid, episodes.get(pid, 0)]
            if ordinary(prey['sp']) and nearest['d'] <= 650 and nearest['age'] <= 150:
                activations.append({'key': key, 'S': frame.server, 'speed': prey['sp'],
                                    'flight': prey['sp'] != 3.8, 'isolated': isolated,
                                    'snakes': snakes,
                                    'previous_mode_changed': (previous[1] != (prey['sp'] != 3.8))
                                    if previous else None})
            away = math.atan2(nearest['dy'], nearest['dx'])
            error = abs(math.remainder(prey['wang'] - away, math.tau))
            if not (6.14 <= prey['sp'] < 11 and nearest['d'] < 450 and error < .5):
                continue
            sid = nearest['sid']
            source = speed_source.get(sid)
            flights.append({'key': key, 'S': frame.server, 'prey_speed': prey['sp'],
                            'snake_id': sid, 'snake_speed': nearest['speed'],
                            'snake_segments': world.snakes[sid].sct,
                            'prey_packet_hex': packet.hex(),
                            'snake_speed_source_S': source[0] if source else None,
                            'snake_speed_source_hex': source[1] if source else None,
                            'nearest_head_distance': nearest['d'], 'target_away_error': error,
                            'strict': nearest['age'] <= 150 and isolated and nearest['box'] < 320,
                            'speed_age_ms': frame.server - source[0] if source else None,
                            'history_range': history_range(history.get(sid, []), frame.server)})
    if not complete:
        raise ValueError('Missing capture-end')
    return {'census': dict(census), 'j_layouts': dict(sorted(layouts.items())),
            'flights': flights, 'activations': activations}


def flight_scores(rows):
    models = {}
    for name in ('A', 'F', 'B'):
        errors = [r['prey_speed'] - laws(r['snake_speed'])[name] for r in rows]
        absolute = sorted(map(abs, errors))
        models[name] = {'mae': statistics.mean(absolute) if rows else None,
                        'rmse': math.sqrt(statistics.mean(e * e for e in errors)) if rows else None,
                        'signed_mean': statistics.mean(errors) if rows else None,
                        'p90_abs': absolute[int(.9 * (len(rows) - 1))] if rows else None,
                        'max_abs': max(absolute) if rows else None,
                        'incompatible': {str(j): sum(e > speed_budget(j) for e in absolute)
                                         for j in JITTERS}}
        if name != 'B':
            eligible = [r for r in rows if r['history_range'] is not None]
            misses = []
            for r in eligible:
                low, high = [laws(x)[name] for x in r['history_range']]
                misses.append(max(low - r['prey_speed'], r['prey_speed'] - high, 0))
            models[name]['causal_history'] = {
                'rows': len(eligible), 'missing': len(rows) - len(eligible),
                'outside_quantizer_only': sum(e > speed_budget() for e in misses),
                'mae_to_envelope': statistics.mean(misses) if misses else None}
    previous, pairs = {}, []
    for row in rows:
        key = tuple(row['key'])
        old = previous.get(key)
        if (old and old['snake_id'] == row['snake_id'] and 0 < row['S'] - old['S'] < 2000
                and abs(row['snake_speed'] - old['snake_speed']) >= 1):
            pairs.append((old, row))
        previous[key] = row
    return {'rows': len(rows), 'episodes': len({tuple(r['key']) for r in rows}),
            'models': models, 'same_snake_pairs': len(pairs),
            'pair_ratio_median': statistics.median((b['prey_speed'] - a['prey_speed']) /
                                                  (b['snake_speed'] - a['snake_speed'])
                                                  for a, b in pairs) if pairs else None,
            'pair_delta_mae': {name: statistics.mean(abs((b['prey_speed'] - a['prey_speed']) -
                                                       (laws(b['snake_speed'])[name] -
                                                        laws(a['snake_speed'])[name]))
                                                    for a, b in pairs) if pairs else None
                               for name in models},
            'AF_disjoint_quantizer_only': sum(abs(laws(r['snake_speed'])['A'] -
                                                  laws(r['snake_speed'])['F']) >
                                              2 * speed_budget() for r in rows)}


def classification(rows, name):
    counts = Counter()
    for row in rows:
        p = prediction(row, name)
        counts[('t' if p == row['flight'] else 'f') + ('p' if p else 'n')] += 1
    tp, fp, tn, fn = [counts[k] for k in ('tp', 'fp', 'tn', 'fn')]
    return {'rows': len(rows), 'tp': tp, 'fp': fp, 'tn': tn, 'fn': fn,
            'balanced_accuracy': (tp / (tp + fn) + tn / (tn + fp)) / 2
            if tp + fn and tn + fp else None}


def activation_scores(rows):
    groups = defaultdict(list)
    for r in rows:
        groups[tuple(r['key'])].append(r)
    result = {'rows': len(rows), 'episodes': len(groups), 'models': {}}
    for name in REGIONS:
        recalls, specifics = [], []
        for group in groups.values():
            c = classification(group, name)
            if c['tp'] + c['fn']:
                recalls.append(c['tp'] / (c['tp'] + c['fn']))
            if c['tn'] + c['fp']:
                specifics.append(c['tn'] / (c['tn'] + c['fp']))
        model = classification(rows, name)
        model['episode_balanced_accuracy'] = (statistics.mean(recalls) + statistics.mean(specifics)) / 2 if recalls and specifics else None
        model['class_episodes'] = {'flight': len(recalls), 'rest': len(specifics)}
        model['first_per_episode'] = classification([g[0] for g in groups.values()], name)
        model['uncertainty'] = {}
        for lag in (False, True):
            for j in JITTERS:
                counts = Counter()
                for row in rows:
                    p = robust_prediction(row, name, j, lag)
                    counts['ambiguous' if p is None else 'correct' if p == row['flight'] else 'wrong'] += 1
                model['uncertainty'][f'j{j}_lag{int(lag)}'] = {
                    k: counts[k] for k in ('correct', 'wrong', 'ambiguous')}
        result['models'][name] = model
    disagreements = [r for r in rows if prediction(r, 'square315') != prediction(r, 'square320')]
    result['width_disagreements'] = {'rows': len(disagreements),
        'episodes': len({tuple(r['key']) for r in disagreements}),
        'previous_mode_changed': dict(Counter('missing' if r['previous_mode_changed'] is None
                                             else 'yes' if r['previous_mode_changed'] else 'no'
                                             for r in disagreements)),
        'robust_disjoint_j0': sum(robust_prediction(r, 'square315', 0) is not None and
                                 robust_prediction(r, 'square320', 0) is not None
                                 for r in disagreements)}
    return result


def fixture_check(rows):
    fixture = json.loads((ROOT / 'tests/fixtures/prey-flight-speed-fresh.json').read_text())
    lookup = {(r['key'][0], r['S'], r['key'][1]): r for r in rows}
    fields = ('prey_speed', 'snake_id', 'snake_speed', 'snake_segments',
              'prey_packet_hex', 'snake_speed_source_S', 'snake_speed_source_hex',
              'nearest_head_distance', 'target_away_error')
    failures = []
    for case in fixture['cases']:
        row = lookup.get((case['conn'], case['S'], case['prey_id']))
        if row is None:
            failures.append('missing-row')
            continue
        for field in fields:
            if row[field] != case[field]:
                failures.append(field)
        if row['key'][2] != case['episode']:
            failures.append('episode')
    return {'cases': len(fixture['cases']), 'failures': dict(Counter(failures)),
            'matched_exactly': not failures}


def activation_fixture_check(rows, fixture):
    lookup = {(tuple(r['key']), r['S']): r for r in rows}
    failures = []
    for case in fixture['cases']:
        r = lookup.get(((case['conn'], case['pid'], case['episode']), case['S']))
        if r is None:
            failures.append('missing-row')
            continue
        for field in ('speed', 'flight'):
            if r[field] != case[field]:
                failures.append(field)
        if len(r['snakes']) != len(case['snakes']):
            failures.append('snake-count')
        for actual, expected in zip(r['snakes'], case['snakes']):
            for field in ('sid', 'd', 'box', 'dx', 'dy', 'age', 'speed'):
                if actual[field] != expected[field]:
                    failures.append(field)
    return {'cases': len(fixture['cases']), 'failures': dict(Counter(failures)),
            'matched_exactly': not failures}


def flight_diagnostics(rows):
    errors = [abs(r['prey_speed'] - laws(r['snake_speed'])['A']) for r in rows]
    worst = rows[max(range(len(rows)), key=errors.__getitem__)] if rows else None
    return {'A_errors_above_027': sum(e > .27 for e in errors),
            'maximum_witness': {k: worst[k] for k in
                                ('key', 'S', 'prey_speed', 'snake_speed', 'speed_age_ms',
                                 'strict', 'history_range')} if worst else None,
            'snake_speed_range': [min(r['snake_speed'] for r in rows),
                                  max(r['snake_speed'] for r in rows)] if rows else None}


def power():
    endpoints = [abs(laws(x)['A'] - laws(x)['F']) for x in (5.79, 14)]
    maximum = max(endpoints)
    needed = (maximum / 2 - .001) / LIPSCHITZ
    return {'AF_endpoint_separations': endpoints, 'AF_max_separation': maximum,
            'speed_quantizer_only_2E': 2 * speed_budget(),
            'speed_2E_by_j': {str(j): 2 * speed_budget(j) for j in JITTERS},
            'needed_snake_speed_full_cell_below': needed,
            'current_snake_speed_cell': 1 / 18,
            'width_separation': 5, 'coordinate_only_2U': 12,
            'needed_geometry_half_width_below': 2.5,
            'timing_only_needed_seconds_below_at_max_speed': 2.5 / ((14 + 11) / .032),
            'width_capture_pass': 5 > 12, 'AF_capture_pass': maximum > 2 * speed_budget(),
            'capture_proposed': False}


def run(archive_root, output):
    output.mkdir(parents=True, exist_ok=False)
    for name, digest in SOURCES.items():
        path = archive_root / 'recordings/headless' / name / 'bot0.jsonl.gz'
        if not path.is_file() or sha(path) != digest:
            raise ValueError(f'Missing/mismatched frozen source: {name}')
    frozen = subprocess.run(['git', 'show', f'{PLAN_COMMIT}:{(HERE / "PLAN.md").relative_to(ROOT).as_posix()}'],
                            cwd=ROOT, check=True, capture_output=True).stdout
    if hashlib.sha256(frozen).hexdigest() != sha(HERE / 'PLAN.md'):
        raise ValueError('PLAN.md differs from its frozen commit')
    selected = {'plan_commit': PLAN_COMMIT, 'simulator_base': BASE, 'arena_pin': ARENA,
                'split': 'development_only', 'sources': {}, 'power': power(),
                'historical_artifact_sha256': {
                    p: hashlib.sha256(subprocess.run(['git', '-C', str(archive_root), 'show', f'{ARENA}:{p}'],
                                                    check=True, capture_output=True).stdout).hexdigest()
                    for p in HISTORICAL_PATHS},
                'hashes': {str(p.relative_to(ROOT)).replace('\\', '/'): sha(p)
                           for p in (HERE / 'PLAN.md', HERE / 'analyze.py',
                                     HERE / 'test_analysis.py',
                                     ROOT / 'tools/fidelity/slither_protocol.py',
                                     ROOT / 'tests/fixtures/prey-flight-speed-fresh.json',
                                     ROOT / 'tests/fixtures/prey-outward-motion.json',
                                     ROOT / 'tests/fixtures/prey-motion-quantization.json')}}
    for name, digest in SOURCES.items():
        path = archive_root / 'recordings/headless' / name / 'bot0.jsonl.gz'
        raw = inspect(path)
        with (output / (name + '-rows.json')).open('x', encoding='utf-8', newline='\n') as f:
            json.dump(raw, f)
        source = {'sha256': digest, 'capture_end': True, 'census': raw['census'],
                  'j_layouts': raw['j_layouts'],
                  'flight_diagnostics': flight_diagnostics(raw['flights']),
                  'flight': {label: flight_scores([r for r in raw['flights'] if label == 'all' or r['strict']])
                             for label in ('all', 'strict')},
                  'activation': {label: activation_scores([r for r in raw['activations'] if label == 'all' or r['isolated']])
                                 for label in ('all', 'isolated')}}
        if name == 'fidelity-prey-transitions-20260927':
            source['fixture_reproduction'] = fixture_check(raw['flights'])
        if name == 'prey-controlled-20260927-first':
            fixture = subprocess.run(['git', '-C', str(archive_root), 'show',
                                      f'{ARENA}:tests/fixtures/prey-activation-shape.json'],
                                     check=True, capture_output=True).stdout
            source['historical_fixture_reproduction'] = activation_fixture_check(
                raw['activations'], json.loads(fixture))
        for key in ('fixture_reproduction', 'historical_fixture_reproduction'):
            if key in source and not source[key]['matched_exactly']:
                raise ValueError(f'Frozen fixture mismatch: {name}: {source[key]}')
        selected['sources'][name] = source
        print(json.dumps({'source': name, 'flight': source['flight']['all']['rows'],
                          'activation': source['activation']['all']['rows']}), flush=True)
    with (output / 'selected-results.json').open('x', encoding='utf-8', newline='\n') as f:
        json.dump(selected, f, indent=2, sort_keys=True)
        f.write('\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    one_cpu()
    run(args.archive_root, args.output)
