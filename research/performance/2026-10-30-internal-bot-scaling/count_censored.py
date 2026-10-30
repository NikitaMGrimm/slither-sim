"""Count the final guarded 10k snapshot without extending its warmup."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
from run import CHILD_TIMEOUT_SECONDS, digest, emit


def execute(root,study):
    root=Path(root).resolve();study=Path(study)
    directory=root/'results/10000';fixture=directory/'fixture'
    qualification=json.loads((directory/'warm-qualification.json').read_text())
    assert not qualification['stabilized']
    master=json.loads((root/'measurement-pins.json').read_text())
    binary=root/'target/release/examples/internal_scaling'
    assert digest(binary)==master['binarySha256']
    verified={}
    def measure(path,mode,kind):
        assert not path.exists()
        subprocess.run([str(binary),'measure',str(fixture),mode,'512',kind,str(path)],check=True,timeout=CHILD_TIMEOUT_SECONDS)
        return json.loads(path.read_text())
    for mode in ['serial','parallel']:
        verified[mode]=measure(directory/f'verify-{mode}.json',mode,'verify')
    for key in ['actionSha256','finalStateSha256']:
        assert verified['serial'][key]==verified['parallel'][key]
    assert digest(directory/'verify-serial.tape')==digest(directory/'verify-parallel.tape')
    shutil.copyfile(directory/'verify-serial.tape',fixture/'tape.bin')
    reference=verified['serial']['finalStateSha256']
    for pair in range(-1,5):
        order=['serial','parallel'] if pair%2==0 else ['parallel','serial']
        order=(['simulation']+order) if pair%2 else (order+['simulation'])
        for mode in order:
            emit('guarded-measurement-start',pair=pair,mode=mode)
            row=measure(directory/f'pair-{pair}-{mode}.json',mode,'plain')
            assert row['finalStateSha256']==reference
            emit('guarded-measurement-end',pair=pair,mode=mode)
    for mode in ['serial','parallel']:
        assert measure(directory/f'profile-{mode}.json',mode,'profile')['finalStateSha256']==reference
    receipt={'scope':'final guarded checkpoint; ecology unsettled; no extra warmup',
             'masterPinsSha256':digest(root/'measurement-pins.json'),
             'amendmentSha256':digest(study/'AMENDMENT.md'),
             'controllerSha256':digest(Path(__file__)),'allFinalStateSha256':reference}
    with (directory/'censored-counts.json').open('x') as out:out.write(json.dumps(receipt,indent=2)+'\n')
    emit('guarded-counting-complete',**receipt)


if __name__=='__main__':execute(*sys.argv[1:])
