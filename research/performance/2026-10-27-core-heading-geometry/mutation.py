"""Mutation control: a fresh archive must reject a stale heading direction.

python3 mutation.py REPOSITORY NEW_OUTPUT [--cpu 2]
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
a = p.parse_args()
os.sched_setaffinity(0, {a.cpu})
ref = '536f8aa3636f367e2d32b48ca02a72e63b1e146a'
out = a.output.resolve()
out.mkdir(parents=True, exist_ok=False)
archive = out / 'source.tar'
subprocess.run(['git', '-C', str(a.repository.resolve()), 'archive', '-o', str(archive), ref], check=True)
source = out / 'source'
source.mkdir()
with tarfile.open(archive) as stream:
    stream.extractall(source, filter='data')
path = source / 'src/physics.rs'
original = path.read_bytes()
needle = '    let direction = (next.cos(), next.sin());'
text = original.decode()
assert text.count(needle) == 1
path.write_text(text.replace(needle, '    let direction = (heading.cos(), heading.sin());'), newline='\n')
env = dict(os.environ, CARGO_BUILD_JOBS='1', RAYON_NUM_THREADS='1', RUST_TEST_THREADS='1',
           RUSTUP_TOOLCHAIN='1.99.0', CARGO_TARGET_DIR=str(out / 'target'))
with (out / 'test.log').open('w') as log:
    result = subprocess.run(['cargo', 'test', '--locked', '--lib', 'directed_geometry_matches_public_oracles_bit_for_bit'],
                            cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
log = (out / 'test.log').read_text()
assert result.returncode != 0 and 'FAILED. 0 passed; 1 failed' in log
assert 'assertion `left == right` failed:' in log
selected = {'runtime': ref, 'mutation': 'use prior direction after turning', 'regressionFailed': True,
            'originalPhysicsSha256': hashlib.sha256(original).hexdigest(),
            'mutatedPhysicsSha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'logSha256': hashlib.sha256((out / 'test.log').read_bytes()).hexdigest()}
(out / 'selected.json').write_text(json.dumps(selected, indent=2) + '\n')
print('MUTATION CONTROL PASS')
