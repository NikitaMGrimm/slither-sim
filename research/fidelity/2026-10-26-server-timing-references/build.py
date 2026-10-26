"""Offline derivatives of #111's completed streams; no collection or fitting.

See REPORT.md for selection, clock definitions and limits. Output must be new.
"""
import argparse
from bisect import bisect_right
from collections import defaultdict
import json
from pathlib import Path
import statistics
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
STUDY = ROOT / "research/fidelity/2026-10-26-vps-timing"
sys.path.insert(0, str(ROOT / "tools/fidelity"))
import transport_parity as tp
from capture_quality import audit_capture

sp = tp.sp


class Clean:
    """The frozen study mask, including inclusive boundaries and coverage."""
    def __init__(self, windows, coverage):
        self.windows, self.coverage = windows, coverage
        self.starts = [a for a, _ in windows]

    def interval(self, a, b):
        if a < self.coverage[0] or b > self.coverage[1]:
            return False
        i = bisect_right(self.starts, b) - 1
        return i < 0 or self.windows[i][1] < a


def gap_summary(values):
    return dict(n=len(values), min=min(values) if values else None,
                median=statistics.median(values) if values else None,
                p95=tp.quantiles(values)["p95"], max=max(values) if values else None)


def select_witnesses(frames, clean):
    """Minimum clean held-out motion/mass gaps, reverse pair, maximum prey gap."""
    previous, appearances, selected = {}, {}, {}
    world, conn, setup = None, None, None
    for fi, frame in enumerate(frames):
        if frame.conn != conn or any(p[:1] == b'a' for p in frame.packets):
            world, conn, previous, appearances = sp.World(contract=False), frame.conn, {}, {}
        motions, masses = {}, {}
        for pi, packet in enumerate(frame.packets):
            if packet[:1] == b'a':
                setup = packet.hex()
            if packet[:1] == b'y':
                pid = sp.u16(packet, 1)
                previous.pop(('prey', pid), None)
                if len(packet) > 5:
                    appearances[('prey', pid)] = packet.hex()
            world.feed(packet, frame.server)
            for event in world.events:
                kind, sid = event[0], event[2] if len(event) > 2 else None
                if kind in ('spawn', 'remove'):
                    for family in ('motion', 'mass'):
                        previous.pop((family, sid), None)
                        if kind == 'spawn':
                            appearances[(family, sid)] = packet.hex()
                if kind == 'rot': motions[sid] = pi
                if kind in ('fam', 'tail'): masses[sid] = pi
                family = {'rot': 'motion', 'fam': 'mass', 'prey_move': 'prey'}.get(kind)
                if family is None:
                    continue
                key = family, sid
                current = dict(frame=fi, packet=pi, serverMs=frame.server,
                               receiptMs=frame.t, packetHex=packet.hex())
                old = previous.get(key)
                if old and clean.interval(old['receiptMs'], frame.t):
                    gap = frame.server - old['serverMs']
                    better = (family not in selected or
                              (gap > selected[family]['serverGapMs'] if family == 'prey' else
                               gap < selected[family]['serverGapMs']))
                    if better:
                        selected[family] = dict(kind=family, connection=conn, entity=sid,
                            setupHex=setup, appearanceHex=appearances[key], first=old, last=current,
                            serverGapMs=gap, receiptGapMs=frame.t-old['receiptMs'],
                            interveningFrames=fi-old['frame']-1, clean=True)
                previous[key] = current
            world.events.clear()
        for sid in motions.keys() & masses.keys():
            if motions[sid] < masses[sid] and clean.interval(frame.t, frame.t) and 'reverse' not in selected:
                selected['reverse'] = dict(kind='reverse', connection=conn, entity=sid,
                    setupHex=setup, appearanceHex=appearances[('motion', sid)], frame=fi,
                    serverMs=frame.server, receiptMs=frame.t,
                    packetIndices=[motions[sid], masses[sid]],
                    packetHex=[frame.packets[i].hex() for i in (motions[sid], masses[sid])])
    return selected


def cadence(frames, clean):
    values, prior, conn, known = defaultdict(list), {}, None, set()
    for frame in frames:
        if frame.conn != conn or any(p[:1] == b'a' for p in frame.packets):
            prior, conn, known = {}, frame.conn, set()
        for packet in frame.packets:
            command = packet[:1]
            if command == b'y':
                pid = sp.u16(packet, 1)
                prior.pop(('prey', pid), None)
                if len(packet) > 5: known.add(pid)
                else: known.discard(pid)
            if command not in (b'j', b'l', b'M', b'V'):
                continue
            if command == b'j' and sp.u16(packet, 1) not in known:
                continue
            family = 'prey' if command == b'j' else 'leaderboard' if command == b'l' else 'minimap'
            key = family, sp.u16(packet, 1) if command == b'j' else None
            if key in prior:
                old = prior[key]
                if clean.interval(old[0], frame.t):
                    values[family].append(frame.server-old[1])
            prior[key] = frame.t, frame.server
    return values


def compact(summary):
    return {k: v for k, v in summary.items() if k not in ('packetFamilies', 'headerDeltaHistogramMs')}


def build(raw, out):
    pins = json.loads((STUDY / 'source-pins.json').read_text())['secondContinuation']
    selected = json.loads((STUDY / 'selected-results.json').read_text())['secondContinuation']
    sources = {r['path']: r for r in pins['vpsFiles']}
    # Verify full completed raw files and the frozen development/validation outputs.
    for name in ('development.json', 'validation.json'):
        pin = next(r for r in pins['artifacts'] if Path(r['path']).name == name)
        if tp.pc.sha256(raw / name) != pin['sha256']:
            raise ValueError('analysis hash mismatch: ' + name)
    captures, full_captures = [], []
    total, full_total, all_cadence, witnesses = tp.Samples(), tp.Samples(), defaultdict(list), []
    for row in selected['batches']:
        batch = row['batch']
        name = f"{batch['round']}/{batch['id']}/network0.jsonl.gz"
        path = raw / name
        if tp.pc.sha256(path) != sources[name]['sha256']:
            raise ValueError('raw hash mismatch: ' + name)
        audit = audit_capture(path)
        if audit['issues'] or not row['eligibility']:
            raise ValueError('inadmissible completed batch: ' + batch['id'])
        frames, telemetry, active = [], [], False
        for item in sp.read(path):
            if isinstance(item, dict):
                if item.get('event') == 'study-timing': telemetry.append(item)
                if item.get('event') in ('connect', 'close'): active = False
            elif isinstance(item, sp.Frame):
                if any(p[:1] == b'a' for p in item.packets): active = True
                if active: frames.append(item)
        clean = Clean(row['stallWindows'], (telemetry[0]['since'], telemetry[-1]['until']))
        admitted = {r['epoch'] for r in row['epochs']}
        epoch_number = -1

        def selection(epoch):
            nonlocal epoch_number
            epoch_number += 1
            if epoch_number not in admitted:
                return lambda a, b: False
            return lambda a, b: a >= epoch.setup_receipt + 1000 and clean.interval(a, b)

        report, samples = tp.measure_capture(path, selection)
        report['path'], report['batch'], report['split'] = name, batch['id'], batch['split']
        report['connectionSetupEpochs'] = len(report.pop('connections'))
        report['summary'] = compact(report['summary'])
        # Retain the masked development/held-out characterization separately
        # from the default gate's symmetric, unfiltered six-batch corpus.
        captures.append(report)
        if batch['split'] == 'development': total.merge(samples)
        full, full_samples = tp.measure_capture(path)
        full['path'], full['batch'], full['split'] = name, batch['id'], batch['split']
        full['connectionSetupEpochs'] = len(full.pop('connections'))
        full['summary'] = compact(full['summary'])
        full_captures.append(full)
        full_total.merge(full_samples)
        gaps = cadence(frames, clean)
        for key, values in gaps.items(): all_cadence[key].extend(values)
        if batch['split'] == 'heldout':
            for witness in select_witnesses(frames, clean).values():
                witness['source'] = dict(archive=name, archiveSha256=report['sha256'], batch=batch['id'])
                witnesses.append(witness)
        print('Verified and measured ' + batch['id'], flush=True)
    provenance = dict(study='research/fidelity/2026-10-26-vps-timing/REPORT.md',
        sourcePinsSha256=tp.pc.sha256(STUDY / 'source-pins.json'),
        selectedResultsSha256=tp.pc.sha256(STUDY / 'selected-results.json'),
        builderSha256=tp.pc.sha256(Path(__file__)), decoderSha256=tp.pc.sha256(Path(sp.__file__)))
    reference = dict(schemaVersion=1, wireVersion=19, clientContract=None, status='complete',
        dataOrigin='captured', summaryOnly=True, provenance=provenance,
        toolSha256=tp.pc.sha256(Path(tp.__file__)), censusSha256=tp.pc.sha256(Path(tp.pc.__file__)),
        decoderSha256=provenance['decoderSha256'],
        selection='All six completed #111 second-continuation batches, unfiltered: same measurement as candidates, without telemetry masking, epoch admission or burn-in. Failed attempts excluded.',
        definitions=dict(delay='Original transport metric: centered +/-600-post-setup-frame minimum; no slope correction. Not #111 excess gauge.',
            limits='False-reject control is only the fixed allowance LOO check over six captures; batch drift observed. No population error-rate guarantee, independent validation, renderer or one-way delay claim.'),
        captures=full_captures, excluded=[], summary=compact(full_total.summary()),
        maskedSelection='First-attempt server characterization: #111 admitted epochs, 1 s burn-in and frozen clean telemetry mask. Whole gap/ten-anchor windows must be clean.',
        maskedDevelopmentCaptures=[r for r in captures if r['split'] == 'development'],
        maskedDevelopmentSummary=compact(total.summary()),
        heldoutCaptures=[r for r in captures if r['split'] == 'heldout'])
    cadence_fixture = dict(schemaVersion=1, wireProtocol=19, clientContract=None, dataOrigin='captured',
        provenance=provenance, sources=[dict(archive=r['path'], archiveSha256=r['sha256'], split=r['split']) for r in captures],
        selection='All six completed batches; consecutive same-prey j, l and combined M/V gaps; reset at setup/connection and prey y; whole intervals pass frozen clean telemetry mask. No metric-value filtering.',
        limits='Descriptive correlated server-header gaps, including post-death tail; fixed simulator cadences are models, not universal timers.',
        **{key+'UpdateGapsMs' if key == 'prey' else key+'GapsMs': gap_summary(values) for key, values in all_cadence.items()})
    # Reduce across both held-out batches; stable ties keep the first batch/frame.
    cases = []
    for kind in ('motion', 'mass', 'reverse', 'prey'):
        choices = [w for w in witnesses if w['kind'] == kind]
        cases.append(choices[0] if kind == 'reverse' else
                     (max if kind == 'prey' else min)(choices, key=lambda w: w['serverGapMs']))
    witness_fixture = dict(schemaVersion=1, wireProtocol=19, clientContract='slither-client-v2', dataOrigin='captured',
        provenance=provenance, selection='Held-out clean minimum motion/mass gaps, first reverse pair, maximum repeat-prey gap; ties first batch/frame.',
        limits='Selected logical packets with setup/appearance context, not complete wire frames or simulator cadence requirements. Frame indices are zero-based post-setup across the archive; packet indices zero-based. Intervening traffic checked in full raw stream; omitted here.', cases=cases)
    out.mkdir(parents=True, exist_ok=False)
    for name, value in [('transport-official-near-server.json', reference),
                        ('client-reporting-cadence-near-server.json', cadence_fixture),
                        ('scheduler-near-server-witnesses.json', witness_fixture)]:
        (out / name).write_text(json.dumps(value, indent=2, allow_nan=False)+'\n', encoding='utf-8', newline='\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--raw', type=Path, required=True, help='Existing #111 second-continuation raw copy')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    build(args.raw, args.out)
