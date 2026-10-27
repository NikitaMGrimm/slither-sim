"""Offline frame-gap and temporary host-trace summaries (milliseconds)."""
import argparse
import csv
from collections import Counter, defaultdict, deque
import hashlib
import gzip
import json
import math
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/fidelity'))
import slither_protocol as sp


def summary(values):
    xs = sorted(values)
    if not xs:
        return {'n': 0}
    return dict(n=len(xs), **{f'p{p}': xs[math.ceil(len(xs)*p/100)-1] for p in (50,90,99)},
                min=xs[0], max=xs[-1])


def read_frames(path):
    with gzip.open(path,'rt') as f:
        first = json.loads(next(f))
    if not isinstance(first,dict) or 'seconds' not in first:
        yield from sp.read(path)
        return
    # #177's one-session catalog archive records reconstructed WS messages
    # alongside TCP/wire events. Admit only complete binary receive messages.
    clock = None
    with gzip.open(path,'rt') as f:
        for line in f:
            record = json.loads(line)
            if record.get('event') != 'receive' or record.get('opcode') != 2:
                continue
            data = bytes.fromhex(record['hex'])
            packets = sp.split(data)
            h = int.from_bytes(data[:2],'big')
            clock = 0 if clock is None or any(p[:1] == b'a' for p in packets) else clock+h
            yield sp.Frame(1,record['seconds']*1000,clock,packets)


def capture(path, burn=5000):
    receipt, header, pairs, previous, setup = [], [], [], None, None
    events, setups = [], 0
    for frame in read_frames(path):
        if not isinstance(frame, sp.Frame):
            if isinstance(frame, dict) and frame.get('type') == 'meta':
                events.append(frame)
            continue
        if previous is not None and previous.conn != frame.conn:
            setup, previous = None, None
        if any(p[:1] == b'a' for p in frame.packets):
            setup, previous = frame.t, None
            setups += 1
        if (setup is not None and previous and previous.conn == frame.conn
                and previous.t >= setup + burn):
            r, h = frame.t - previous.t, frame.server - previous.server
            receipt.append(r)
            header.append(h)
            # Only originally adjacent eligible frame intervals form a pair.
            pairs.append((r, h, frame.t))
        previous = frame if setup is not None else None
    n = len(receipt)
    adjacent = [(a,b) for a,b in zip(pairs,pairs[1:])
                if abs(a[2] + b[0] - b[2]) < .002]
    bursts = sum(a[0] < 1 and b[0] > 14 for a,b in adjacent)
    return dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest(), setups=setups,
        receipt=summary(receipt), header=summary(header), gapMinusHeader=summary([r-h for r,h in zip(receipt,header)]),
        absGapMinusHeader=summary([abs(r-h) for r,h in zip(receipt,header)]),
        over14=sum(r>14 for r in receipt), over14Percent=100*sum(r>14 for r in receipt)/n if n else None,
        under1=sum(r<1 for r in receipt), burstPairs=bursts, eligibleAdjacentPairs=len(adjacent),
        burstPercent=100*bursts/max(1,len(adjacent)), collectorFinal=events[-1] if events else None)


def traces(root):
    result = {}
    parsed = {}
    for kind in ('simulation','release','early','gateway','admission'):
        path = root / f'trace-{kind}.csv'
        if not path.exists():
            continue
        rows = [tuple(map(float,row)) for row in csv.reader(path.open()) if float(row[0]) >= 6]
        parsed[kind] = rows
        result[kind] = dict(sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
            lateness=summary([(r[1]-r[0])*1000 for r in rows]))
        if kind == 'simulation':
            result[kind]['advanceIncludingEncode'] = summary([(r[3]-r[2])*1000 for r in rows])
            result[kind]['commandAndAdmission'] = summary([(r[2]-r[1])*1000 for r in rows])
        if kind == 'gateway':
            result[kind]['pollIntervals'] = summary([(b[0]-a[0])*1000 for a,b in zip(rows,rows[1:])])
    early = defaultdict(deque)
    for due, at, *_ in parsed.get('early', []):
        early[due].append(at)
    after = []
    for due, at, *_ in parsed.get('release', []):
        if early[due]:
            after.append((at-early[due].popleft())*1000)
    result['handoffToRelease'] = summary(after)
    ticks = {round(r[0],6):r for r in parsed.get('simulation', [])}
    causes = Counter()
    for due, at, *_ in parsed.get('early', []):
        if at-due <= .004:
            continue
        tick = ticks.get(round(due,6))
        if tick is None:
            causes['unmatched'] += 1
            continue
        stages = {'wake':tick[1]-tick[0], 'commandsAdmission':tick[2]-tick[1],
                  'advanceBeforeHandoff':at-tick[2]}
        causes[max(stages,key=stages.get)] += 1
    result['over4msEarlyLargestStage'] = dict(causes)
    path = root/'trace-encode.csv'
    if path.exists():
        rows = [tuple(map(float,row)) for row in csv.reader(path.open()) if float(row[0]) >= 6]
        result['broadcastEncode'] = dict(sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                                         duration=summary([r[1]*1000 for r in rows]))
    return result


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('paths', nargs='+', type=Path)
    p.add_argument('--burn-ms', type=float, default=5000)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    result = []
    for path in args.paths:
        if path.is_dir():
            result.append(dict(run=json.loads((path/'run.json').read_text()),
                capture=capture(path/'capture/bot0.jsonl.gz', args.burn_ms), traces=traces(path),
                host=(path/'host.log').read_text()))
        else:
            result.append(dict(capture=capture(path, args.burn_ms)))
    with args.out.open('x') as f:
        json.dump(result, f, indent=2)
        f.write('\n')
    for r in result:
        c=r['capture']
        print(Path(c['path']).parent, json.dumps({k:c[k] for k in ('receipt','header','absGapMinusHeader','over14Percent','burstPercent')}))
        if 'traces' in r:
            print(json.dumps(r['traces']))
