"""Mutation control: a fresh archive must reject a duplicated migrated pellet.

python3 rebuild_mutation.py REPOSITORY NEW_OUTPUT [--cpu 2] [--target INACTIVE_MUTATION_TARGET]
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
p.add_argument('--cpu', type=int, default=2)
p.add_argument('--target', type=Path)
a = p.parse_args()
os.sched_setaffinity(0, {a.cpu})
ref = '9a3f6f5fe85184e42b32c2251480b94356c9210e'
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
needle = '            self.remove_cell(self.food_slots[food]);'
text = original.decode()
assert text.count(needle) == 1
path.write_text(text.replace(needle, '            let _ = self.food_slots[food];'), newline='\n')
env = dict(os.environ, CARGO_BUILD_JOBS='1', RAYON_NUM_THREADS='1', RUST_TEST_THREADS='1',
           RUSTUP_TOOLCHAIN='1.99.0', CARGO_TARGET_DIR=str(a.target.resolve() if a.target else out / 'target'))
with (out / 'test.log').open('w') as log:
    result = subprocess.run(['cargo', 'test', '--locked', '--lib', 'incremental_rebuild_mirrors_prefix_suffix_edits_and_clone'],
                            cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
log = (out / 'test.log').read_text()
assert result.returncode != 0 and 'FAILED. 0 passed; 1 failed' in log
assert 'assertion `left == right` failed' in log
selected = {'runtime': ref, 'mutation': 'keep copied pellet in old cell list', 'regressionFailed': True,
            'originalFoodGridSha256': hashlib.sha256(original).hexdigest(),
            'mutatedFoodGridSha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'logSha256': hashlib.sha256((out / 'test.log').read_bytes()).hexdigest()}
(out / 'selected.json').write_text(json.dumps(selected, indent=2) + '\n')
print('MUTATION CONTROL PASS')
