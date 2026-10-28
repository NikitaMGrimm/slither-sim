"""Post-#249 SHORT-body diagnostic brackets; no speed or mature-host claim.

ROOT holds base.tar/candidate.tar and one bounded target. REFERENCE holds the
accepted plain candidate pair stderr files from the preceding study. Never
run this diagnostic concurrently with a timing/build worker.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import tempfile

study = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    'shared', study.parent / '2026-10-28-broadcast-distance/reproduce.py')
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('mode', choices=['build', 'profile', 'reconstruct'])
p.add_argument('root', type=Path)
p.add_argument('--reference', type=Path)
p.add_argument('--cpu', type=int, default=0)
a = p.parse_args()
root = a.root.resolve()
os.sched_setaffinity(0, {a.cpu})
original = shared.prepare


def prepare(root, arm, profile):
    name, source, pin = original(root, arm, profile)
    if profile:
        f = source / 'src/simulator.rs'
        shared.swap(f,
            'fn compact_food(food: &mut Vec<Food>, removed: impl Iterator<Item = usize>) {',
            'fn compact_food(food: &mut Vec<Food>, removed: impl Iterator<Item = usize>) {\nlet _compact=crate::study_counter::enter(10);')
        shared.swap(f,
            '        // Stable-slot indexing avoids searching the living list for every pair.',
            '        let collision_build=crate::study_counter::enter(11);\n        // Stable-slot indexing avoids searching the living list for every pair.')
        shared.swap(f, '        candidates.clear();\n        dead.clear();',
            '        drop(collision_build);let collision_query=crate::study_counter::enter(12);\n        candidates.clear();\n        dead.clear();')
        shared.swap(f, '        if PROFILE {\n            profile.collision_seconds',
            '        drop(collision_query);\n        if PROFILE {\n            profile.collision_seconds')
    return name, source, pin


shared.prepare = prepare
if a.mode == 'build':
    shared.build(root, 'candidate', True)
elif a.mode == 'reconstruct':
    expected = json.loads((root / 'candidate-profile/pins.json').read_text())['sources']
    with tempfile.TemporaryDirectory(prefix='reconstruct-', dir=root) as temp:
        temp = Path(temp)
        (temp / 'candidate.tar').symlink_to(root / 'candidate.tar')
        _, source, _ = prepare(temp, 'candidate', True)
        actual = {str(f.relative_to(source)): shared.sha(f) for f in source.rglob('*.rs')}
        assert actual == expected, 'generated Rust source differs from measured diagnostic'
    print('Every generated Rust source hash matches the measured diagnostic')
else:
    assert a.reference is not None, 'plain accepted pair summaries are required'
    results = {}
    for case in ['official500', 'official500-legacy', 'official2281']:
        label = 'remaining-' + case
        shared.inventory(root, label + '-before')
        counts = shared.run(root, 'candidate-profile', 'study_protocols', [case], label)
        shared.inventory(root, label + '-after')
        assert (root / (label + '.stderr')).read_bytes() == (
            a.reference / f'pair-{case}-0-candidate.stderr').read_bytes()
        results[case] = counts
        (root / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
        print(case, json.dumps(counts), flush=True)
