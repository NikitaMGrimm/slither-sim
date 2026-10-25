"""Offline analysis of frozen batches; validation requires a sealed analysis source."""
import os
for key in ('OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS', 'NUMEXPR_NUM_THREADS'):
    os.environ[key] = '1'
import argparse
from collections import deque
import ctypes
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

import numpy as np
import scipy

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
RAW = Path(r'C:\Users\Nikita\.t3\scratch\official-capture-20261025')
OLD = Path(r'C:\Users\Nikita\.t3\scratch\network-model-study-20261025')
ARENA = Path(r'C:\Users\Nikita\Source\slither-arena-rs\tools\fidelity')
sys.path.insert(0, str(REPO / 'tools/fidelity'))
import slither_protocol as sp
from capture_quality import audit_capture
sys.path.insert(0, str(OLD))
from analyze import baseline, summary, acf_time, acf_frames
from fit import simulate


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def clean_mask(times, windows):
    result = np.ones(len(times), dtype=bool)
    for a, b in windows:
        result &= ~((times >= a) & (times <= b))
    return result


def frame_floor_excess(t, s):
    """Exact parent metric: [i-600, i+600), clipped at each epoch boundary."""
    offsets = t - s
    candidates, right, floor = deque(), 0, np.empty(len(t))
    for i in range(len(t)):
        while right < min(len(t), i + 600):
            while candidates and offsets[candidates[-1]] >= offsets[right]:
                candidates.pop()
            candidates.append(right)
            right += 1
        while candidates and candidates[0] < i - 600:
            candidates.popleft()
        floor[i] = offsets[candidates[0]]
    return offsets - floor


def tails(t, e, mask, threshold):
    high = (e > threshold) & mask
    starts = np.flatnonzero(high & ~np.r_[False, high[:-1]])
    ends = np.flatnonzero(high & ~np.r_[high[1:], False])
    valid = mask[:-1] & mask[1:]
    seconds = float(np.diff(t)[valid].sum() / 1000)
    peaks, durations, brackets, censored = [], [], [], 0
    for a, b in zip(starts, ends):
        peaks.append(float(e[a:b+1].max()))
        durations.append(float(t[b] - t[a]))
        if a == 0 or b + 1 == len(t) or not mask[a-1] or not mask[b+1]:
            censored += 1
        else:
            brackets.append(float(t[b+1] - t[a-1]))
    return {'episodes': len(starts), 'observedSeconds': seconds,
            'ratePerSecond': len(starts) / seconds if seconds else None,
            'peakMs': summary(peaks), 'observedDurationMs': summary(durations),
            'surroundingBracketMs': summary(brackets), 'censoredEpisodes': censored}


def features(epochs, clean=False):
    values, acs, weights, bursts, pairs = [], [], [], 0, 0
    for epoch in epochs:
        t, e = epoch['t'], epoch['e']
        mask = epoch['clean'] if clean else np.ones(len(t), dtype=bool)
        if not mask.any():
            continue
        values.extend(e[mask])
        acs.append(acf_time(t, e, mask))
        weights.append(int(mask.sum()))
        valid_pair = mask[:-1] & mask[1:]
        bursts += int(((np.diff(t) < 1) & valid_pair).sum())
        pairs += int(valid_pair.sum())
    if not values:
        return {'n': 0}
    ac = {}
    for lag in ('10', '50', '100', '500', '1000'):
        eligible = [(row[lag], w) for row, w in zip(acs, weights) if row[lag] is not None]
        ac[lag] = float(np.average([v for v, _ in eligible], weights=[w for _, w in eligible])) if eligible else None
    tail_rows = {}
    for threshold in (10, 20):
        tail_rows[str(threshold)] = [tails(row['t'], row['e'], row['clean'] if clean else
                                         np.ones(len(row['t']), dtype=bool), threshold) for row in epochs]
    return {'excess': summary(values), 'acf': ac, 'tailEpisodesByEpoch': tail_rows,
            'burstLt1': bursts / pairs if pairs else None, 'adjacentPairs': pairs}


def gates(observed, modeled):
    if 'excess' not in observed or 'excess' not in modeled:
        return {'eligible': False}
    q = observed['excess']
    result = {name: abs(modeled['excess'][name] - q[name]) <= max(.5, .15 * q[name])
              for name in ('p50', 'p90')}
    result['p99'] = abs(modeled['excess']['p99'] - q['p99']) <= .2 * q['p99']
    result['burst'] = abs(modeled['burstLt1'] - observed['burstLt1']) <= .005
    result['acf'] = {lag: observed['acf'][lag] is not None and modeled['acf'][lag] is not None
                     and abs(modeled['acf'][lag] - observed['acf'][lag]) <= .10 for lag in observed['acf']}
    return result


def rtt_sensitivity(params, seed):
    schedule = np.arange(0, 600000, 7.)
    rng = np.random.default_rng(seed)
    down, _ = simulate(schedule, params, seed)
    up, _ = simulate(schedule, params, seed + 100)
    ordinary = list(params)
    ordinary[5] = 0
    up_ordinary, _ = simulate(schedule, ordinary, seed + 100)
    ix = rng.integers(0, len(schedule), 100000)
    iy = rng.integers(0, len(schedule), 100000)
    d, u, u0 = (down-schedule)[ix], (up-schedule)[iy], (up_ordinary-schedule)[iy]
    wait = rng.uniform(0, 7, len(ix))
    return {'interpretation': 'Chosen marginal sensitivity, not a fit, direction identification or RTT-time model; base4.974ms and waitUniform0to7ms.',
            'bothIndependentDirections': summary(4.974 + d + u + wait),
            'downlinkPausesOnly': summary(4.974 + d + u0 + wait),
            'identicalDirectionalDelaySensitivity': summary(4.974 + 2*d + wait)}


def network(path, batch, params, seeds):
    quality = audit_capture(path)
    epochs, active, timing, pings, windows, populations = [], None, [], [], [], []
    meta = {'connect': 0, 'open': 0, 'close': 0, 'ping-censored': 0, 'socket-error': 0}
    previous_backpressure = 0
    terminal = None
    for rec in sp.read(path):
        if isinstance(rec, dict):
            event = rec.get('event')
            if event in meta:
                meta[event] += 1
            if event in ('connect', 'close'):
                active = None
            if event == 'ping-rtt' and active is not None:
                active['pings'].append([rec['receivedAt'], rec['rttMs']])
                pings.append(rec['rttMs'])
            if event == 'study-timing':
                timing.append(rec)
                stalled = (rec['loop']['maxMs'] > 5 or any(g['duration'] > 5 for g in rec['gc'])
                           or rec['backpressureWrites'] > previous_backpressure
                           or sum(rec['timerGapBins'][6:]) or sum(rec['receiveCostBins'][6:]))
                if stalled:
                    windows.append([rec['since'] - 100, rec['until'] + 100])
                previous_backpressure = rec['backpressureWrites']
            if event in ('capture-end', 'capture-interrupted'):
                terminal = rec
        elif isinstance(rec, sp.Frame):
            populations.extend(sp.u16(p, 4) for p in rec.packets if p[:1] == b'l' and len(p) >= 6)
            if any(p[:1] == b'a' for p in rec.packets):
                active = {'frames': [], 'pings': []}
                epochs.append(active)
            if active is not None:
                active['frames'].append([rec.t, rec.server])
    retained, excluded = [], []
    for index, epoch in enumerate(epochs):
        a = np.asarray(epoch['frames'], dtype=float).reshape((-1, 2))
        legacy = a[50:].copy()
        a = a[a[:, 0] >= a[0, 0] + 1000]
        if len(a) < 1000 or a[-1, 0] - a[0, 0] < 30000 or quality['issues']:
            excluded.append({'epoch': index, 'framesAfterBurnIn': len(a),
                             'reason': 'archive_issues' if quality['issues'] else 'short_epoch'})
            continue
        t, s = a.T
        slope, linear, excess = baseline(t, s)
        clean = clean_mask(t, windows)
        p = np.asarray(epoch['pings'], dtype=float).reshape((-1, 2))
        p = p[(p[:, 0] >= t[0]) & (p[:, 0] <= t[-1])]
        row = dict(t=t, s=s, e=excess, clean=clean, pings=p, slopePpm=slope * 1e6,
                   wallSpanMs=t[-1]-t[0], serverSpanMs=s[-1]-s[0], linear=summary(linear), legacy=legacy)
        retained.append(row)
    observed = {label: features(retained, clean) for label, clean in [('full', False), ('clean', True)]}
    modeled = []
    for seed in seeds:
        variants = {}
        for name in ('fullModel', 'noPauses', 'halfPauseRate', 'halfPauseLength'):
            candidate = list(params)
            if name == 'noPauses': candidate[5] = 0
            if name == 'halfPauseRate': candidate[4] *= .5
            if name == 'halfPauseLength': candidate[5] *= .5
            synthetic, legacy_values = [], []
            for index, row in enumerate(retained):
                t, e = simulate(row['s'], candidate, seed + index)
                synthetic.append({'t': t, 'e': e, 'clean': row['clean']})
                lt, _ = simulate(row['legacy'][:, 1], candidate, seed + index)
                legacy_values.extend(frame_floor_excess(lt, row['legacy'][:, 1]))
            variants[name] = {'full': features(synthetic), 'clean': features(synthetic, True),
                              'parentFrameFloor': summary(legacy_values)}
        modeled.append({'seed': seed, **variants['fullModel'], 'ablations': variants})
    epoch_reports = []
    for row in retained:
        p = row['pings']
        entry = {k: row[k] for k in ('slopePpm', 'wallSpanMs', 'serverSpanMs', 'linear')}
        entry.update(frames=len(row['t']), cleanFrames=int(row['clean'].sum()))
        if len(p):
            e = np.interp(p[:, 0], row['t'], row['e'])
            entry.update(rtt=summary(p[:, 1]), rttMinusDownlinkExcess=summary(p[:, 1] - e),
                         rttDownlinkCorrelation=float(np.corrcoef(p[:, 1], e)[0, 1]),
                         rttAcfByPingLag=acf_frames(p[:, 1]))
        epoch_reports.append(entry)
    legacy_observed = np.concatenate([frame_floor_excess(row['legacy'][:, 0], row['legacy'][:, 1])
                                      for row in retained]) if retained else []
    return dict(batch=batch, path=str(path), sha256=digest(path), quality=quality, metadataCounts=meta,
                timingSeconds=len(timing), terminal=terminal, rtt=summary(pings), epochs=epoch_reports,
                leaderboardPopulation=summary(populations),
                parentFrameFloor=summary(legacy_observed), excludedEpochs=excluded,
                stallWindows=windows, observed=observed, modeled=modeled,
                conditionalRttSensitivity=rtt_sensitivity(params, seeds[0]),
                gatesPerSeed=[{'seed': m['seed'], **{label: gates(observed[label], m[label])
                                                    for label in ('full', 'clean')}} for m in modeled])


def attribute_heads(rows, sources):
    targets = {(r['sha256'], r['conn']): r for r in rows}
    for source in sources:
        world, connection = None, None
        for frame in sp.read(source['path']):
            if not isinstance(frame, sp.Frame):
                continue
            if frame.conn != connection:
                connection, world = frame.conn, sp.World()
            for packet in frame.packets:
                world.feed(packet, frame.server)
            for event in world.events:
                if event[0] != 'spawn' or event[2] != world.self_id:
                    continue
                row = targets.get((source['sha256'], frame.conn))
                if row is None:
                    continue
                own = event[3].base
                candidates = [{'snake_id': snake.id, 'anchor_age_ms': frame.server-snake.anchor_server,
                               'distance': float(np.hypot(own[0]-snake.head(frame.server)[0],
                                                        own[1]-snake.head(frame.server)[1]))}
                              for snake in world.snakes.values() if snake.id != world.self_id and snake.pts]
                row['head_nearest'] = min(candidates, key=lambda x: x['distance']) if candidates else None
            world.events.clear()


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--validation', action='store_true')
    args = p.parse_args()
    kernel = ctypes.windll.kernel32
    kernel.GetCurrentProcess.restype = ctypes.c_void_p
    kernel.SetProcessAffinityMask.argtypes = (ctypes.c_void_p, ctypes.c_size_t)
    if not kernel.SetProcessAffinityMask(kernel.GetCurrentProcess(), 48):
        raise OSError('analysis affinity failed')
    if args.validation:
        seal = json.loads((HERE / 'analysis-seal.json').read_text())
        if seal['sha256'] != digest(__file__):
            raise ValueError('analysis changed since seal')
    plan = json.loads((HERE / 'continuation-plan.json').read_text())
    for pin in plan['sourcePins']:
        if digest(pin['path']) != pin['sha256']:
            raise ValueError('changed frozen source: ' + pin['path'])
    selected = [b for b in plan['batches'] if (b['split'] == 'heldout') == args.validation]
    result_dir = RAW / ('validation' if args.validation else 'development')
    result_dir.mkdir(exist_ok=False)
    params = list(json.loads((OLD / 'model-fit.json').read_text())['parameters'].values())
    results = []
    spawn_paths = []
    for batch in selected:
        directory = RAW / 'raw' / batch['id']
        results.append(network(directory / 'network0.jsonl.gz', batch, params, plan['network']['modelSeeds']))
        spawn_paths.extend([directory / 'network0.jsonl.gz', directory / 'spawn0.jsonl.gz'])
        print(batch['id'], results[-1]['quality']['issues'], results[-1]['observed'], flush=True)
    ecology = module('ecology', ARENA / 'analyze-headless-ecology.py')
    placement = module('placement', ARENA / 'analyze-spawn-placement.py')
    neighbors = module('neighbors', ARENA / 'analyze-spawn-neighbors.py')
    files = []
    audits = []
    for path in spawn_paths:
        quality = audit_capture(path)
        audits.append({'path': str(path), 'quality': quality})
        if quality['issues']:
            continue
        files.append(ecology.analyze_file(path))
    joins, sources = placement.extract([{'files': files}])
    rows, incomplete = neighbors.analyze({'joins': joins, 'sources': sources})
    attribute_heads(rows, sources)
    with (result_dir / 'spawn-neighbors.json').open('x', encoding='utf-8', newline='\n') as f:
        json.dump({'sources': sources, 'rows': rows, 'incomplete': incomplete,
                   'summary': placement.summarize(joins)}, f, indent=2, allow_nan=False)
        f.write('\n')
    with (result_dir / 'network.json').open('x', encoding='utf-8', newline='\n') as f:
        json.dump({'analysisSha256': digest(__file__), 'numpy': np.__version__, 'scipy': scipy.__version__,
                   'results': results, 'allAudits': audits}, f, indent=2, allow_nan=False)
        f.write('\n')
    print('saved', result_dir, 'verified joins', len(joins), 'reconstructed', len(rows), flush=True)


if __name__ == '__main__':
    main()
