"""Fixed-work Linux x86-64 build/profile/parity/instruction study.

One bounded target, existing locked dependencies, no outside game servers.
Archive base.tar before edits and candidate.tar after the behavior commit.
Raw sources, process command lines, logs, binaries and traces stay outside Git.
"""
import argparse
import gzip
import hashlib
import json
import os
import pwd
import re
from pathlib import Path
import shutil
import struct
import subprocess
import tarfile

STUDY = Path(__file__).resolve().parent
PHASES = {1: 'process', 2: 'host-tick', 3: 'world-step', 4: 'food-interest',
          5: 'broadcast', 6: 'snake-coverage', 7: 'snake-encoding',
          8: 'food-updates', 9: 'prey-updates'}
MUTATION_LABEL = 'squared-radius comparator multiplied by 0.98'


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def swap(path, before, after):
    text = path.read_text()
    assert text.count(before) == 1, (path.name, before)
    path.write_text(text.replace(before, after), newline='\n')


def inventory(root, name):
    commands = subprocess.check_output(['ps', '-eo', 'pid,ppid,pcpu,args', '--sort=-pcpu'], text=True)
    # Process titles can include the SSH login. Keep command lines, redacting
    # this credential in memory before any inventory is written.
    account = pwd.getpwuid(os.getuid()).pw_name
    commands = re.sub(r'(?<![A-Za-z0-9_])' + re.escape(account) + r'(?![A-Za-z0-9_])',
                      '<redacted-account>', commands)
    with (root / (name + '-processes.txt')).open('x') as f:
        f.write(commands)


def prepare(root, arm, profile):
    name = arm + ('-profile' if profile else '')
    source = root / name / 'source'
    source.mkdir(parents=True)
    archive_arm = 'candidate' if arm == 'mutation' else arm
    with tarfile.open(root / (archive_arm + '.tar')) as f:
        pin = f.pax_headers.get('comment')
        f.extractall(source, filter='data')
    if arm == 'mutation':
        swap(source / 'src/model.rs', 'let r2 = r * r;', 'let r2 = r * r * 0.98;')
    prefix = source / 'research/performance/2026-10-27-host-snake-candidates'
    shutil.copy2(prefix / 'protocol_fixture.rs', source / 'src/client/simulation/study_protocols.rs')
    child = source / 'src/client/simulation/study_protocols'
    child.mkdir()
    shutil.copy2(prefix / 'tick_counter.rs', child / 'tick_counter.rs')
    p = source / 'src/client/simulation.rs'
    p.write_text(p.read_text() + '\npub mod study_protocols;\n', newline='\n')
    fixture = source / 'src/client/simulation/study_protocols.rs'
    swap(fixture, '        "official500-legacy" => (500, 500),',
         '        "official500-legacy" => (500, 500),\n        "official2281" => (2281, 0),')
    # The 2,281-seat row preserves the 500-seat seed/control/lifecycle recipe.
    # Whole command and per-tick counters remain separate scopes.
    (source / 'examples/study_protocols.rs').write_text('''fn main(){
let a:Vec<_>=std::env::args().collect();
let g=slither_sim::study_counter::enter(1);
slither_sim::client::simulation::study_protocols::run(&a[1],a.get(2).is_some_and(|s|s=="--trace"));
drop(g);slither_sim::study_counter::finish();}\n''')
    shutil.copy2(STUDY / 'counter.rs', source / 'src/study_counter.rs')
    p = source / 'src/lib.rs'
    p.write_text(p.read_text() + '\npub mod study_counter;\n', newline='\n')
    p = source / 'examples/perf_baseline.rs'
    swap(p, 'fn main() {', 'fn study_main() {')
    p.write_text(p.read_text() + '''\nfn main(){let g=slither_sim::study_counter::enter(1);
study_main();drop(g);slither_sim::study_counter::finish();}\n''', newline='\n')
    if profile:
        p = source / 'src/client/simulation/broadcast.rs'
        swap(p, '        let radius = self.world.radius();',
             '        let study_world=crate::study_counter::enter(3);\n        let radius = self.world.radius();')
        swap(p, '        // Mirror update_radius', '        drop(study_world);\n        // Mirror update_radius')
        swap(p, '        let now = self.time();\n        for (slot, snake) in self.slots.iter_mut().zip(self.world.snakes()) {',
             '        let now = self.time();\n        let study_interest=crate::study_counter::enter(4);\n        for (slot, snake) in self.slots.iter_mut().zip(self.world.snakes()) {')
        swap(p, '        self.broadcast_with(early, hand_over, false)?;',
             '        drop(study_interest);\n        self.broadcast_with(early, hand_over, false)?;')
        swap(p, '        self.legacy_maps.clear();', '        let _study_broadcast=crate::study_counter::enter(5);\n        self.legacy_maps.clear();')
        swap(p, '        for i in order {', '''        let study_cover=crate::study_counter::enter(6);
        let eligibility: Vec<bool> = order.clone().map(|i| {
            let snake = &world.snakes()[i];
            snake.alive && (i == slot || covered(head, snake, bounds[i], vision))
        }).collect();
        drop(study_cover);
        let study_encode=crate::study_counter::enter(7);
        for (i, eligible) in order.zip(eligibility) {''')
        swap(p, '            let eligible = snake.alive && (i == slot || covered(head, snake, bounds[i], vision));', '')
        swap(p, '        let connection = &mut self.slots[slot].connection;\n        let food_start',
             '        let connection = &mut self.slots[slot].connection;\n        let study_food=crate::study_counter::enter(8);\n        let food_start')
        swap(p, '        }\n        let connection = &mut self.slots[slot].connection;\n        let study_food',
             '        }\n        drop(study_encode);\n        let connection = &mut self.slots[slot].connection;\n        let study_food')
        swap(p, '        let mut visible = BTreeMap::new();',
             '        drop(study_food);\n        let _study_prey=crate::study_counter::enter(9);\n        let mut visible = BTreeMap::new();')
        swap(fixture, '        sim.advance_tick_with(0, &mut |_| Ok(())).unwrap();',
             '        let study_tick=crate::study_counter::enter(2);\n        sim.advance_tick_with(0, &mut |_| Ok(())).unwrap();\n        drop(study_tick);')
    return name, source, pin


def build(root, arm, profile):
    name, source, pin = prepare(root, arm, profile)
    env = dict(os.environ, CARGO_TARGET_DIR=str(root / 'target'), CARGO_BUILD_JOBS='1',
               CARGO_PROFILE_RELEASE_DEBUG='1', RAYON_NUM_THREADS='1', RUST_TEST_THREADS='1')
    # The installed compiler wrapper drops Cargo's hyphenated binary-path key.
    # Use the existing toolchain directly, without modifying its installation.
    direct = subprocess.check_output(['rustup', 'which', 'rustc'], text=True).strip()
    env['RUSTC'] = direct
    env['RUSTDOC'] = str(Path(direct).with_name('rustdoc'))
    command = ['cargo', 'build', '--offline', '--locked', '--release',
               '--example', 'study_protocols', '--example', 'perf_baseline']
    with (root / name / 'build.log').open('x') as f:
        subprocess.run(command, cwd=source, env=env, stdout=f, stderr=subprocess.STDOUT, check=True)
    bins = root / name / 'bin'
    bins.mkdir()
    for binary in ['study_protocols', 'perf_baseline']:
        shutil.copy2(root / 'target/release/examples' / binary, bins / binary)
    archive_arm = 'candidate' if arm == 'mutation' else arm
    receipt = {'archiveCommit': pin, 'archiveSha256': sha(root / (archive_arm + '.tar')),
               'command': command, 'toolchain': subprocess.check_output([direct, '--version'], text=True).strip(),
               'profile': profile, 'sources': {str(p.relative_to(source)): sha(p) for p in source.rglob('*.rs')},
               'binaries': {p.name: sha(p) for p in bins.iterdir()},
               'driverSha256': sha(Path(__file__)), 'counterSha256': sha(STUDY / 'counter.rs')}
    (root / name / 'pins.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print('BUILT', name, flush=True)


def run(root, arm, binary, args, label, extra=None):
    pins = json.loads((root / arm / 'pins.json').read_text())
    exe = root / arm / 'bin' / binary
    assert sha(exe) == pins['binaries'][binary]
    env = dict(os.environ, RAYON_NUM_THREADS='1', RUST_TEST_THREADS='1',
               STUDY_COUNTS=str(root / (label + '-counts.json')), **(extra or {}))
    with (root / (label + '.stdout')).open('xb') as out, (root / (label + '.stderr')).open('xb') as err:
        subprocess.run([str(exe), *args], env=env, stdout=out, stderr=err, check=True)
    assert sha(exe) == pins['binaries'][binary]
    return json.loads((root / (label + '-counts.json')).read_text())


def trace(root, arm, binary, args, label):
    exe = root / arm / 'bin' / binary
    pins = json.loads((root / arm / 'pins.json').read_text())
    assert sha(exe) == pins['binaries'][binary]
    digest = hashlib.sha256()
    size = 0
    last = b''
    path = root / (label + '.gz')
    with path.open('xb') as raw, gzip.GzipFile(fileobj=raw, mode='wb', compresslevel=1) as output, (root / (label + '.stderr')).open('xb') as err:
        process = subprocess.Popen([str(exe), *args, '--trace'], stdout=subprocess.PIPE, stderr=err,
                                   env=dict(os.environ, RAYON_NUM_THREADS='1'))
        for chunk in iter(lambda: process.stdout.read(1 << 20), b''):
            output.write(chunk)
            digest.update(chunk)
            size += len(chunk)
            last = (last + chunk)[-65536:]
        assert process.wait() == 0, label
    if binary == 'study_protocols':
        frames = wire = 0
        states = []
        summary = json.loads((root / (label + '.stderr')).read_text().splitlines()[-1])
        with gzip.open(path, 'rb') as f:
            while header := f.read(16):
                assert len(header) == 16
                tag, n = struct.unpack('<QQ', header)
                data = f.read(n)
                assert len(data) == n
                if tag in [0, 3]:
                    assert json.loads(data)['state']
                    states.append(hashlib.sha256(data).hexdigest())
                elif tag == 1:
                    tick, slot, _ = struct.unpack('<QQQ', data)
                    assert tick < 1080 and slot < summary['seats']
                elif tag == 2:
                    frames += 1
                    wire += n
                elif tag == 4:
                    assert json.loads(data) == summary
                else:
                    raise AssertionError(tag)
        assert tag == 4 and len(states) == 2 and frames == summary['frames'] and wire == summary['bytes']
    else:
        report = json.loads(last.splitlines()[-1])
        assert report['kind'] in ['report', 'end'] and report['mode'] == 'trace'
        assert report['case'] == args[0] and report['iterations'] == int(args[1])
        # Lifecycle rows require the end marker as well as the final report.
        if args[0].startswith(('official-', 'sandbox-', 'battledome-', 'client-')):
            endings = [json.loads(line) for line in last.splitlines()[1:-1] if b'"kind":"end"' in line or b'"kind": "end"' in line]
            assert len(endings) == 1 and endings[0]['operations'] == int(args[1]), label
    assert sha(exe) == pins['binaries'][binary]
    return {'sha256': digest.hexdigest(), 'bytes': size}


def compare(left, right):
    with gzip.open(left, 'rb') as a, gzip.open(right, 'rb') as b:
        while True:
            x, y = a.read(1 << 20), b.read(1 << 20)
            if x != y:
                return False
            if not x:
                return True


def matrix(root):
    catalog = json.loads(subprocess.check_output([str(root / 'base/bin/perf_baseline'), '--list']))
    other = json.loads(subprocess.check_output([str(root / 'candidate/bin/perf_baseline'), '--list']))
    assert catalog == other
    (root / 'catalog.json').write_text(json.dumps(catalog, indent=2) + '\n')
    cases = [('perf_baseline', [r['name'], str(r['traceIterations'])])
             for r in catalog['workloads'] if r['available'] and not r['name'].startswith('host-')]
    cases += [('study_protocols', [c]) for c in ['official500', 'official500-half', 'official500-legacy', 'official2281']]
    result = []
    for binary, args in cases:
        row = {'binary': binary, 'args': args}
        for arm in ['base', 'candidate']:
            row[arm] = trace(root, arm, binary, args, 'trace-' + args[0] + '-' + arm)
        assert row['base'] == row['candidate'], args
        assert compare(root / ('trace-' + args[0] + '-base.gz'), root / ('trace-' + args[0] + '-candidate.gz')), args
        result.append(row)
        (root / 'parity.json').write_text(json.dumps(result, indent=2) + '\n')
        print('PARITY', args[0], flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=['build', 'profile', 'pairs', 'trace', 'mutation', 'reconstruct'])
    p.add_argument('root', type=Path)
    p.add_argument('--arm', default='base')
    p.add_argument('--profile', action='store_true')
    p.add_argument('--libm-only', action='store_true')
    p.add_argument('--plain-libm', action='store_true')
    p.add_argument('--cases', nargs='+', choices=['official500', 'official500-legacy', 'official2281', 'client-official-500'])
    a = p.parse_args()
    root = a.root.resolve()
    os.sched_setaffinity(0, {0})
    if a.mode == 'build':
        build(root, a.arm, a.profile)
    elif a.mode == 'trace':
        matrix(root)
    elif a.mode == 'reconstruct':
        out = root / 'reconstructed'
        out.mkdir()
        for arm in ['base', 'candidate']:
            (out / (arm + '.tar')).symlink_to(root / (arm + '.tar'))
        results = []
        for arm, profile in [('base', False), ('candidate', False), ('mutation', False), ('base', True)]:
            expected_name = arm + ('-profile' if profile else '')
            if not (root / expected_name / 'pins.json').exists():
                continue
            name, source, _ = prepare(out, arm, profile)
            expected = json.loads((root / name / 'pins.json').read_text())['sources']
            actual = {str(p.relative_to(source)): sha(p) for p in source.rglob('*.rs')}
            assert actual == expected, name
            results.append({'arm': name, 'allGeneratedRustSourceHashesMatch': True,
                            'rustSourceFiles': len(actual)})
        (root / 'reconstruction.json').write_text(json.dumps(results, indent=2) + '\n')
    elif a.mode == 'mutation':
        results = []
        for case in ['official500', 'official500-legacy']:
            row = trace(root, 'mutation', 'study_protocols', [case], 'trace-mutation-' + case)
            equal = compare(root / ('trace-' + case + '-candidate.gz'), root / ('trace-mutation-' + case + '.gz'))
            assert not equal, 'deliberate radius perturbation escaped the wire comparison'
            results.append({'case': case, 'mutation': MUTATION_LABEL,
                            'equal': equal, **row})
        (root / 'mutation.json').write_text(json.dumps(results, indent=2) + '\n')
    elif a.mode == 'profile':
        subprocess.run(['gcc', '-O2', '-Werror=attributes', '-fPIC', '-shared', '-o', str(root / 'libstudy.so'),
                        str(STUDY / 'libm.c'), '-ldl'], check=True)
        disassembly = ''
        for name in ['empty1', 'empty2', 'empty_sc', 'probe1', 'probe2', 'probe_sc']:
            code = subprocess.check_output(['objdump', '-d', '--no-show-raw-insn',
                                            '--disassemble=' + name, str(root / 'libstudy.so')], text=True)
            instructions = re.findall(r'^\s*[0-9a-f]+:\s+([a-z][a-z0-9]*)', code, re.MULTILINE)
            expected = ['ret'] if name.startswith('empty') else ['nop'] * 32 + ['ret']
            assert instructions == expected, (name, instructions)
            disassembly += code
        prefix = a.arm + ('plain' if a.plain_libm else '')
        (root / ('libm-' + prefix + '-calibration-disassembly.txt')).write_text(disassembly)
        (root / ('libm-' + prefix + '-build.json')).write_text(json.dumps({
            'command': ['gcc', '-O2', '-Werror=attributes', '-fPIC', '-shared', '-o', 'libstudy.so', 'libm.c', '-ldl'],
            'gccVersion': subprocess.check_output(['gcc', '-dumpfullversion'], text=True).strip(),
            'sourceSha256': sha(STUDY / 'libm.c'), 'binarySha256': sha(root / 'libstudy.so'),
            'emptyFunctionInstructions': {n: ['ret'] for n in ['empty1', 'empty2', 'empty_sc']},
            'probeFunctionInstructions': {n: ['nop'] * 32 + ['ret'] for n in ['probe1', 'probe2', 'probe_sc']},
            'calibrationRepetitions': 64}, indent=2) + '\n')
        for case in (a.cases or ['official500', 'official500-legacy', 'official2281', 'client-official-500']):
            binary = 'perf_baseline' if case.startswith('client-') else 'study_protocols'
            args = [case, '24'] if binary == 'perf_baseline' else [case]
            for interpose in ([True] if a.libm_only or a.plain_libm else [False, True]):
                label = prefix + '-' + case + ('-libm' if interpose else '-phases')
                inventory(root, label + '-before')
                extra = {'LD_PRELOAD': str(root / 'libstudy.so'), 'STUDY_LIBM': str(root / (label + '.csv'))} if interpose else {}
                counts = run(root, a.arm + ('' if a.plain_libm else '-profile'), binary, args, label, extra)
                inventory(root, label + '-after')
                print(label, json.dumps(counts), flush=True)
    elif a.mode == 'pairs':
        assert (root / 'parity.json').exists() and (root / 'mutation.json').exists()
        checks = json.loads((root / 'checks/results.json').read_text())
        assert checks[-1]['name'] == 'rust-group' and checks[-1]['exit'] == 0
        results = {}
        for case in ['official500', 'official500-legacy', 'official2281', 'client-official-500']:
            binary = 'perf_baseline' if case.startswith('client-') else 'study_protocols'
            args = [case, '24'] if binary == 'perf_baseline' else [case]
            rows = []
            for pair in range(-1, 5):
                row = {'pair': pair, 'warmup': pair == -1}
                for arm in (['base', 'candidate'] if pair % 2 == 0 or pair == -1 else ['candidate', 'base']):
                    label = f'pair-{case}-{pair}-{arm}'
                    inventory(root, label + '-before')
                    extra = {'STUDY_TICK_COUNTS': str(root / (label + '-ticks.json'))} if binary == 'study_protocols' else {}
                    counts = run(root, arm, binary, args, label, extra)
                    inventory(root, label + '-after')
                    row[arm] = counts['instructions'][1]
                    if extra:
                        ticks = json.loads((root / (label + '-ticks.json')).read_text())
                        assert len(ticks) == 1080 and all(t[2] == 0 or t[3] / t[2] >= .99 for t in ticks)
                        row[arm + 'Ticks'] = sum(t[1] for t in ticks)
                row['ratio'] = row['candidate'] / row['base']
                if binary == 'study_protocols':
                    row['tickRatio'] = row['candidateTicks'] / row['baseTicks']
                    assert (root / f'pair-{case}-{pair}-base.stderr').read_bytes() == (root / f'pair-{case}-{pair}-candidate.stderr').read_bytes()
                else:
                    reports = []
                    for arm in ['base', 'candidate']:
                        report = json.loads((root / f'pair-{case}-{pair}-{arm}.stdout').read_text().splitlines()[-1])
                        report.pop('timing')
                        for key in ['admissionSeconds', 'admissionSecondsPerDecision']:
                            report['details'].pop(key)
                        reports.append(report)
                    assert reports[0] == reports[1], 'client sample changed work or lifecycle'
                rows.append(row)
                print(case, json.dumps(row), flush=True)
            results[case] = rows
            (root / 'pairs.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()
