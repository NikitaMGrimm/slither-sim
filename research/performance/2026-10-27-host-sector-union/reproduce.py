"""Rebuild and compare the pinned offline fixtures on Linux/WSL2."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('repository', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--perf', required=True)
p.add_argument('--base', default='5eb0736ee3650687adff5b5c431c33994bf75bb9')
p.add_argument('--candidate', default='46ecdf8e9f83f35c77780f32a081d4c8b84e304f')
a = p.parse_args()
repo, out = a.repository.resolve(), a.output.resolve()
out.mkdir(parents=True, exist_ok=False)
os.sched_setaffinity(0, {0, 2})
os.environ.update(CARGO_BUILD_JOBS='1', RAYON_NUM_THREADS='1', RUST_TEST_THREADS='1',
                  RUSTUP_TOOLCHAIN='1.99.0', CARGO_PROFILE_RELEASE_DEBUG='1')
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, cwd, log):
    with (out / log).open('w') as stream:
        subprocess.run(command, cwd=cwd, stdout=stream, stderr=subprocess.STDOUT, check=True)


for arm, pin in [('base', a.base), ('candidate', a.candidate)]:
    source = out / arm / 'source'
    source.mkdir(parents=True)
    archive = out / f'{arm}.tar'
    subprocess.run(['git', '-C', str(repo), 'archive', '--format=tar', '-o', str(archive), pin], check=True)
    with tarfile.open(archive) as stream:
        stream.extractall(source, filter='data')

candidate = out / 'candidate/source'
os.environ['CARGO_TARGET_DIR'] = str(out / 'checks-target')
run([sys.executable, 'tools/check.py', 'rust'], candidate, 'rust-checks.log')
old = candidate / 'research/performance/2026-10-26-host-known-slots'
food = candidate / 'research/performance/2026-10-26-host-food-sharing'
harness = out / 'harness'
harness.mkdir()
for name in ['loopback.py', 'instructions.py', 'trace_compare.py', 'battledome-catalog.patch']:
    shutil.copyfile(old / name, harness / name)
run(['patch', '-p1', '-i', str(food / 'harness.patch')], harness, 'harness-patch.log')
names = ['perf_baseline', 'perf_battledome', 'perf_client_retained']
pins = {'base': a.base, 'candidate': a.candidate, 'binaries': {}, 'harness': {}}
for arm in ['base', 'candidate']:
    source = out / arm / 'source'
    run(['patch', '-p1', '-i', str(food / 'client-reconnect.patch')], source, f'{arm}-reconnect.log')
    run(['patch', '-o', str(source / 'examples/perf_battledome.rs'),
         str(source / 'examples/perf_baseline.rs'), str(harness / 'battledome-catalog.patch')], source, f'{arm}-catalog.log')
    os.environ['CARGO_TARGET_DIR'] = str(out / arm / 'target')
    run(['cargo', 'build', '--locked', '--release', '--features', 'networking',
         *[x for name in names for x in ['--example', name]]], source, f'{arm}-build.log')
    if arm == 'candidate':
        os.environ['CARGO_TARGET_DIR'] = str(out / 'harness-target')
        run(['cargo', 'test', '--locked', '--example', 'perf_baseline', '--example', 'perf_battledome'], source, 'harness-checks.log')
    copied = out / arm / 'bin'
    copied.mkdir()
    for name in names:
        shutil.copy2(out / arm / 'target/release/examples' / name, copied / name)
        pins['binaries'][f'{arm}/{name}'] = sha(copied / name)
        digest = sha(source / 'examples' / f'{name}.rs')
        if arm == 'base':
            pins['harness'][name] = digest
        else:
            assert pins['harness'][name] == digest
(out / 'pins.json').write_text(json.dumps(pins, indent=2) + '\n')
for directory, extra in [('traces', []), ('traces-dome', ['--battledome'])]:
    run([sys.executable, str(harness / 'trace_compare.py'), str(out / 'base/bin'),
         str(out / 'candidate/bin'), str(out / directory), *extra], out, f'{directory}.log')
run([sys.executable, str(harness / 'instructions.py'), str(out / 'base/bin'),
     str(out / 'candidate/bin'), str(out / 'instructions'), '--perf', a.perf], out, 'instructions.log')
for key, digest in pins['binaries'].items():
    arm, name = key.split('/')
    assert sha(out / arm / 'bin' / name) == digest
print('Complete: equal full traces, five instruction pairs, unchanged binaries')
