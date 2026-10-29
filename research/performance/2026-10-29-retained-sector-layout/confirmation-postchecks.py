"""Check both series separately and compare final runtime with its measured arm."""
from pathlib import Path
import json,subprocess,sys,tarfile

r = Path(__file__).resolve().parent
source = r / 'delivery-source'
source.mkdir()
with tarfile.open(r / 'final-delivery.tar') as archive:
    archive.extractall(source, filter='data')
study = source / 'research/performance/2026-10-29-retained-sector-layout'
results = json.loads((study / 'confirmation-results.json').read_text())
commands = []
for label, command in [
    ('documentation', [sys.executable, '-B', '-m', 'unittest', 'discover',
                       '-s', 'tools', '-p', 'test_documentation.py']),
    ('original-tables-and-gate', [sys.executable, str(study / 'tables.py')]),
    ('confirmation-tables-and-gate', [sys.executable, str(study / 'confirmation-tables.py')]),
    ('attribution-tables', [sys.executable, str(study / 'attribution-tables.py')]),
]:
    with (r / (label + '-delivery.log')).open('x') as log:
        process = subprocess.run(command, cwd=source, stdout=log, stderr=subprocess.STDOUT)
    commands.append({'label': label, 'command': command, 'exit': process.returncode})
    assert process.returncode == 0, label
arm = 'candidate' if results['decision']['runtimeQualifies'] else 'base'
paths = ['src/client/visibility.rs', 'src/client/simulation/food.rs']
with tarfile.open(r / (arm + '.tar')) as archive:
    for path in paths:
        assert (source / path).read_bytes() == archive.extractfile(path).read(), path
out = {'commands': commands, 'bothSeriesCheckedSeparately': True,
       'finalRuntimeSourceEqualsConfirmationArm': arm, 'bothRuntimeFilesByteChecked': True}
if arm == 'base':
    patch = study / 'rejected-layout.patch'
    subprocess.run(['git', 'apply', '--unidiff-zero', str(patch)], cwd=source, check=True)
    with tarfile.open(r / 'candidate.tar') as archive:
        for path in paths:
            assert (source / path).read_bytes() == archive.extractfile(path).read(), path
    subprocess.run(['git', 'apply', '--unidiff-zero', '-R', str(patch)], cwd=source, check=True)
    with tarfile.open(r / 'base.tar') as archive:
        for path in paths:
            assert (source / path).read_bytes() == archive.extractfile(path).read(), path
    out['rejectedPatchRoundTripMatchesBothConfirmationArms'] = True
with tarfile.open(r / 'final-delivery.tar') as archive:
    out['deliveryArchiveCommit'] = archive.pax_headers['comment']
(r / 'confirmation-postprocessing-checks.json').write_text(json.dumps(out, indent=2) + '\n')
print(json.dumps(out), flush=True)
