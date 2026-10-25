"""Offline, batch-split analysis. No network access or model fitting."""
import os
for key in ('OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS', 'NUMEXPR_NUM_THREADS'):
    os.environ[key] = '1'
import argparse
from bisect import bisect_right
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import sys

import numpy as np
import scipy
from timing import summary, baseline, acf_time, simulate

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / 'tools/fidelity'))
import slither_protocol as sp
from capture_quality import audit_capture


def digest(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()


def merge_windows(windows):
    result = []
    for a, b in sorted(windows):
        if result and a <= result[-1][1]:
            result[-1][1] = max(b, result[-1][1])
        else:
            result.append([a, b])
    return result


class Clean:
    def __init__(self, rows):
        self.windows = []
        previous = 0
        for r in rows:
            if (r['timerLateMaxMs'] > 5 or r['loop']['maxMs'] > 6 or r['receiveMaxMs'] > 5
                    or any(g['duration'] > 5 for g in r['gc'])
                    or r['backpressureWrites'] > previous):
                self.windows.append([r['since'] - 100, r['until'] + 100])
            previous = r['backpressureWrites']
        self.windows = merge_windows(self.windows)
        self.starts = [r[0] for r in self.windows]
        self.coverage = (rows[0]['since'], rows[-1]['until']) if rows else (1, 0)

    def interval(self, a, b):
        if a < self.coverage[0] or b > self.coverage[1]:
            return False
        i = bisect_right(self.starts, b) - 1
        return i < 0 or self.windows[i][1] < a


def tails(t, e, mask, threshold):
    high = (e > threshold) & mask
    starts = np.flatnonzero(high & ~np.r_[False, high[:-1]])
    ends = np.flatnonzero(high & ~np.r_[high[1:], False])
    seconds = sum(float(t[i+1]-t[i]) for i in range(len(t)-1) if mask[i] and mask[i+1]) / 1000
    peaks, durations, brackets = [], [], []
    censored = 0
    for a, b in zip(starts, ends):
        peaks.append(float(e[a:b+1].max()))
        durations.append(float(t[b]-t[a]))
        if a == 0 or b+1 == len(t) or not mask[a-1] or not mask[b+1]:
            censored += 1
        else:
            brackets.append(float(t[b+1]-t[a-1]))
    return dict(episodes=len(starts), seconds=seconds, rate=len(starts)/seconds if seconds else None,
                peaksMs=summary(peaks), durationsMs=summary(durations), bracketsMs=summary(brackets), censored=censored)


def features(epochs, clean=False):
    values, tails_by_epoch, bursts, pairs = [], [], 0, 0
    ac = defaultdict(list)
    for row in epochs:
        t, e = row['t'], row['e']
        mask = row['mask'] if clean else np.ones(len(t), bool)
        if not mask.any():
            continue
        values.extend(e[mask])
        valid = mask[:-1] & mask[1:]
        bursts += int(((np.diff(t) < 1) & valid).sum()); pairs += int(valid.sum())
        for lag, value in acf_time(t, e, mask).items():
            if value is not None:
                ac[lag].append((value, int(mask.sum())))
        tails_by_epoch.append({str(k): tails(t, e, mask, k) for k in (10, 20)})
    ts = {}
    for k in ('10', '20'):
        count = sum(r[k]['episodes'] for r in tails_by_epoch)
        seconds = sum(r[k]['seconds'] for r in tails_by_epoch)
        ts[k] = dict(episodes=count, seconds=seconds, rate=count/seconds if seconds else None)
    return dict(excess=summary(values), burstLt1=bursts/pairs if pairs else None,
                tails=ts, tailsByEpoch=tails_by_epoch,
                acf={k:float(np.average([a for a, _ in v], weights=[b for _, b in v])) for k,v in ac.items()})


def gap_summary(rows):
    return {label: dict(serverMs=summary([r['serverGapMs'] for r in selected]),
                        receiptMs=summary([r['receiptGapMs'] for r in selected]),
                        lt35=sum(r['serverGapMs'] < 35 for r in selected),
                        lt42=sum(r['serverGapMs'] < 42 for r in selected),
                        lt100=sum(r['serverGapMs'] < 100 for r in selected),
                        lt105=sum(r['serverGapMs'] < 105 for r in selected),
                        lt300=sum(r['serverGapMs'] < 300 for r in selected),
                        lt333=sum(r['serverGapMs'] < 333 for r in selected),
                        lt210=sum(r['serverGapMs'] < 210 for r in selected),
                        gt500=sum(r['serverGapMs'] > 500 for r in selected))
            for label, selected in [('full', rows), ('clean', [r for r in rows if r['clean']])]}


def scheduler(frames, clean):
    gaps, witnesses, counts = defaultdict(list), defaultdict(list), Counter()
    world, conn, prior = None, None, {}
    for fi, frame in enumerate(frames):
        if frame.conn != conn or any(p[:1] == b'a' for p in frame.packets):
            world, conn, prior = sp.World(contract=False), frame.conn, {}
        motions, masses = defaultdict(list), defaultdict(list)
        for pi, packet in enumerate(frame.packets):
            if packet[:1] == b'y':
                prior.pop(('prey', sp.u16(packet, 1)), None)
            world.feed(packet, frame.server)
            for event in world.events:
                kind = event[0]
                if kind in ('spawn', 'remove'):
                    for fam in ('motion', 'mass'):
                        prior.pop((fam, event[2]), None)
                family = {'rot':'motion', 'fam':'mass', 'prey_move':'prey'}.get(kind)
                if kind in ('fam', 'tail'): masses[event[2]].append(pi)
                if kind == 'rot': motions[event[2]].append(pi)
                if family is None:
                    continue
                sid = event[2]
                key = family, sid
                speed = event[3]['sp'] if family == 'prey' else None
                if key in prior:
                    previous = prior[key]
                    row = dict(conn=conn, entity=sid, firstFrame=previous['frame'], lastFrame=fi,
                               firstServerMs=previous['s'], lastServerMs=frame.server,
                               serverGapMs=frame.server-previous['s'], receiptGapMs=frame.t-previous['t'],
                               clean=clean.interval(previous['t'], frame.t),
                               own=(sid == world.self_id) if family != 'prey' else None,
                               speed11=(speed == 11 or previous['speed'] == 11) if family == 'prey' else None)
                    gaps[family].append(row)
                    exceptional = row['serverGapMs'] < {'motion':35, 'mass':100, 'prey':300}[family]
                    exceptional |= family == 'prey' and row['serverGapMs'] > 500
                    if exceptional and row['clean'] and len(witnesses[family]) < 8:
                        witnesses[family].append({**row, 'packetHex': packet.hex()})
                prior[key] = dict(s=frame.server, t=frame.t, frame=fi, speed=speed)
            world.events.clear()
        for sid in motions.keys() & masses.keys():
            for m in motions[sid]:
                for h in masses[sid]:
                    label = 'massBeforeMotion' if h < m else 'motionBeforeMass'
                    counts[label] += 1
                    if clean.interval(frame.t, frame.t):
                        counts[label+'Clean'] += 1
                        if label == 'motionBeforeMass' and len(witnesses['reverse']) < 8:
                            witnesses['reverse'].append(dict(conn=conn, frame=fi, serverMs=frame.server,
                                receiptMs=frame.t, entity=sid, packetHex=[p.hex() for p in frame.packets]))
    return dict(gaps={k:gap_summary(v) for k,v in gaps.items()}, ordering=dict(counts), witnesses=dict(witnesses),
                preyBySpeed11={str(flag):gap_summary([r for r in gaps['prey'] if r['speed11'] == flag]) for flag in (True, False)})


def gates(observed, modeled):
    q, m = observed['excess'], modeled['excess']
    if not q['n'] or not m['n']:
        return {'eligible': False}
    return {**{k: abs(m[k]-q[k]) <= max(.5,.15*q[k]) for k in ('p50','p90')},
            'p99': abs(m['p99']-q['p99']) <= .2*q['p99'],
            'burst': abs(modeled['burstLt1']-observed['burstLt1']) <= .005,
            'acf': {k: k in observed['acf'] and k in modeled['acf'] and
                      abs(observed['acf'][k]-modeled['acf'][k]) <= .1
                    for k in ('10','50','100','500','1000')}}


def analyze(path, batch, plan):
    quality = audit_capture(path)
    telemetry, frames, epochs, pings, metadata = [], [], [], [], Counter()
    active = None
    for rec in sp.read(path):
        if isinstance(rec, dict):
            metadata[rec.get('event', 'header')] += 1
            if rec.get('event') == 'study-timing': telemetry.append(rec)
            if rec.get('event') == 'ping-rtt': pings.append(rec['rttMs'])
            if rec.get('event') in ('connect', 'close'): active = None
        elif isinstance(rec, sp.Frame):
            if any(p[:1] == b'a' for p in rec.packets):
                active = []; epochs.append(active)
            if active is not None:
                active.append(rec); frames.append(rec)
    clean = Clean(telemetry)
    kept, reports, excluded = [], [], []
    for i, epoch in enumerate(epochs):
        epoch = [f for f in epoch if f.t >= epoch[0].t+1000]
        if quality['issues'] or len(epoch) < 1000 or epoch[-1].t-epoch[0].t < 30000:
            excluded.append(dict(epoch=i, frames=len(epoch))); continue
        t, s = np.array([[f.t, f.server] for f in epoch]).T
        slope, linear, e = baseline(t, s)
        mask = np.array([clean.interval(float(t[max(0, j-1)]), float(x)) for j, x in enumerate(t)])
        kept.append(dict(t=t, s=s, e=e, mask=mask))
        reports.append(dict(epoch=i, frames=len(t), cleanFrames=int(mask.sum()), slopePpm=slope*1e6,
                            wallSpanMs=float(t[-1]-t[0]), serverSpanMs=float(s[-1]-s[0]), linear=summary(linear)))
    observed = {label:features(kept, flag) for label,flag in [('full',False),('clean',True)]}
    modeled = []
    for seed in plan['network']['modelSeeds']:
        model = []
        for i, row in enumerate(kept):
            t,e = simulate(row['s'], plan['network']['parameters'], seed+i)
            model.append(dict(t=t,e=e,mask=row['mask']))
        m = dict(seed=seed, full=features(model), clean=features(model, True))
        m['gates'] = {label:gates(observed[label],m[label]) for label in ('full','clean')}
        modeled.append(m)
    clean_seconds = observed['clean']['tails']['10']['seconds']
    full_seconds = observed['full']['tails']['10']['seconds']
    eligible = (not quality['issues'] and clean_seconds >= 300 and full_seconds > 0
                and clean_seconds/full_seconds >= .8)
    q = observed['clean']['excess'].get('p99')
    r = observed['clean']['tails']['10']['rate']
    verdict = ('ineligible' if not eligible else 'near-zero' if q <= 3 and r <= .05 else
               'persistent' if q >= 8 or r >= .25 else 'intermediate')
    return dict(batch=batch, sha256=digest(path), quality=quality, metadata=dict(metadata),
                epochs=reports, excludedEpochs=excluded, rtt=summary(pings), stallWindows=clean.windows,
                observed=observed, modeled=modeled, eligibility=eligible, tailClass=verdict,
                scheduler=scheduler(frames, clean) if not quality['issues'] else {'quarantined':True})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--raw', type=Path, required=True)
    parser.add_argument('--validation', action='store_true')
    args = parser.parse_args()
    # Numerical libraries already have one thread. Keep all local work within two logical CPUs.
    if sys.platform == 'win32':
        import ctypes
        kernel = ctypes.windll.kernel32
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.SetProcessAffinityMask.argtypes = (ctypes.c_void_p, ctypes.c_size_t)
        if not kernel.SetProcessAffinityMask(kernel.GetCurrentProcess(), 48):
            raise OSError('analysis affinity failed')
    plan = json.loads((HERE/'plan.json').read_text())
    for pin in plan['sourcePins']:
        if digest(REPO/pin['path']) != pin['sha256']:
            raise ValueError('changed source '+pin['path'])
    if args.validation and not (args.raw/'development.json').exists():
        raise ValueError('complete development analysis first')
    batches = [b for b in plan['batches'] if (b['split']=='heldout') == args.validation]
    results = []
    for batch in batches:
        p = args.raw/batch['round']/batch['id']/'network0.jsonl.gz'
        if p.exists(): results.append(analyze(p,batch,plan))
        else: results.append(dict(batch=batch, missing=True))
    out = args.raw/('validation.json' if args.validation else 'development.json')
    with out.open('x',encoding='utf-8',newline='\n') as stream:
        json.dump(dict(analysisSha256=digest(__file__), numpy=np.__version__, scipy=scipy.__version__,
                       results=results),stream,indent=2,allow_nan=False)
        stream.write('\n')
    print(out)


if __name__ == '__main__':
    main()
