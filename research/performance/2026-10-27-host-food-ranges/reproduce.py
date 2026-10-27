"""Frozen archive overlay for the guarded stock/half13/all13 and historical study."""
import argparse, hashlib, json, os, shutil, subprocess, sys, tarfile
from pathlib import Path

if not __debug__:
    raise SystemExit('run without -O: parity and pin checks require asserts')
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('mode', choices=['build', 'trace', 'count', 'sockets', 'pairs'])
p.add_argument('archives', type=Path, help='base.tar and candidate.tar from git archive')
p.add_argument('output', type=Path)
p.add_argument('--perf')
a = p.parse_args()
root, archives = a.output.resolve(), a.archives.resolve()
os.sched_setaffinity(0, {0})
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
if a.mode == 'build':
    root.mkdir(parents=True, exist_ok=False)
    harness = root / 'harness'
    harness.mkdir()
    pins = {}
    for arm in ['base', 'candidate']:
        archive = archives / (arm + '.tar')
        with tarfile.open(archive) as f:
            pin = f.pax_headers['comment']
            assert len(pin) == 40 and all(c in '0123456789abcdef' for c in pin)
            pins[arm] = {'commit': pin, 'archiveSha256': sha(archive),
                         'foodSha256': hashlib.sha256(f.extractfile('src/client/simulation/food.rs').read()).hexdigest()}
            if arm == 'base':
                prefix = 'research/performance/2026-10-27-host-snake-candidates/'
                for name in ['reproduce.py', 'protocol_fixture.rs', 'tick_counter.rs']:
                    (harness / name).write_bytes(f.extractfile(prefix + name).read())
    assert pins['base']['foodSha256'] != pins['candidate']['foodSha256']
    driver = harness / 'reproduce.py'
    text = driver.read_text()
    before = "subprocess.run(['git','-C',str(repo),'archive','--format=tar','-o',str(archive),ref],check=True)"
    assert text.count(before) == 1
    driver.write_text(text.replace(before, "shutil.copy2(Path(os.environ['STUDY_ARCHIVES'])/(arm+'.tar'),archive)"))
    pins['harnessSha256'] = {f.name: sha(f) for f in harness.iterdir()}
    (root / 'inputs.json').write_text(json.dumps(pins, indent=2) + '\n')
else:
    pins = json.loads((root / 'inputs.json').read_text())
    for arm in ['base', 'candidate']:
        assert sha(archives / (arm + '.tar')) == pins[arm]['archiveSha256']
    for name, digest in pins['harnessSha256'].items():
        assert sha(root / 'harness' / name) == digest
if a.mode == 'sockets':
    folder = root / 'socket-harness'
    folder.mkdir()
    with tarfile.open(archives / 'base.tar') as f:
        prefix = 'research/performance/2026-10-27-host-socket-readiness/'
        for name in ['reproduce.py', 'idle.rs', 'socket_study.rs', 'load_overlay.py', 'measure.py', 'thread_counter.py']:
            (folder / name).write_bytes(f.extractfile(prefix + name).read())
    driver = folder / 'reproduce.py'
    text = driver.read_text()
    before = "(study/'idle.rs').read_text().replace('/* READINESS */','readiness: readiness::Readiness::default(),' if ready else '')"
    assert text.count(before) == 1
    driver.write_text(text.replace(before, before + ".replace('sessions: Vec::new(),', 'sessions: Vec::new(), session_by_slot: vec![None; 500],')"))
    measure = folder / 'measure.py'
    text = measure.read_text()
    assert text.count('warmup, hold, port = 10, 12, 18777') == 1
    measure.write_text(text.replace('warmup, hold, port = 10, 12, 18777',
                                   'warmup, hold, port = 10, 12, 18793'))
    pins['socketHarnessSha256'] = {f.name: sha(f) for f in folder.iterdir()}
    (root / 'inputs.json').write_text(json.dumps(pins, indent=2) + '\n')
    cmd = [sys.executable, str(driver), 'build', '.', str(root / 'socket'), '--archives', str(archives),
           '--base', pins['base']['commit'], '--candidate', pins['candidate']['commit']]
elif a.mode == 'pairs':
    assert (root / 'run/traces.complete').exists()
    for name, digest in pins['socketHarnessSha256'].items():
        assert sha(root / 'socket-harness' / name) == digest
    cmd = [sys.executable, str(root / 'socket-harness/measure.py'), str(root / 'socket')]
else:
    cmd = [sys.executable, str(root / 'harness/reproduce.py'), a.mode, '.', str(root / 'run'),
       '--base', pins['base']['commit'], '--candidate', pins['candidate']['commit']]
if a.mode == 'count':
    assert a.perf
    cmd += ['--perf', a.perf]
with (root / (a.mode + '.log')).open('x') as log:
    subprocess.run(cmd, env=dict(os.environ, STUDY_ARCHIVES=str(archives)),
                   stdout=log, stderr=subprocess.STDOUT, check=True)
print('PASS', a.mode, flush=True)
