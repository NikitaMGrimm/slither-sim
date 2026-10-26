"""Forecast received arena-radius motion without inferring its hidden target.

Fit only the first two seconds of each nonoverlapping four-second window and
compare predictions against the next two seconds. No future target or position
enters either fit. The constant-acceleration model tests inertia, not a complete
server radius controller. Raw captures and predictions remain local.
"""
import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import statistics

import slither_protocol as sp


def solve(matrix, values):
    rows = [list(row) + [value] for row, value in zip(matrix, values)]
    for column in range(len(values)):
        pivot = max(range(column, len(values)), key=lambda i: abs(rows[i][column]))
        rows[column], rows[pivot] = rows[pivot], rows[column]
        scale = rows[column][column]
        if abs(scale) < 1e-12:
            raise ValueError('singular radius fit')
        rows[column] = [v / scale for v in rows[column]]
        for i in range(len(values)):
            if i != column:
                scale = rows[i][column]
                rows[i] = [a - scale * b for a, b in zip(rows[i], rows[column])]
    return [row[-1] for row in rows]


def fit(points, degree):
    """Polynomial in elapsed seconds; points have time zero near the window."""
    matrix = [[math.fsum(t ** (i + j) for t, _ in points)
               for j in range(degree + 1)] for i in range(degree + 1)]
    values = [math.fsum(r * t ** i for t, r in points) for i in range(degree + 1)]
    return solve(matrix, values)


def evaluate(coefficients, time):
    return math.fsum(c * time ** i for i, c in enumerate(coefficients))


def extract(path):
    rows, complete = [], False
    for frame in sp.read(path):
        if isinstance(frame, dict):
            complete |= frame.get('event') == 'capture-end'
            continue
        if not isinstance(frame, sp.Frame):
            continue
        for packet in frame.packets:
            if packet[:1] == b'z':
                rows.append({'conn': frame.conn, 'S': frame.server,
                             'radius': sp.u24(packet, 1)})
    if not complete:
        raise ValueError('archive has no capture-end marker')
    return {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'radius': rows}


def forecasts(rows):
    groups = defaultdict(list)
    for row in rows:
        groups[row['conn'], row['S'] // 4000].append(row)
    results = []
    for (conn, bucket), points in groups.items():
        start = bucket * 4000
        before = [p for p in points if p['S'] < start + 2000]
        after = [p for p in points if p['S'] >= start + 2000]
        # Only dense broadcasts constrain acceleration at this horizon. Each
        # half must cover most of its interval; reconnects never share a group.
        if len(before) < 6 or len(after) < 6:
            continue
        if any(half[-1]['S'] - half[0]['S'] < 1250 for half in (before, after)):
            continue
        if max(b['S'] - a['S'] for a, b in zip(points, points[1:])) > 350:
            continue
        radius0 = before[0]['radius']
        observations = [((p['S'] - start) / 1000, p['radius'] - radius0)
                        for p in before]
        future = [((p['S'] - start) / 1000, p['radius'] - radius0) for p in after]
        linear = fit(observations, 1)
        quadratic = fit(observations, 2)
        error = lambda coefficients, data: statistics.mean(
            abs(evaluate(coefficients, t) - r) for t, r in data)
        # A stronger constant-speed baseline uses the quadratic fit's current
        # velocity, so any gain is from continuing acceleration, not merely
        # estimating the endpoint velocity better than a linear regression.
        last_time = observations[-1][0]
        velocity = quadratic[1] + 2 * quadratic[2] * last_time
        endpoint = evaluate(quadratic, last_time)
        tangent = [endpoint - velocity * last_time, velocity]
        results.append({'conn': conn, 'start_server_ms': start,
                        'initial_radius': radius0, 'train_points': len(before),
                        'future_points': len(after), 'acceleration': 2 * quadratic[2],
                        'velocity_at_last_observation': velocity,
                        'fit_mae': error(quadratic, observations),
                        'linear_forecast_mae': error(linear, future),
                        'current_velocity_forecast_mae': error(tangent, future),
                        'acceleration_forecast_mae': error(quadratic, future)})
    return results


def summarize(rows):
    def aggregate(selected):
        return {'windows': len(selected), **{
            key: statistics.mean(r[key] for r in selected) if selected else None
            for key in ('linear_forecast_mae', 'current_velocity_forecast_mae',
                        'acceleration_forecast_mae')},
            'acceleration_beats_current_velocity': sum(
                r['acceleration_forecast_mae'] < r['current_velocity_forecast_mae']
                for r in selected)}
    # The subset uses only the fit interval, never the forecast residual.
    stable = [r for r in rows if r['fit_mae'] <= .3]
    return {'all': aggregate(rows), 'stable_fit_only': aggregate(stable),
            'interpretation': 'Equal weight per window; units are radius world units. '
            'A short forecast of inertia does not identify future target changes.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archives', type=Path, nargs='+')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('output exists; choose a fresh path')
    files = []
    for path in args.archives:
        data = extract(path)
        results = forecasts(data['radius'])
        files.append({'path': data['path'], 'sha256': data['sha256'],
                      'summary': summarize(results), 'forecasts': results})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({'files': files}, indent=2) + '\n', encoding='utf-8')
    print(json.dumps([{k: f[k] for k in ('path', 'summary')} for f in files], indent=2))


if __name__ == '__main__':
    main()
