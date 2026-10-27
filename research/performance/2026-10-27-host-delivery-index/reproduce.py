"""Reuse the frozen readiness and snake-candidate studies; keep raw evidence outside Git."""
import argparse, hashlib, json, os, shutil, subprocess, sys, tarfile
from pathlib import Path

if not __debug__:
    raise SystemExit('run without -O: parity and pin checks require asserts')

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('mode', choices=['build', 'trace', 'gateway', 'count', 'pairs'])
p.add_argument('archives', type=Path, help='base.tar and candidate.tar from git archive')
p.add_argument('output', type=Path)
p.add_argument('--perf')
a = p.parse_args()
root, archives = a.output.resolve(), a.archives.resolve()
os.sched_setaffinity(0, {0})
env = dict(os.environ, CARGO_BUILD_JOBS='1', RAYON_NUM_THREADS='1', RUST_TEST_THREADS='1', RUSTUP_TOOLCHAIN='1.99.0', CARGO_PROFILE_RELEASE_DEBUG='1')
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()

def run(command, name, extra=None, cwd=None):
    with (root / (name + '.log')).open('x') as log:
        subprocess.run(command, cwd=cwd, env=dict(env, **(extra or {})), stdout=log, stderr=subprocess.STDOUT, check=True)

if a.mode == 'build':
    root.mkdir(parents=True, exist_ok=False)
    pins = {}
    for arm in ['base', 'candidate']:
        with tarfile.open(archives / (arm + '.tar')) as f:
            pin = f.pax_headers['comment']
            assert len(pin) == 40 and all(c in '0123456789abcdef' for c in pin)
            pins[arm] = {'commit': pin, 'archiveSha256': sha(archives / (arm + '.tar'))}
            pins[arm]['gatewaySha256'] = hashlib.sha256(f.extractfile('src/client/server.rs').read()).hexdigest()
    assert pins['base']['gatewaySha256'] != pins['candidate']['gatewaySha256'], 'delivery change missing from candidate'
    with tarfile.open(archives / 'base.tar') as f:
        for name, files in [('readiness', ['reproduce.py', 'idle.rs', 'socket_study.rs', 'load_overlay.py', 'measure.py', 'thread_counter.py']), ('candidates', ['reproduce.py', 'protocol_fixture.rs', 'tick_counter.rs'])]:
            dest = root / 'harness' / name
            dest.mkdir(parents=True)
            source = '2026-10-27-host-' + ('socket-readiness' if name == 'readiness' else 'snake-candidates')
            for file in files:
                (dest / file).write_bytes(f.extractfile('research/performance/' + source + '/' + file).read())
    # The prior diagnostic's private Gateway literal needs the new slot index.
    # Adapt only its setup; production source and measurement loops stay pinned.
    builder = root / 'harness/readiness/reproduce.py'
    text = builder.read_text()
    before = "(study/'idle.rs').read_text().replace('/* READINESS */','readiness: readiness::Readiness::default(),' if ready else '')"
    after = before + ".replace('sessions: Vec::new(),', 'sessions: Vec::new(), session_by_slot: vec![None; 500],' if 'session_by_slot:' in (source/server).read_text() else 'sessions: Vec::new(),')"
    assert text.count(before) == 1
    builder.write_text(text.replace(before, after))
    trace_builder = root / 'harness/candidates/reproduce.py'
    text = trace_builder.read_text()
    before = "subprocess.run(['git','-C',str(repo),'archive','--format=tar','-o',str(archive),ref],check=True)"
    after = "shutil.copy2(Path(os.environ['STUDY_ARCHIVES'])/(arm+'.tar'),archive)"
    assert text.count(before) == 1
    trace_builder.write_text(text.replace(before, after))
    pins['harnessSha256'] = {str(p.relative_to(root)): sha(p) for p in sorted((root / 'harness').rglob('*')) if p.is_file()}
    (root / 'inputs.json').write_text(json.dumps(pins, indent=2) + '\n')
    common = ['--base', pins['base']['commit'], '--candidate', pins['candidate']['commit']]
    run([sys.executable, str(builder), 'build', '.', str(root / 'socket'), *common, '--archives', str(archives)], 'build-sockets')
    run([sys.executable, str(trace_builder), 'build', '.', str(root / 'trace'), *common], 'build-traces', {'STUDY_ARCHIVES': str(archives)})
else:
    pins = json.loads((root / 'inputs.json').read_text())
    for name, digest in pins['harnessSha256'].items():
        assert sha(root / name) == digest, name
    for arm in ['base', 'candidate']:
        assert sha(archives / (arm + '.tar')) == pins[arm]['archiveSha256']
    if a.mode == 'trace':
        run([sys.executable, str(root / 'harness/candidates/reproduce.py'), 'trace', '.', str(root / 'trace')], 'traces')
    elif a.mode == 'gateway':
        assert (root / 'trace/traces.complete').is_file(), 'compare traces first'
        for arm in ['base', 'candidate']:
            out = root / 'trace' / ('native-' + arm)
            source = out / 'source'
            fixture = source / 'src/client/simulation/study_protocols.rs'
            text = fixture.read_text()
            edits = [
                ('pub fn run(case: &str, trace: bool) {', 'pub fn run(case: &str, trace: bool) { run_with_route(case, trace, &mut |_| {}); }\npub type Batch = Vec<(f64, Vec<u8>)>;\npub fn run_with_route(case: &str, trace: bool, route: &mut impl FnMut(Vec<Batch>)) {'),
                ('        for slot in 0..count {\n            for (at, frame) in sim.take(slot).unwrap() {', '        let mut delivered = Vec::with_capacity(count);\n        for slot in 0..count {\n            let batch = sim.take(slot).unwrap();\n            for (at, frame) in &batch {'),
                ('                black_box(frame);\n            }\n        }\n    }\n    assert!(forced', '                black_box(frame);\n            }\n            delivered.push(batch);\n        }\n        route(delivered);\n    }\n    assert!(forced'),
            ]
            for before, after in edits:
                assert text.count(before) == 1, before
                text = text.replace(before, after)
            fixture.write_text(text)
            server = source / 'src/client/server.rs'
            indexed = 'session_by_slot:' in server.read_text()
            bridge = (Path(__file__).parent / 'gateway.rs').read_text().replace('/* INDEX */', 'session_by_slot: vec![None; n],' if indexed else '')
            (source / 'src/client/server/study_gateway.rs').write_text(bridge)
            server.write_text(server.read_text() + '\npub mod study_gateway;\n')
            (source / 'examples/study_gateway.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::server::study_gateway::run(&a[1]);}\n')
            run(['cargo', 'build', '--release', '--locked', '--example', 'study_gateway'], 'gateway-build-' + arm, {'CARGO_TARGET_DIR': str(out / 'target')}, source)
            binary = out / 'bin/study_gateway'
            shutil.copy2(out / 'target/release/examples/study_gateway', binary)
            (out / 'gateway-pins.json').write_text(json.dumps({'runtimeCommit': pins[arm]['commit'], 'binarySha256': sha(binary), 'fixtureSha256': sha(fixture), 'gatewaySha256': sha(source / 'src/client/server/study_gateway.rs')}, indent=2) + '\n')
        driver = (root / 'harness/candidates/reproduce.py').read_text()
        assert driver.count("/'bin/study_protocols'") == 1
        driver = driver.replace("/'bin/study_protocols'", "/'bin/study_gateway'").replace("/'pins.json'", "/'gateway-pins.json'")
        (root / 'harness/gateway-count.py').write_text(driver)
    elif a.mode == 'count':
        assert a.perf
        import resource
        soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
        resource.setrlimit(resource.RLIMIT_NOFILE, (min(max(soft, 4096), hard), hard))
        run([sys.executable, str(root / 'harness/gateway-count.py'), 'count', '.', str(root / 'trace'), '--perf', a.perf], 'instructions')
    else:
        assert (root / 'trace/traces.complete').is_file(), 'compare traces first'
        run([sys.executable, str(root / 'harness/readiness/measure.py'), str(root / 'socket')], 'pairs')
print('PASS', a.mode, flush=True)
