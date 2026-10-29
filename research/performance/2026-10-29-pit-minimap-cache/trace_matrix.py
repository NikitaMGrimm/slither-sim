"""Portable complete-byte parity checks; no timing results are selected.

Usage: python trace_matrix.py REPO OUTPUT [catalog|host|battledome|mutation]
Uses the existing catalog trace validator and the earlier protocol fixture's
record format. Retains failed/truncated raw output and every successful trace.
"""
from pathlib import Path
import gzip
import hashlib
import json
import os
import struct
import subprocess
import sys

repo, root = map(lambda p: Path(p).resolve(), sys.argv[1:3])
mode = sys.argv[3]
os.environ['RAYON_NUM_THREADS'] = '1'
sys.path.insert(0, str(repo / 'tools'))
from perf_baseline import capture_trace

suffix = '.exe' if os.name == 'nt' else ''
rows = []


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def executable(arm, binary):
    path = root / arm / 'bin' / (binary + suffix)
    pins = json.loads((root / arm / 'pins.json').read_text())
    assert sha(path) == pins['binaries'][path.name]
    return path


def equal(a, b):
    with gzip.open(a, 'rb') as x, gzip.open(b, 'rb') as y:
        while True:
            left, right = x.read(1 << 20), y.read(1 << 20)
            if left != right:
                return False
            if not left:
                return True


def host(arm, case):
    path = root / arm / f'{case}.bin'
    errors = path.with_suffix('.stderr')
    with path.open('xb') as out, errors.open('xb') as err:
        subprocess.run([str(executable(arm, 'study_protocols')), case],
                       stdout=out, stderr=err, check=True, timeout=600)
    summary = json.loads(errors.read_text().splitlines()[-1])
    states, frames, size, wire = 0, 0, 0, hashlib.sha256()
    with path.open('rb') as stream, gzip.open(path.with_suffix('.gz'), 'wb', compresslevel=1) as packed:
        while header := stream.read(16):
            assert len(header) == 16
            tag, length = struct.unpack('<QQ', header)
            data = stream.read(length)
            assert len(data) == length
            packed.write(header); packed.write(data)
            if tag in [0, 3]:
                assert json.loads(data)['state']; states += 1
            elif tag == 1:
                tick, slot, _ = struct.unpack('<QQQ', data)
                assert tick < 1080 and slot < summary['seats']
            elif tag == 2:
                frames += 1; size += length
                wire.update(header); wire.update(data)
            elif tag == 4:
                assert json.loads(data) == summary
            else:
                raise AssertionError(tag)
    assert tag == 4 and states == 2 and frames == summary['frames'] and size == summary['bytes']
    # Recheck the immutable copied executable after the complete run.
    executable(arm, 'study_protocols')
    return {'sha256': sha(path), 'bytes': path.stat().st_size,
            'wireSha256': wire.hexdigest(), 'frames': frames}


if mode == 'catalog':
    catalogs = [json.loads(subprocess.check_output([str(executable(a, 'perf_baseline')), '--list']))
                for a in ['base', 'candidate']]
    assert catalogs[0] == catalogs[1]
    for case in catalogs[0]['workloads']:
        name = case['name']
        if not case['available'] or name.startswith('host-'):
            continue
        row = {'case': name}
        for arm in ['base', 'candidate']:
            row[arm] = capture_trace(executable(arm, 'perf_baseline'), name,
                                     case['traceIterations'], root / arm,
                                     terminal=name.startswith(('official-', 'sandbox-', 'battledome-', 'client-')))
            executable(arm, 'perf_baseline')
        assert row['base'] == row['candidate'], name
        assert equal(root / 'base' / f'{name}.jsonl.gz', root / 'candidate' / f'{name}.jsonl.gz'), name
        rows.append(row)
        (root / 'catalog-parity.json').write_text(json.dumps(rows, indent=2) + '\n')
        print('PARITY', name, flush=True)
elif mode in ['host', 'battledome', 'mutation']:
    cases = ['battledome8', 'battledome8-legacy']
    if mode == 'host':
        cases = ['official500', 'official500-half', 'official500-legacy', 'official2281'] + cases
    for case in cases:
        row = {'case': case}
        if mode != 'mutation':
            for arm in ['base', 'candidate']:
                row[arm] = host(arm, case)
            assert row['base'] == row['candidate'], case
            assert equal(root / 'base' / f'{case}.gz', root / 'candidate' / f'{case}.gz'), case
        else:
            row['mutation'] = host('mutation', case)
            original_path = root / 'battledome-parity.json'
            if not original_path.exists():
                original_path = root / 'host-parity.json'
            original = next(r for r in json.loads(original_path.read_text()) if r['case'] == case)
            assert row['mutation']['sha256'] != original['candidate']['sha256'], case
            assert row['mutation']['wireSha256'] != original['candidate']['wireSha256'], case
            row['wireMutationDetected'] = True
        rows.append(row)
        (root / f'{mode}-parity.json').write_text(json.dumps(rows, indent=2) + '\n')
        print(mode.upper(), case, flush=True)
else:
    raise ValueError(mode)
