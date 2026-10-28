"""Reconstruct every measured Rust source without compiling or rerunning work."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

study = Path(__file__).resolve().parent
root = Path(sys.argv[1]).resolve()
rows = []
for arm in ['base', 'candidate', 'mutation', 'base-profile', 'candidate-profile']:
    receipt = root / arm / 'pins.json'
    if not receipt.exists():
        continue
    expected = json.loads(receipt.read_text())['sources']
    with tempfile.TemporaryDirectory(prefix='reconstruct-', dir=root) as name:
        out = Path(name)
        for file in ['build.py', 'fixture.rs']:
            shutil.copy2(study / file, out / file)
        (out / 'shared').symlink_to(root / 'shared', target_is_directory=True)
        for file in ['base.tar', 'candidate.tar']:
            (out / file).symlink_to(root / file)
        command = [sys.executable, str(out / 'build.py'), arm.removesuffix('-profile'), '--prepare-only']
        if arm.endswith('-profile'):
            command.append('--profile')
        actual = json.loads(subprocess.check_output(command, text=True))['sources']
        assert actual == expected, (arm, [key for key in actual.keys() | expected.keys()
                                         if actual.get(key) != expected.get(key)])
    rows.append({'arm': arm, 'allRustSourceHashesMatch': True, 'files': len(expected)})
    print('SOURCE HASHES MATCH', arm, len(expected), flush=True)
(root / 'reconstruction.json').write_text(json.dumps(rows, indent=2) + '\n')
