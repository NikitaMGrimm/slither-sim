"""Check every online/offline result field using the pinned staged reducer.

This supplements the inherited selector, which permits declared tail changes.
Run against each unmodified retained archive; no field is removed or rounded.
"""
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tarfile
import tempfile


def compare(online, offline):
    if online != offline:
        raise ValueError('Full online/offline result differs')


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('harness', type=Path, help='Staged linked v3 harness from PLAN.md')
    p.add_argument('archive', type=Path)
    p.add_argument('output', type=Path)
    args = p.parse_args()
    sys.path.insert(0, str(args.harness.resolve()))
    spec = importlib.util.spec_from_file_location('replay_acceptance', args.harness / 'acceptance.py')
    reducer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(reducer)
    with tarfile.open(args.archive) as tar, tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        window = json.load(tar.extractfile('run-summary.json'))['windows'][0]
        (root / 'load-run').mkdir()
        for name in ('host-stream.log', 'stats.jsonl', 'client.log'):
            (root / 'load-run' / name).write_bytes(tar.extractfile('load-run/' + name).read())
        offline = reducer.reduce_window(root / 'load-run', window)
        online = window['result']
        compare(online, offline)
        assert digest(online) == digest(offline)
        controls = []
        for key in ('p50', 'p99', 'p999', 'max'):
            mutated = copy.deepcopy(offline)
            mutated['worldCpuMsPerTick'][key] += 1
            try:
                compare(online, mutated)
            except ValueError:
                controls.append(key)
            else:
                raise AssertionError('Tail/max mutation escaped comparison')
    with args.archive.open('rb') as stream:
        archive_hash = hashlib.file_digest(stream, 'sha256').hexdigest()
    receipt = {'archiveSha256': archive_hash, 'fullResultEqual': True,
               'onlineFullResultSha256': digest(online), 'offlineFullResultSha256': digest(offline),
               'omittedFields': [], 'tailAndMaximumMutationsDetected': controls,
               'scope': 'Post-result retained-data verification; no selection or gate changes'}
    args.output.write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    main()
