"""Test global minimap/leaderboard proxies against arena-radius movement.

These broadcasts cover more than the observer's nearby snakes. Minimap cells
are binary occupancy, not snake count or mass; leaderboard entries are only the
leaders. Fit on one archive and evaluate unchanged on separate archives.
"""
import argparse
from collections import defaultdict
import hashlib
import importlib.util
import json
import math
from pathlib import Path

import slither_protocol as sp

SPEC = importlib.util.spec_from_file_location('radius_dynamics',
    Path(__file__).with_name('analyze-radius-dynamics.py'))
radius = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(radius)


class Minimap:
    def __init__(self):
        self.size = 0
        self.cells = bytearray()
        self.server = None

    def feed(self, packet, server):
        if packet[:1] == b'M':
            self.size = min(512, sp.u16(packet, 1))
            self.cells = bytearray(self.size ** 2)
            offset, delta = 3, False
        elif packet[:1] == b'V' and self.cells:
            offset, delta = 1, True
        else:
            return
        index = len(self.cells) - 1
        while offset < len(packet) and index >= 0:
            value = packet[offset]
            offset += 1
            if value >= 128:
                if value == 255:
                    if offset >= len(packet):
                        raise ValueError('truncated minimap run')
                    count = 126 * packet[offset]
                    offset += 1
                else:
                    count = value - 128
                index -= count
            else:
                for bit in (64, 32, 16, 8, 4, 2, 1):
                    if index < 0:
                        break
                    if value & bit:
                        self.cells[index] = self.cells[index] ^ 1 if delta else 1
                    index -= 1
        self.server = server


def leaderboard(packet):
    """Return leader segment-mass proxies without retaining player names."""
    masses, offset = [], 6
    while offset < len(packet):
        if offset + 7 > len(packet):
            raise ValueError('truncated leaderboard entry')
        masses.append(sp.u16(packet, offset) + sp.u24(packet, offset + 2) / 16777215)
        offset += 7 + packet[offset + 6]
        if offset > len(packet):
            raise ValueError('truncated leaderboard nickname')
    return {'players': sp.u16(packet, 4), 'leader_count': len(masses),
            'leader_mass': sum(masses), 'largest_mass': max(masses, default=0)}


def extract(path):
    conn, grid, rank, rank_time = None, None, None, None
    rows, complete = [], False
    for frame in sp.read(path):
        if isinstance(frame, dict):
            complete |= frame.get('event') == 'capture-end'
            continue
        if not isinstance(frame, sp.Frame):
            continue
        if frame.conn != conn:
            conn, grid, rank, rank_time = frame.conn, Minimap(), None, None
        for packet in frame.packets:
            if packet[:1] in (b'M', b'V'):
                grid.feed(packet, frame.server)
            elif packet[:1] == b'l':
                rank, rank_time = leaderboard(packet), frame.server
            elif packet[:1] == b'z' and rank is not None:
                rows.append({'conn': conn, 'S': frame.server, 'radius': sp.u24(packet, 1),
                             **rank, 'rank_age_ms': frame.server - rank_time,
                             'map_age_ms': frame.server - grid.server if grid.server is not None else None,
                             'map_fraction': sum(grid.cells) / len(grid.cells) if grid.cells else None,
                             'map_size': grid.size})
    if not complete:
        raise ValueError('archive has no capture-end marker')
    return {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'radius': rows}


def windows(report):
    groups = defaultdict(list)
    for row in report['radius']:
        groups[row['conn']].append(row)
    samples = []
    for rows in groups.values():
        first, buckets = rows[0]['S'], {}
        for row in rows:
            buckets.setdefault((row['S'] - first) // 5000, row)
        for i, row in buckets.items():
            before, after = buckets.get(i - 1), buckets.get(i + 1)
            if before is None or after is None:
                continue
            if not (3500 <= row['S'] - before['S'] <= 6500 and
                    3500 <= after['S'] - row['S'] <= 6500):
                continue
            if row['rank_age_ms'] > 3500 or row['map_age_ms'] is None or row['map_age_ms'] > 5000:
                continue
            delta = after['radius'] - row['radius']
            if not delta:
                continue
            samples.append({**row, 'direction': 1 if delta > 0 else -1,
                            'previous_direction': 1 if row['radius'] > before['radius'] else -1})
    return samples


def compare(train, test):
    if not train or not test:
        return {'train_windows': len(train), 'test_windows': len(test), 'models': None}
    results = {'last_direction': {'train': radius.score(train, lambda r: r['previous_direction']),
                                 'test': radius.score(test, lambda r: r['previous_direction'])}}
    fractions = sorted({r['map_fraction'] for r in train})
    candidates = [(k, lambda r, k=k: 1 if r['map_fraction'] > k else -1)
                  for k in [fractions[0] * .99] + [(a+b)/2 for a,b in zip(fractions, fractions[1:])] + [fractions[-1] * 1.01]]
    threshold, predictor = radius.choose(train, candidates)
    results['map_density_threshold'] = {'threshold': threshold, 'train': radius.score(train, predictor),
                                        'test': radius.score(test, predictor)}
    for key in ('players', 'map_fraction', 'leader_mass', 'largest_mass'):
        # A finite threshold search fitted only to training states. No future
        # radius, test labels, or per-connection phase sets the target.
        ratios = sorted({r['radius'] / math.sqrt(r[key]) for r in train if r[key] > 0})
        if not ratios:
            continue
        candidates = [(k, lambda r, k=k: 1 if k * math.sqrt(r[key]) > r['radius'] else -1)
                      for k in [ratios[0] * .99] + [(a + b) / 2 for a, b in zip(ratios, ratios[1:])] + [ratios[-1] * 1.01]]
        coefficient, predictor = radius.choose(train, candidates)
        results[key] = {'coefficient': coefficient, 'train': radius.score(train, predictor),
                        'test': radius.score(test, predictor)}
    return {'train_windows': len(train), 'test_windows': len(test), 'models': results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('train', type=Path)
    parser.add_argument('test', type=Path, nargs='+')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('output exists; choose a fresh path')
    training = extract(args.train)
    tests = [extract(path) for path in args.test]
    result = {'training': training, 'validation': tests,
              'comparisons': [compare(windows(training), windows(test)) for test in tests],
              'limits': ['Minimap binary occupancy is neither global mass nor a complete body census',
                         'Leader mass is saturated/censored at protocol limits and omits other snakes',
                         'Adjacent windows are serially dependent; coefficient search can overfit training']}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result['comparisons'], indent=2))


if __name__ == '__main__':
    main()
