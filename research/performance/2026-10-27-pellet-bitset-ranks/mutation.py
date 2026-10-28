"""Mutation control: a fresh archive must reject an inclusive live-bit rank.

python3 mutation.py REPOSITORY NEW_OUTPUT [--cpu 2] [--target INACTIVE_MUTATION_TARGET]
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile

if not __debug__:
    raise SystemExit('Run without optimization')
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('repository', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--cpu', type=int, default=3)
p.add_argument('--target', type=Path)
a = p.parse_args()
os.sched_setaffinity(0, {a.cpu})
ref = '47e416a3eacc99163a3af12fa5eb11af317f9e28'
out = a.output.resolve()
out.mkdir(parents=True, exist_ok=False)
archive = out / 'source.tar'
subprocess.run(['git', '-C', str(a.repository.resolve()), 'archive', '-o', str(archive), ref], check=True)
source = out / 'source'
source.mkdir()
with tarfile.open(archive) as stream:
    stream.extractall(source, filter='data')
path = source / 'src/food_grid.rs'
original = path.read_bytes()
needle = '((1u64 << (slot % 64)) - 1)'
text = original.decode()
assert text.count(needle) == 1
path.write_text(text.replace(needle, '((1u64 << (slot % 64)) | ((1u64 << (slot % 64)) - 1))'), newline='\n')
env = dict(os.environ, CARGO_BUILD_JOBS='1', RAYON_NUM_THREADS='1', RUST_TEST_THREADS='1',
           RUSTUP_TOOLCHAIN='1.99.0', CARGO_TARGET_DIR=str(a.target.resolve() if a.target else out / 'target'))
with (out / 'test.log').open('w') as log:
    result = subprocess.run(['cargo', 'test', '--locked', '--lib', 'live_ranks_cross_word_boundaries_and_survive_lazy_rebuilds'],
                            cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
log = (out / 'test.log').read_text()
assert result.returncode != 0 and 'FAILED. 0 passed; 1 failed' in log
assert 'assertion `left == right` failed' in log
selected = {'runtime': ref, 'mutation': 'include current live bit in rank', 'regressionFailed': True,
            'originalFoodGridSha256': hashlib.sha256(original).hexdigest(),
            'mutatedFoodGridSha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'logSha256': hashlib.sha256((out / 'test.log').read_bytes()).hexdigest()}
(out / 'selected.json').write_text(json.dumps(selected, indent=2) + '\n')
print('MUTATION CONTROL PASS')
