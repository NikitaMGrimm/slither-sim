"""Select actual exactness receipts after all counted and direct-parent windows."""
from pathlib import Path
import json,tarfile

r = Path(__file__).resolve().parent
assert (r / 'ALL-COHORTS-DONE').exists()
checks = json.loads((r / 'required-checks-pass.json').read_text())
assert checks['passed'] and checks['sourceRestored']
assert len(checks['checks']) == 7
assert all(v['exit'] == (101 if v['expectedFailure'] else 0) for v in checks['checks'])
assert sum(v['expectedFailure'] for v in checks['checks']) == 4
with tarfile.open(r / 'candidate.tar') as archive:
    assert (r / 'checks/source/src/client/visibility.rs').read_bytes() == archive.extractfile('src/client/visibility.rs').read()
parity = json.loads((r / 'parity.json').read_text())
assert len(parity) == 27 and all(p['base'] == p['candidate'] for p in parity)
wire = json.loads((r / 'wire-mutations.json').read_text())
assert len(wire) == 2 and all(v['fullTraceMutationDetected'] and v['wirePayloadMutationDetected'] for v in wire)
stream = json.loads((r / 'stream-mutation.json').read_text())
assert stream['expectedFailure'] and stream['detected'] and stream['exit'] != 0
documentation = (r / 'documentation-tests.log').read_text()
assert 'Ran 7 tests' in documentation and documentation.rstrip().endswith('OK')
results = json.loads((r / 'selected-results.json').read_text())
assert len(results['rows']) == 10
assert all(v['proofs']['completeWarmStreamsEqual'] and v['proofs']['allSixContinuationStreamsEqual']
           and v['proofs']['terminalRecordsValidated'] for v in results['rows'])
out = {
    'prerequisiteReceipt': checks, 'sourceRestoreAlsoComparedToCandidateArchive': True,
    'catalogAndHostTraces': parity, 'wireMutationControls': wire,
    'streamByteMutation': stream,
    'documentation': {'command': ['python3', '-B', '-m', 'unittest', 'discover',
                                   '-s', 'tools', '-p', 'test_documentation.py'],
                      'exit': 0, 'tests': 7},
    'allTenCohortsHaveEqualCompleteWarmAndSixContinuationStreams': True,
    'streamReceipts': 'results.json rows[].proofs',
}
(r / 'proofs.json').write_text(json.dumps(out, indent=2) + '\n')
print('Actual oracle, mutation, restore, 27-trace and ten-cohort stream receipts selected.', flush=True)
