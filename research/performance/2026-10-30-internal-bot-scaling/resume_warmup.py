"""Conditional, separately pinned 10k warmup continuation; original data stay intact."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
from run import digest, emit, stable

ENTRY = r'''
fn main() {
    let args:Vec<_>=std::env::args().collect();
    if args[1]!="resume" { return original_main(); }
    pin(0);
    let mut scene=Scene::load(Path::new(&args[2]),true);
    let output=Path::new(&args[3]);
    fs::create_dir_all(output).unwrap();
    let mut hash=Sha256::new();let began=Instant::now();
    eprintln!("{}",json!({"kind":"initial","tick":scene.meta.tick,"census":census(&scene.world)}));
    while scene.meta.tick<257143 {
        let sample=scene.tick(false,&mut hash,false,None);
        if scene.meta.tick.is_multiple_of(1000) {
            eprintln!("{}",json!({"kind":"warm","tick":scene.meta.tick,"last":sample,"census":census(&scene.world)}));
            if output.join("STOP_WARM").exists() || began.elapsed().as_secs()>7200 { break; }
        }
    }
    scene.save(output);
    eprintln!("{}",json!({"kind":"ready","tick":scene.meta.tick,"census":census(&scene.world)}));
}
'''


def execute(root):
    root=Path(root).resolve()
    original_dir=root/'results/10000'
    receipt=json.loads((original_dir/'warm-qualification.json').read_text())
    if receipt['stabilized']:
        emit('continuation-not-needed');return
    meta=json.loads((original_dir/'fixture/meta.json').read_text())
    if meta['tick']>=257143:
        emit('continuation-not-run',reason='absolute virtual-time ceiling already reached');return
    output=root/'results/10000-continuation';output.mkdir(exist_ok=False)
    master=json.loads((root/'measurement-pins.json').read_text())
    original=root/'target/release/examples/internal_scaling'
    assert digest(original)==master['binarySha256']
    source=root/'examples/internal_scaling.rs'
    assert digest(source)==master['harnessSha256']
    text=source.read_text();assert text.count('fn main() {')==1
    generated=root/'examples/internal_scaling_resume.rs'
    generated.write_text(text.replace('fn main() {','fn original_main() {')+ENTRY)
    env=dict(os.environ,CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'))
    for key in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_PROFILE_RELEASE_DEBUG']:env.pop(key,None)
    for key,tool in [('RUSTC','rustc'),('RUSTDOC','rustdoc')]:env[key]=subprocess.check_output(['rustup','which',tool],text=True).strip()
    command=['cargo','build','--release','--locked','--no-default-features','--features','bot,parallel','--example','internal_scaling_resume']
    with (output/'build.log').open('wb') as log:
        subprocess.run(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
    binary=root/'target/release/examples/internal_scaling_resume'
    for label,executable in [('original',original),('resume',binary)]:
        subprocess.run([str(executable),'measure',str(original_dir/'fixture'),'parallel','24','verify',str(output/f'proof-{label}.json')],check=True)
    a=json.loads((output/'proof-original.json').read_text());b=json.loads((output/'proof-resume.json').read_text())
    assert a['finalStateSha256']==b['finalStateSha256'] and a['actionSha256']==b['actionSha256']
    assert digest(output/'proof-original.tape')==digest(output/'proof-resume.tape')
    pins={'masterPinsSha256':digest(root/'measurement-pins.json'),'helperSha256':digest(Path(__file__)),
          'generatedSourceSha256':digest(generated),'binarySha256':digest(binary),
          'buildLogSha256':digest(output/'build.log'),'command':command,
          'continuationProofActionSha256':a['actionSha256'],'continuationProofStateSha256':a['finalStateSha256']}
    (output/'pins.json').write_text(json.dumps(pins,indent=2)+'\n')
    records=[json.loads(line) for line in (original_dir/'warm.jsonl').read_text().splitlines() if json.loads(line)['kind']=='warm']
    fixture=output/'fixture';acceptance=None;began=time.monotonic()
    with (output/'warm.jsonl').open('w') as log:
        child=subprocess.Popen([str(binary),'resume',str(original_dir/'fixture'),str(fixture)],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True)
        try:
            for line in child.stderr:
                log.write(line);log.flush();row=json.loads(line)
                if row['kind']=='warm':
                    records.append(row)
                    emit('continuation-warm',tick=row['tick'],meanLength=row['census']['meanLength'],elapsedSeconds=time.monotonic()-began)
                    if acceptance is None:
                        acceptance=stable(records,10000)
                        if acceptance is not None:(fixture/'STOP_WARM').write_text('first original stability screen passed\n')
            assert child.wait()==0
        finally:
            if child.poll() is None:child.terminate();child.wait(timeout=15)
    qualification={'population':10000,'stabilized':acceptance is not None,'acceptance':acceptance,
                   'originalAttempt':'10000','elapsedSeconds':time.monotonic()-began,
                   'fixtureSha256':digest(fixture/'world.json'),'metaSha256':digest(fixture/'meta.json')}
    (output/'warm-qualification.json').write_text(json.dumps(qualification,indent=2)+'\n')
    emit('continuation-warm-finished',**qualification)
    if acceptance is None:return
    verified={}
    for mode in ['serial','parallel']:
        path=output/f'verify-{mode}.json'
        subprocess.run([str(original),'measure',str(fixture),mode,'512','verify',str(path)],check=True)
        verified[mode]=json.loads(path.read_text())
    for key in ['actionSha256','finalStateSha256']:assert verified['serial'][key]==verified['parallel'][key]
    assert digest(output/'verify-serial.tape')==digest(output/'verify-parallel.tape')
    shutil.copyfile(output/'verify-serial.tape',fixture/'tape.bin')
    reference=verified['serial']['finalStateSha256']
    for pair in range(-1,5):
        order=['serial','parallel'] if pair%2==0 else ['parallel','serial']
        order=(['simulation']+order) if pair%2 else (order+['simulation'])
        for mode in order:
            path=output/f'pair-{pair}-{mode}.json'
            emit('continuation-measurement-start',pair=pair,mode=mode)
            subprocess.run([str(original),'measure',str(fixture),mode,'512','plain',str(path)],check=True)
            assert json.loads(path.read_text())['finalStateSha256']==reference
            emit('continuation-measurement-end',pair=pair,mode=mode)
    for mode in ['serial','parallel']:
        path=output/f'profile-{mode}.json'
        subprocess.run([str(original),'measure',str(fixture),mode,'512','profile',str(path)],check=True)
        assert json.loads(path.read_text())['finalStateSha256']==reference
    assert digest(original)==master['binarySha256'] and digest(binary)==pins['binarySha256']
    with (root/'selection.json').open('x') as stream:
        stream.write(json.dumps({'10000':'10000-continuation'},indent=2)+'\n')
    emit('continuation-complete')


if __name__=='__main__':execute(sys.argv[1])
