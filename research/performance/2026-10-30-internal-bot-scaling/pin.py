"""Freeze a completed remote build and smoke proof, before primary timing.

python3 pin.py EXECUTION_DIRECTORY ORIGINAL_RUNTIME_ARCHIVE RUNTIME_REVISION
The execution directory must have build-final.log and the smoke fixture.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def digest(path):
    value=hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda:stream.read(1<<20),b''):
            value.update(chunk)
    return value.hexdigest()


def pin(root, archive, revision):
    r=Path(root).resolve()
    output=r/'measurement-pins.json'
    assert not output.exists(), 'Never overwrite an existing measurement pin'
    binary=r/'target/release/examples/internal_scaling'
    for mode in ['serial','parallel']:
        subprocess.run([str(binary),'measure',str(r/'smoke'),mode,'24','verify',
                        str(r/f'final-pilot-{mode}.json')],check=True)
    (r/'smoke/tape.bin').write_bytes((r/'final-pilot-serial.tape').read_bytes())
    subprocess.run([str(binary),'measure',str(r/'smoke'),'simulation','24','plain',
                    str(r/'final-pilot-simulation.json')],check=True)
    rows={mode:json.loads((r/f'final-pilot-{mode}.json').read_text())
          for mode in ['serial','parallel','simulation']}
    assert len({row['finalStateSha256'] for row in rows.values()})==1
    assert rows['serial']['actionSha256']==rows['parallel']['actionSha256']
    assert digest(r/'final-pilot-serial.tape')==digest(r/'final-pilot-parallel.tape')
    manifest=''.join(f'{path.relative_to(r).as_posix()}\0{digest(path)}\n'
                     for path in sorted((r/'src').rglob('*')) if path.is_file())
    manifest+='examples/internal_scaling.rs\0'+digest(r/'examples/internal_scaling.rs')+'\n'
    (r/'execution-source-manifest.txt').write_text(manifest)
    values={'runtime':revision,'sourceArchiveSha256':digest(archive),
            'executionManifestSha256':digest(r/'execution-source-manifest.txt'),
            'harnessSha256':digest(r/'examples/internal_scaling.rs'),
            'runnerSha256':digest(r/'run.py'),'planSha256':digest(r/'PLAN.md'),
            'binarySha256':digest(binary),'cargoLockSha256':digest(r/'Cargo.lock'),
            'buildLogSha256':digest(r/'build-final.log'),
            'profile':'repository default portable release; lto=thin, codegen-units=1, debug=0; empty Rust flags',
            'command':'cargo build --release --locked --no-default-features --features bot,parallel --example internal_scaling',
            'rustc':subprocess.check_output(['rustc','-Vv'],text=True),
            'finalSmoke':{'population':1000,'ticks':24,'serialParallelActionsEqual':True,
                          'allThreeFinalStatesEqual':True,'serialParallelTapesEqual':True}}
    with output.open('x') as stream:
        stream.write(json.dumps(values,indent=2)+'\n')
    return values


if __name__=='__main__':
    print(json.dumps(pin(*sys.argv[1:]),indent=2))
