"""Bounded post-churn core continuation; do not overwrite either failed pilot."""
import json,os,shutil,subprocess,sys
from pathlib import Path
root=Path(sys.argv[1]).resolve();study=Path(__file__).resolve().parent
os.sched_setaffinity(0,{0})
source=root/'generator/source';shutil.copytree(root/'base/source',source)
shutil.copy2(study/'extend.rs',source/'src/client/simulation/study_extend.rs')
p=source/'src/client/simulation.rs';p.write_text(p.read_text()+'\npub mod study_extend;\n')
(source/'examples/study_extend.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_extend::run(std::path::Path::new(&a[1]),std::path::Path::new(&a[2]));}\n')
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'),CARGO_BUILD_JOBS='1',CARGO_PROFILE_RELEASE_DEBUG='1',RAYON_NUM_THREADS='1',RUSTC=compiler)
with (root/'generator/build.log').open('x') as log:subprocess.run(['cargo','build','--release','--locked','--offline','--example','study_extend'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
binary=root/'generator/study_extend';shutil.copy2(root/'target/release/examples/study_extend',binary)
import hashlib
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
pins={'binarySha256':sha(binary),'parentCheckpointSha256':sha(root/'warm/unqualified-checkpoint.json'),
      'fixtureSha256':sha(study/'extend.rs'),'driverSha256':sha(Path(__file__)),
      'sources':{str(p.relative_to(source)):sha(p) for p in source.rglob('*.rs')}}
(root/'generator/pins.json').write_text(json.dumps(pins,indent=2)+'\n')
with (root/'extended.stdout').open('x') as out,(root/'extended.stderr').open('x') as err:
    subprocess.run([str(binary),str(root/'warm/unqualified-checkpoint.json'),str(root/'extended')],env=env,stdout=out,stderr=err,check=True)
assert sha(binary)==pins['binarySha256']
shutil.move(str(root/'warm'),root/'warm-churn-180k')
shutil.move(str(root/'extended'),root/'warm')
(root/'checkpoint-sha256.txt').write_text(sha(root/'warm/checkpoint.json')+'\n')
print('QUALIFIED', (root/'warm/meta.json').read_text(),flush=True)
