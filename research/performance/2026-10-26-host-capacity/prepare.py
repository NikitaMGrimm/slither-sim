"""Build an isolated source snapshot; never modify the measured host sources."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

HERE = Path(__file__).resolve().parent

def prepare(repo, out, ref):
    out.mkdir(parents=True, exist_ok=False)
    archive = out / 'source.tar'
    subprocess.run(['git', '-C', str(repo), 'archive', '--format=tar', '-o', str(archive), ref], check=True)
    source = out / 'source'
    source.mkdir()
    with tarfile.open(archive) as tar:
        tar.extractall(source, filter='data')
    for name in ['host_capacity_load', 'host_probe']:
        shutil.copyfile(HERE / (name + '.rs'), source / 'examples' / (name + '.rs'))
    pin = {'sourceCommit': subprocess.check_output(['git', '-C', str(repo), 'rev-parse', ref], text=True).strip(),
           'archiveSha256': hashlib.sha256(archive.read_bytes()).hexdigest(),
           'harnessSha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in HERE.iterdir() if p.is_file()}}
    (out / 'source-pins.json').write_text(json.dumps(pin, indent=2) + '\n', encoding='utf-8')
    return source

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--ref', default='origin/main')
    parser.add_argument('--build', action='store_true')
    args = parser.parse_args()
    root = Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip())
    source = prepare(root, args.output.resolve(), args.ref)
    if args.build:
        env = dict(os.environ, CARGO_BUILD_JOBS='2', CARGO_TARGET_DIR=str(args.output.resolve() / 'target'), CARGO_PROFILE_RELEASE_DEBUG='1')
        subprocess.run(['cargo', 'build', '--release', '--locked', '--bin', 'slither-sim', '--example', 'host_capacity_load', '--example', 'host_probe'], cwd=source, env=env, check=True)
