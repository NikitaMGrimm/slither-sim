"""Compare complete deterministic traces, retaining gzip streams locally."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('base', type=Path)
parser.add_argument('candidate', type=Path)
parser.add_argument('output', type=Path)
parser.add_argument('--battledome', action='store_true')
options = parser.parse_args()
out = options.output.resolve()
out.mkdir(exist_ok=False)
rows = []
cases = [
    ('portable', 'perf_baseline', ['portable', '32', '--trace']),
    ('protocol', 'perf_baseline', ['protocol', '32', '--trace']),
    ('client-official-500', 'perf_baseline', ['client-official-500', '1', '--trace']),
    ('client-battledome-64', 'perf_baseline', ['client-battledome-64', '4', '--trace']),
    ('retained-step_all', 'perf_client_retained', ['step_all', '384', '--trace']),
    ('retained-next_frame', 'perf_client_retained', ['next_frame', '384', '--trace']),
]
if options.battledome:
    cases = [(f'client-battledome-{n}', 'perf_battledome', [f'client-battledome-{n}', '4', '--trace']) for n in [40, 64, 80]]
for name, binary, args in cases:
    records = {}
    for label in ['base', 'candidate']:
        target = out / f'{name}-{label}.jsonl.gz'
        executable = getattr(options, label).resolve() / binary
        digest = hashlib.sha256()
        size = 0
        last = b''
        with (out / f'{name}-{label}.stderr').open('wb') as err:
            process = subprocess.Popen([str(executable), *args], stdout=subprocess.PIPE, stderr=err)
            try:
                with gzip.open(target, 'wb', compresslevel=1) as output:
                    for line in process.stdout:
                        output.write(line)
                        digest.update(line)
                        size += len(line)
                        last = line
                assert process.wait(timeout=10) == 0
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)
        terminal = json.loads(last)
        assert terminal['kind'] in ['report', 'end'], terminal
        if binary in ['perf_baseline', 'perf_battledome']:
            assert terminal['case'] == args[0] and terminal['iterations'] == int(args[1])
            assert terminal['mode'] == 'trace'
        records[label] = {'bytes': size, 'sha256': digest.hexdigest(), 'terminalKind': terminal['kind'],
                          'binarySha256': hashlib.sha256(executable.read_bytes()).hexdigest()}
    with gzip.open(out / f'{name}-base.jsonl.gz', 'rb') as a, gzip.open(out / f'{name}-candidate.jsonl.gz', 'rb') as b:
        while True:
            left, right = a.read(1 << 20), b.read(1 << 20)
            assert left == right, f'{name}: unequal decompressed bytes'
            if not left:
                break
    row = {'case': name, 'equal': True, **records}
    rows.append(row)
    (out / 'identity.json').write_text(json.dumps(rows, indent=2) + '\n')
    print(json.dumps(row), flush=True)
