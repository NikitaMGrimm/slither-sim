"""Offline, single-process retained-official radius audit. See PLAN.md."""
import argparse
import bisect
from collections import Counter, defaultdict
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import statistics as st
import sys


def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for b in iter(lambda: f.read(1024 * 1024), b''):
            h.update(b)
    return h.hexdigest()


def dump(path, obj):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('x', encoding='utf-8', newline='\n') as f:
        json.dump(obj, f, indent=2, allow_nan=False)
        f.write('\n')


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    obj = importlib.util.module_from_spec(spec)
    sys.modules[name] = obj
    spec.loader.exec_module(obj)
    return obj


def quant(v):
    v = sorted(v)
    if not v:
        return {'n': 0}
    return {'n': len(v), 'min': v[0], 'median': st.median(v),
            'p95': v[math.ceil(.95 * len(v)) - 1], 'max': v[-1]}


def sign(x):
    return (x > 0) - (x < 0)


def corr(a, b):
    if len(a) < 3:
        return None
    ma, mb = st.mean(a), st.mean(b)
    aa = sum((x-ma)**2 for x in a)
    bb = sum((x-mb)**2 for x in b)
    return sum((x-ma)*(y-mb) for x, y in zip(a, b)) / math.sqrt(aa*bb) if aa*bb else None


def extract(path, rel, sp, glob):
    out = {'archive': rel, 'sha256': sha(path), 'bytes': path.stat().st_size,
           'complete': False, 'interrupted': False, 'frames': 0,
           'connections': [], 'metadata_events': {}, 'error': None}
    conns = {}
    conn = None
    header = {}
    events = Counter()
    try:
        for frame in sp.read(path):
            if isinstance(frame, dict):
                event = frame.get('event')
                events[event or 'header'] += 1
                if event == 'capture-end':
                    out['complete'] = True
                if event == 'capture-interrupted':
                    out['interrupted'] = True
                if 'wall' in frame and 't' in frame:
                    header = {'wall': frame['wall'], 't': frame['t']}
                continue
            if not isinstance(frame, sp.Frame):
                continue
            out['frames'] += 1
            if frame.conn != conn:
                conn = frame.conn
                c = {'conn': conn, 'first_receipt_ms': frame.t,
                     'last_receipt_ms': frame.t, 'last_server_ms': frame.server,
                     'protocol': None, 'setup': None, 'radius': [], 'leaderboards': [],
                     'deaths': [], 'visibility_removals': 0, 'appearances': 0}
                conns[conn] = c
                grid, rank, rank_at, visible = glob.Minimap(), None, None, set()
                own = None
                setup_ok = False
            c['last_receipt_ms'], c['last_server_ms'] = frame.t, frame.server
            for p in frame.packets:
                kind = p[:1]
                if kind == b'a':
                    if len(p) < 25:
                        raise ValueError('short setup')
                    c['protocol'] = p[23]
                    setup_ok = p[23] == 19
                    if not setup_ok:
                        continue
                    c['setup'] = {'S': frame.server, 'receipt_ms': frame.t,
                                  'packet_hex': p.hex(), 'incoming_frame': out['frames']}
                    radius = sp.u24(p, 27) if len(p) >= 30 else .98*sp.u24(p, 1)
                elif not setup_ok:
                    continue
                elif kind == b'l':
                    rank, rank_at = glob.leaderboard(p), frame.server
                    c['leaderboards'].append({'S': frame.server, 'players': rank['players'],
                                             'prefix_hex': p[:6].hex(), 'incoming_frame': out['frames']})
                    continue
                elif kind in (b'M', b'V'):
                    grid.feed(p, frame.server)
                    continue
                elif kind == b's':
                    sid = sp.u16(p, 1)
                    if len(p) <= 7:
                        if len(p) > 3 and p[3] == 1:
                            c['deaths'].append({'S': frame.server, 'known': sid in visible, 'self': sid == own})
                        else:
                            c['visibility_removals'] += 1
                        visible.discard(sid)
                    else:
                        visible.add(sid)
                        c['appearances'] += 1
                        if own is None:
                            own = sid
                    continue
                elif kind == b'z':
                    if len(p) != 4:
                        raise ValueError('invalid radius length')
                    radius = sp.u24(p, 1)
                else:
                    continue
                row = {'conn': conn, 'S': frame.server, 'receipt_ms': frame.t,
                       'wall_ms': header.get('wall', 0) + frame.t - header.get('t', 0),
                       'radius': radius, 'kind': kind.decode(), 'packet_hex': p.hex(),
                       'incoming_frame': out['frames'], 'players': rank['players'] if rank else None,
                       'population_age_ms': frame.server-rank_at if rank_at is not None else None,
                       'rank_age_ms': frame.server-rank_at if rank_at is not None else None,
                       'leader_mass': rank['leader_mass'] if rank else None,
                       'largest_mass': rank['largest_mass'] if rank else None,
                       'visible_cache': len(visible), 'map_size': grid.size,
                       'map_fraction': sum(grid.cells)/len(grid.cells) if grid.cells else None,
                       'map_age_ms': frame.server-grid.server if grid.server is not None else None}
                # Initialization bytes contain no identity; other radius packets are four bytes.
                c['radius'].append(row)
        out['connections'] = list(conns.values())
    except Exception as exc:
        out['connections'] = list(conns.values())
        out['error'] = type(exc).__name__ + ': ' + str(exc)
    out['metadata_events'] = dict(events)
    out['eligible'] = out['complete'] and not out['interrupted'] and out['error'] is None
    return out


def connection_summary(c):
    rows = c['radius']
    if not rows:
        return None
    updates = [r for r in rows if r['kind'] == 'z']
    intervals = []
    runs, run_start, run_delta, last_sign = [], None, 0, 0
    for a, b in zip(rows, rows[1:]):
        dt = b['S']-a['S']
        dr = b['radius']-a['radius']
        if dt <= 0:
            continue
        intervals.append({'dt': dt, 'receipt_dt': b['receipt_ms']-a['receipt_ms'],
                          'dr': dr, 'rate': dr*1000/dt, 'a': a, 'b': b})
        direction = sign(dr)
        if direction != last_sign or dt > 1000:
            if run_start is not None:
                runs.append({'direction': last_sign, 'seconds': (a['S']-run_start)/1000, 'delta': run_delta})
            run_start, run_delta, last_sign = a['S'], 0, direction
        run_delta += dr
    if intervals and run_start is not None:
        runs.append({'direction': last_sign, 'seconds': (rows[-1]['S']-run_start)/1000, 'delta': run_delta})
    dense = [p for p in intervals if 100 <= p['dt'] <= 300]
    counts = [r['players'] for r in rows if r['players'] is not None]
    ledger = {'conn': c['conn'], 'protocol': c['protocol'],
              'game_seconds': c['last_server_ms']/1000, 'radius_rows': len(rows), 'z_packets': len(updates),
              'radius_start': rows[0]['radius'], 'radius_end': rows[-1]['radius'],
              'radius_min': min(r['radius'] for r in rows), 'radius_max': max(r['radius'] for r in rows),
              'total_variation': sum(abs(p['dr']) for p in intervals),
              'growth': sum(p['dr'] > 0 for p in intervals), 'shrink': sum(p['dr'] < 0 for p in intervals),
              'same': sum(p['dr'] == 0 for p in intervals), 'dense_intervals': len(dense),
              'delta_counts_dense': dict(sorted(Counter(p['dr'] for p in dense).items())),
              'all_interval_ms': quant([p['dt'] for p in intervals]),
              'dense_rate': quant([p['rate'] for p in dense]),
              'nonzero_run_seconds': quant([r['seconds'] for r in runs if r['direction']]),
              'runs': runs, 'player_min': min(counts) if counts else None,
              'player_max': max(counts) if counts else None,
              'marked_deaths': len(c['deaths']), 'plain_visibility_removals': c['visibility_removals']}
    return ledger, intervals


def trace_bins(c, seconds=30):
    rows = c['radius']
    result = []
    for start in range(0, int(c['last_server_ms']/1000), seconds):
        points = [r for r in rows if start*1000 <= r['S'] < (start+seconds)*1000]
        if len(points) < 2:
            continue
        counts = [r['players'] for r in points if r['players'] is not None and r['population_age_ms'] <= 3500]
        ds = [b['radius']-a['radius'] for a, b in zip(points, points[1:])]
        result.append({'seconds': [start, start+seconds], 'samples': len(points),
                       'start': points[0]['radius'], 'end': points[-1]['radius'],
                       'min': min(r['radius'] for r in points), 'max': max(r['radius'] for r in points),
                       'variation': sum(map(abs, ds)), 'growth': sum(d > 0 for d in ds),
                       'shrink': sum(d < 0 for d in ds),
                       'players_min': min(counts) if counts else None, 'players_max': max(counts) if counts else None})
    return result


def score(rows, predict):
    if not rows:
        return {'n': 0, 'balanced_accuracy': None, 'accuracy': None}
    matrix = Counter((r['direction'], predict(r)) for r in rows)
    pos = sum(r['direction'] == 1 for r in rows)
    neg = len(rows)-pos
    return {'n': len(rows), 'expanding': pos, 'shrinking': neg,
            'accuracy': (matrix[1, 1]+matrix[-1, -1])/len(rows),
            'balanced_accuracy': (matrix[1, 1]/pos + matrix[-1, -1]/neg)/2 if pos and neg else None}


def free_run(c):
    rows = c['radius']
    if len(rows) < 3 or c['last_server_ms'] < 60000:
        return None
    # Start at first causally available l report. No future count enters each 7ms tick.
    boards = c['leaderboards']
    if not boards:
        return None
    first = next((i for i, r in enumerate(rows) if r['players'] is not None and r['population_age_ms'] <= 3500), None)
    if first is None:
        return None
    rows = rows[first:]
    origin, limit = rows[0]['S'], rows[-1]['S']
    if limit-origin < 60000:
        return None
    selected = [r for r in rows if r['S'] <= origin+180000]
    bi = max(0, bisect.bisect_right([b['S'] for b in boards], origin)-1)
    radius = selected[0]['radius']
    fixed = radius
    time = origin
    preds = []
    for r in selected:
        while time+7 <= r['S']:
            while bi+1 < len(boards) and boards[bi+1]['S'] <= time:
                bi += 1
            target = 670*math.sqrt(max(1, boards[bi]['players']))
            radius += max(-3/128*7, min(3/128*7, target-radius))
            time += 7
        preds.append({'S': r['S'], 'observed': r['radius'], 'reported_population_model': radius, 'held_initial': fixed})
    return {'seconds': (selected[-1]['S']-origin)/1000, 'samples': len(selected),
            'population_model_mae': st.mean(abs(p['reported_population_model']-p['observed']) for p in preds),
            'held_initial_mae': st.mean(abs(p['held_initial']-p['observed']) for p in preds),
            'population_model_end_error': preds[-1]['reported_population_model']-preds[-1]['observed'],
            'rows': preds}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--repo', type=Path, required=True)
    ap.add_argument('--source', type=Path, required=True)
    ap.add_argument('--study', type=Path, required=True)
    ap.add_argument('--methods', type=Path, default=Path(__file__).with_name('methods'))
    args = ap.parse_args()
    root = args.source/'recordings'/'headless'
    sys.path.insert(0, str(args.repo/'tools'/'fidelity'))
    import slither_protocol as sp
    glob = module('radius_global', args.methods/'analyze-global-radius.py')
    dyn = module('radius_dynamics_current', args.methods/'analyze-radius-dynamics.py')
    motion = module('radius_motion', args.methods/'analyze-radius-motion.py')
    pins = [args.repo/p for p in ['src/simulator.rs', 'src/client/protocol.rs', 'docs/configuration.md',
                                'docs/server-rules.md', 'docs/research-workflow.md', 'CONTRIBUTING.md',
                                'tests/fixtures/arena-radius-updates.json', 'tools/fidelity/slither_protocol.py', 'uv.lock']]
    pins += [args.methods/p for p in ['analyze-global-radius.py', 'analyze-radius-dynamics.py', 'analyze-radius-motion.py']]
    pins += [Path(__file__), args.study/'PLAN.md']
    source_pins = [{'label': p.relative_to(args.repo).as_posix() if p.is_relative_to(args.repo) else
                    'methods/'+p.name if p.is_relative_to(args.methods) else p.name,
                    'sha256': sha(p)} for p in pins]
    paths = sorted(root.rglob('*.jsonl.gz'))
    inventory = [{'archive': p.relative_to(root).as_posix(), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in paths]
    dump(args.study/'inventory-before.json', inventory)
    files, summaries, groups, seen = [], [], defaultdict(list), set()
    all_intervals, motion_rows, forecasts, witnesses = [], {}, {}, []
    for i, (path, identity) in enumerate(zip(paths, inventory)):
        result = extract(path, identity['archive'], sp, glob)
        result['duplicate'] = result['sha256'] in seen
        seen.add(result['sha256'])
        files.append({k: v for k, v in result.items() if k != 'connections'})
        dump(args.study/'series'/Path(identity['archive']).with_suffix('.json'), result)
        if result['eligible'] and not result['duplicate']:
            for c in result['connections']:
                if c['protocol'] != 19 or not c['radius']:
                    continue
                summary, intervals = connection_summary(c)
                summary['archive'] = identity['archive']
                summary['sha256'] = result['sha256']
                summary['bins_30s'] = trace_bins(c)
                summaries.append(summary)
                for p in intervals:
                    all_intervals.append({**p, 'archive': identity['archive'], 'conn': c['conn']})
                rows = dyn.windows({'files': [{'sha256': result['sha256'], 'radius': c['radius']}]})
                for r in rows:
                    r['archive'] = identity['archive']
                    # Attach proxy state from exactly the window-start observation.
                    row = next(p for p in c['radius'] if p['S'] == r['S'])
                    for key in ('visible_cache', 'map_fraction', 'map_age_ms', 'leader_mass', 'largest_mass'):
                        r[key] = row[key]
                groups[path.parent.name].extend(rows)
                zs = [r for r in c['radius'] if r['kind'] == 'z']
                motion_rows.setdefault(path.parent.name, []).extend(motion.forecasts(zs))
                fr = free_run(c)
                if fr:
                    forecasts[identity['archive']+'#'+str(c['conn'])] = fr
        print(f'{i+1}/{len(paths)} {identity["archive"]}: complete={result["complete"]} error={result["error"]}', flush=True)
    train_dir = 'run3-large-radius-20260927'
    train = groups[train_dir]
    tests = {k: v for k, v in groups.items() if k != train_dir and v}
    models = {}
    for name, rows in tests.items():
        models[name] = dyn.models(train, rows)
        models[name]['current_670'] = {'test': score(rows, lambda r: 1 if 670*math.sqrt(r['players']) > r['radius'] else -1)}
    # Proxy comparisons use matching windows and training-only finite target searches.
    def matched(rows):
        return [r for r in rows if r['map_age_ms'] is not None and r['map_age_ms'] <= 5000
                and all(r[k] is not None and r[k] > 0 for k in ('visible_cache', 'map_fraction', 'leader_mass', 'largest_mass'))]
    gmtrain = matched(train)
    proxy = {}
    for key in ('players', 'visible_cache', 'map_fraction', 'leader_mass', 'largest_mass'):
        ratios = sorted({r['radius']/math.sqrt(r[key]) for r in gmtrain})
        candidates = [(k, lambda r, k=k, key=key: 1 if k*math.sqrt(r[key]) > r['radius'] else -1)
                      for k in [ratios[0]*.99]+[(a+b)/2 for a, b in zip(ratios, ratios[1:])]+[ratios[-1]*1.01]]
        coefficient, predict = dyn.choose(gmtrain, candidates)
        proxy[key] = {'coefficient': coefficient, 'train': score(gmtrain, predict),
                      'tests': {name: score(matched(rows), predict) for name, rows in tests.items()}}
    proxy['persistence'] = {'train': score(gmtrain, lambda r: r['previous_direction']),
                            'tests': {name: score(matched(rows), lambda r: r['previous_direction']) for name, rows in tests.items()}}
    dense = [p for p in all_intervals if 100 <= p['dt'] <= 300]
    states = defaultdict(dict)
    for p in dense:
        r = p['a']
        if r['players'] is not None and r['population_age_ms'] <= 1000 and p['dr']:
            states[r['players'], r['radius']].setdefault(sign(p['dr']), p)
    opposite = [{'players': key[0], 'radius': key[1], 'growth': val[1], 'shrink': val[-1]}
                for key, val in states.items() if 1 in val and -1 in val]
    # Keep within-capture witnesses as well as pooled ones.
    opposite.sort(key=lambda p: (p['growth']['archive'] != p['shrink']['archive'], p['radius']))
    eligible_windows = [r for rows in groups.values() for r in rows]
    coefficients = [p['a']['radius']/math.sqrt(p['a']['players']) for p in dense
                    if p['a']['players'] and p['a']['population_age_ms'] <= 3500]
    all_holdout = [r for rows in tests.values() for r in rows]
    pooled = dyn.models(train, all_holdout)
    pooled['current_670'] = {'train': score(train, lambda r: 1 if 670*math.sqrt(r['players']) > r['radius'] else -1),
                            'test': score(all_holdout, lambda r: 1 if 670*math.sqrt(r['players']) > r['radius'] else -1)}
    result = {'source_pins': source_pins, 'files': files, 'connections': summaries,
              'models_by_directory': models, 'models_pooled_holdout': pooled, 'proxy_models': proxy,
              'motion': {name: motion.summarize(rows) for name, rows in motion_rows.items()},
              'pooled': {'interval_ms': quant([p['dt'] for p in all_intervals]),
                         'receipt_interval_ms': quant([p['receipt_dt'] for p in all_intervals]),
                         'dense_intervals': len(dense), 'dense_signed_deltas': dict(sorted(Counter(p['dr'] for p in dense).items())),
                         'dense_rates': quant([p['rate'] for p in dense]),
                         'coefficient_radius_over_sqrt_players': quant(coefficients),
                         'fastest_growth': max(dense, key=lambda p: p['rate']),
                         'fastest_shrink': min(dense, key=lambda p: p['rate']),
                         'opposite_states_count': len(opposite), 'opposite_states_selected': opposite[:20],
                         'train_windows': len(train), 'holdout_windows': len(all_holdout),
                         'eligible_directions': len(eligible_windows)},
              'free_run': {k: {kk: vv for kk, vv in v.items() if kk != 'rows'} for k, v in forecasts.items()}}
    dump(args.study/'results.json', result)
    dump(args.study/'direction-windows.json', groups)
    dump(args.study/'free-run.json', forecasts)
    after = [{'archive': p.relative_to(root).as_posix(), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in paths]
    if inventory != after:
        raise RuntimeError('input changed during analysis')
    dump(args.study/'inventory-after.json', after)
    if any(sha(p) != pin['sha256'] for p, pin in zip(pins, source_pins)):
        raise RuntimeError('source changed during analysis')
    print(json.dumps({'files': len(files), 'eligible_files': sum(f['eligible'] and not f['duplicate'] for f in files),
                      'connections': len(summaries), 'dense_intervals': len(dense),
                      'train_windows': len(train), 'holdout_windows': len(all_holdout)}, indent=2))


if __name__ == '__main__':
    main()
