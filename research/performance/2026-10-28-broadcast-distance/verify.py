"""Focused geometry mutation controls, then the complete offline Rust group."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile

root = Path(sys.argv[1]).resolve()
out = root / 'checks'
out.mkdir()
source = out / 'source'
source.mkdir()
with tarfile.open(root / 'candidate.tar') as f:
    f.extractall(source, filter='data')
os.sched_setaffinity(0, {0})
direct = subprocess.check_output(['rustup', 'which', 'rustc'], text=True).strip()
env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target'), CARGO_BUILD_JOBS='1',
           RAYON_NUM_THREADS='1', RUST_TEST_THREADS='1', CARGO_NET_OFFLINE='true',
           CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0',
           RUSTC=direct, RUSTDOC=str(Path(direct).with_name('rustdoc')))
results = []


def check(command, name, fails=False):
    with (out / (name + '.log')).open('x') as f:
        result = subprocess.run(command, cwd=source, env=env, stdout=f, stderr=subprocess.STDOUT)
    text = (out / (name + '.log')).read_text()
    assert result.returncode != 0 and 'test result: FAILED' in text if fails else result.returncode == 0, name
    results.append({'name': name, 'command': command, 'exit': result.returncode, 'expectedFailure': fails})
    (out / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
    print('CHECK', name, flush=True)


focused = ['cargo', 'test', '--locked', '--lib', 'segment_comparison_matches_original_projection_and_hypot']
check(focused, 'focused')
check(['cargo', 'test', '--release', '--locked', '--lib', 'segment_comparison_matches_original_projection_and_hypot'], 'focused-release')
path = source / 'src/model.rs'
original = path.read_bytes()
try:
    text = original.decode()
    before = 'if d2 < r2 * (1.0 - 1e-9) {'
    assert text.count(before) == 1
    path.write_text(text.replace(before, 'if d2 < r2 * (1.0 + 1e-9) {'), newline='\n')
    check(focused, 'guard-boundary-mutation', fails=True)
finally:
    path.write_bytes(original)
check(focused, 'restored')
check(['python3', 'tools/check.py', 'rust'], 'rust-group')
