"""Independent minimal byte audit; no gameplay/world decoder imports."""
import argparse
import base64
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path


def packets(raw):
    payload = raw[2:]
    if payload[0] >= 32:
        return [payload]
    result = []
    cursor = 0
    while cursor < len(payload):
        first = payload[cursor]
        if first < 32:
            size = int.from_bytes(payload[cursor:cursor+2], 'big')
            cursor += 2
        else:
            size = first-32
            cursor += 1
        assert size > 0 and cursor+size <= len(payload)
        result.append(payload[cursor:cursor+size])
        cursor += size
    assert cursor == len(payload)
    return result


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--source', type=Path, required=True)
    ap.add_argument('--study', type=Path, required=True)
    args = ap.parse_args()
    results = json.loads((args.study/'results.json').read_text())
    root = args.source/'recordings'/'headless'
    file_results = []
    endpoints = set()
    duplicate_times = Counter()
    total_radius, total_boards = 0, 0
    for f in results['files']:
        if not f['eligible'] or f['duplicate']:
            continue
        original = json.loads((args.study/'series'/Path(f['archive']).with_suffix('.json')).read_text())
        expected_radius = [(c['conn'], r['S'], r['incoming_frame'], r['radius'])
                           for c in original['connections'] for r in c['radius']]
        expected_boards = [(c['conn'], r['S'], r['incoming_frame'], r['players'])
                           for c in original['connections'] for r in c['leaderboards']]
        conn, clock, frames, radius, boards = 0, None, 0, [], []
        last_at = {}
        valid = False
        source_path = root/f['archive']
        assert hashlib.sha256(source_path.read_bytes()).hexdigest() == f['sha256']
        with gzip.open(source_path, 'rt', encoding='utf-8') as stream:
            for line in stream:
                record = json.loads(line)
                if isinstance(record, dict):
                    if record.get('event') == 'connect':
                        conn += 1
                        clock, valid = None, False
                        server = record.get('server', {})
                        # Never export an endpoint; retain only the count.
                        endpoints.add((server.get('ip'), server.get('port')))
                    continue
                if record[0] != 'i':
                    continue
                frames += 1
                raw = base64.b64decode(record[2], validate=True)
                logical = packets(raw)
                clock = 0 if clock is None or any(p[0] == 97 for p in logical) else clock+int.from_bytes(raw[:2], 'big')
                for p in logical:
                    if p[0] == 97:
                        valid = len(p) >= 25 and p[23] == 19
                        if valid:
                            value = int.from_bytes(p[27:30], 'big') if len(p) >= 30 else .98*int.from_bytes(p[1:4], 'big')
                            radius.append((conn, clock, frames, value))
                    elif valid and p[0] == 122:
                        assert len(p) == 4
                        value = int.from_bytes(p[1:], 'big')
                        radius.append((conn, clock, frames, value))
                    elif valid and p[0] == 108:
                        boards.append((conn, clock, frames, int.from_bytes(p[4:6], 'big')))
        assert radius == expected_radius, f['archive']
        assert boards == expected_boards, f['archive']
        for co, time, frame, value in radius:
            key = (co, time)
            if key in last_at:
                duplicate_times['equal' if last_at[key] == value else 'conflict'] += 1
            last_at[key] = value
        total_radius += len(radius)
        total_boards += len(boards)
        file_results.append({'archive': f['archive'], 'radius_values_equal': len(radius),
                             'leaderboard_counts_equal': len(boards)})
    output = {'status': 'pass', 'independent_method': 'standalone base64/framing and int.from_bytes; no decoder import',
              'archives': len(file_results), 'radius_values': total_radius, 'leaderboard_values': total_boards,
              'distinct_endpoint_pairs': len(endpoints), 'duplicate_radius_timestamps': dict(duplicate_times),
              'files': file_results}
    with (args.study/'verification.json').open('x', encoding='utf-8', newline='\n') as stream:
        json.dump(output, stream, indent=2)
        stream.write('\n')
    print(json.dumps({k: v for k, v in output.items() if k != 'files'}, indent=2))


if __name__ == '__main__':
    main()
