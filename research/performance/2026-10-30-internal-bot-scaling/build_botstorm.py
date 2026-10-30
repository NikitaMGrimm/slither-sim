"""Generate/build the added comparison after A4 counting has completely ended."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def build(root, study):
    root=Path(root).resolve();study=Path(study).resolve()
    events=[json.loads(line) for line in (root/'run.log').read_text().splitlines() if line.startswith('{')]
    assert events[-1]['event']=='complete','Do not build during A4 counting'
    pins=json.loads((root/'measurement-pins.json').read_text())
    original=root/'target/release/examples/internal_scaling'
    assert sha(original)==pins['binarySha256']
    saved=root/'bin';saved.mkdir(exist_ok=True)
    shutil.copy2(original,saved/'internal_scaling')
    source=root/'examples/internal_scaling.rs'
    assert sha(source)==pins['harnessSha256']
    text=source.read_text()
    assert text.count('fn main() {')==1
    generated=root/'examples/internal_botstorm.rs'
    generated.write_text('#![allow(dead_code, unused_imports)]\n'+text.split('fn main() {')[0]+(study/'botstorm.rs').read_text())
    env=dict(os.environ,CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'))
    for name in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_PROFILE_RELEASE_DEBUG']:
        env.pop(name,None)
    for name,tool in [('RUSTC','rustc'),('RUSTDOC','rustdoc')]:
        env[name]=subprocess.check_output(['rustup','which',tool],text=True).strip()
    command=['cargo','build','--release','--locked','--no-default-features','--features','bot,parallel','--example','internal_botstorm']
    with (root/'build-botstorm.log').open('wb') as out:
        subprocess.run(command,cwd=root,env=env,stdout=out,stderr=subprocess.STDOUT,check=True)
    binary=root/'target/release/examples/internal_botstorm'
    shutil.copy2(binary,saved/'internal_botstorm')
    values={'masterPinsSha256':sha(root/'measurement-pins.json'),'baseHarnessSha256':sha(source),
            'extensionSha256':sha(study/'botstorm.rs'),'builderSha256':sha(Path(__file__)),
            'generatedHarnessSha256':sha(generated),'binarySha256':sha(binary),
            'buildLogSha256':sha(root/'build-botstorm.log'),'command':command,
            'profile':'unchanged portable release, debug=0, no Rust flags'}
    (root/'botstorm-build.json').write_text(json.dumps(values,indent=2)+'\n')
    print(json.dumps(values,indent=2))


if __name__=='__main__':build(*sys.argv[1:])
