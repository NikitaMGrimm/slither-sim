"""Per-tick user instructions; raw rows stay in the output folder."""
import argparse, gzip, hashlib, json, os, statistics, struct, subprocess
from pathlib import Path

if not __debug__:
    raise SystemExit('run without -O: validation requires asserts')
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('output', type=Path)
p.add_argument('--cpu', type=int, default=1)
a = p.parse_args()
os.sched_setaffinity(0, {a.cpu})
r = a.output.resolve()
out = r / 'mapticks'
out.mkdir()
cases = ['official500-legacy', 'official500-fullmap5',
         'official500-fullmap250', 'official500-fullmap']
arms = ['base', 'candidate']
expected = json.loads((r / 'run/traces/selected.json').read_text())
pins = {arm: json.loads((r / 'run' / ('native-' + arm) / 'pins.json').read_text()) for arm in arms}

def binary(arm):
    return r / 'run' / ('native-' + arm) / 'bin/study_protocols'

def rehash():
    for arm in arms:
        assert hashlib.sha256(binary(arm).read_bytes()).hexdigest() == pins[arm]['binarySha256']

# Legacy frames have two elapsed bytes; derive map ticks from complete traces.
due, map_frames = set(), {}
for case, full in zip(cases, [0, 5, 250, 500]):
    tick, slot = None, None
    counts = {'U': 0, 'MV': 0}
    with gzip.open(r / 'run/traces' / (case + '-base.bin.gz'), 'rb') as f:
        while header := f.read(16):
            tag, size = struct.unpack('<QQ', header)
            data = f.read(size)
            assert len(data) == size
            if tag == 1:
                tick, slot, _ = struct.unpack('<QQQ', data)
            elif tag == 2 and data[2] in b'MVU':
                assert (data[2] == ord('U')) == (slot < full)
                counts['U' if slot < full else 'MV'] += 1
                if case == cases[0]:
                    due.add(tick)
    assert sum(counts.values()) == expected[case]['summary']['maps']
    assert (counts['U'] > 0) == (full > 0)
    assert (counts['MV'] > 0) == (full < 500)
    map_frames[case] = counts
due = sorted(t for t in due if t >= 120 and t != 720)
assert due
rehash()
rows, ids = [], None
for pair in range(-1, 5):
    for arm in (arms if pair < 0 or pair % 2 == 0 else arms[::-1]):
        for case in cases[pair % 4:] + cases[:pair % 4]:
            path = out / f'{pair}-{arm}-{case}.json'
            with path.with_suffix('.log').open('x') as log:
                subprocess.run([str(binary(arm)), case], stdout=log,
                               stderr=subprocess.STDOUT, check=True,
                               env=dict(os.environ, RAYON_NUM_THREADS='1', STUDY_TICK_COUNTS=str(path)))
            assert json.loads(path.with_suffix('.log').read_text().splitlines()[-1]) == expected[case]['summary']
            post_warmup = [v for v in json.loads(path.read_text()) if v[0] >= 120]
            assert len(post_warmup) == 960
            assert all(v[2] > 0 and v[3] / v[2] >= .99 for v in post_warmup)
            data = [v for v in post_warmup if v[0] != 720 and v[4] == 500]
            assert max(v[1] for v in post_warmup) == max(v[1] for v in data)
            ticks = [v[0] for v in data]
            ids = ids or ticks
            assert ticks == ids and len(data) == 853
            assert all(v[2] > 0 and v[3] / v[2] >= .99 for v in data)
            maps = [v[1] for v in data if v[0] in due]
            ordinary = sorted(v[1] for v in data if v[0] not in due)
            assert len(maps) == len(due)
            row = {'pair': pair, 'arm': arm, 'case': case,
                   'dueMean': statistics.mean(maps), 'dueMax': max(maps),
                   'ordinaryMean': statistics.mean(ordinary),
                   'selectedMax': max(v[1] for v in data)}
            rows.append(row)
            (out / 'rows.json').write_text(json.dumps(rows, indent=2) + '\n')
            print('TICKS', pair, arm, case, flush=True)
selected = {'ticks': len(ids), 'mapDueTicks': due, 'mapFrames': map_frames, 'pairs': 5, 'cases': {}}
metrics = ['dueMean', 'dueMax', 'ordinaryMean', 'selectedMax']
for case in cases:
    group = {arm: [v for v in rows if v['pair'] >= 0 and v['case'] == case and v['arm'] == arm] for arm in arms}
    selected['cases'][case] = {}
    for metric in metrics:
        ratios = [b[metric] / a[metric] for a, b in zip(group['base'], group['candidate'])]
        selected['cases'][case][metric] = {
            **{arm + 'Median': statistics.median(v[metric] for v in group[arm]) for arm in arms},
            'pairedMedian': statistics.median(ratios), 'pairedRange': [min(ratios), max(ratios)]}
(out / 'selected.json').write_text(json.dumps(selected, indent=2) + '\n')
rehash()
print('PASS map-due pairs', flush=True)
