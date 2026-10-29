"""Released Linux bench only: build pre-staged, hash-pinned source without Git.

Usage: python build_staged.py OUTPUT ARM
The local prepare.py STUDY_STAGE_ONLY=1 stage is transferred intact first.
"""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys
root=Path(sys.argv[1]).resolve();arm=sys.argv[2];source=root/arm/'source'
assert sys.platform=='linux'

def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

receipt=json.loads((root/arm/'source-stage.json').read_text())
assert receipt['sources']=={p.relative_to(source).as_posix():sha(p) for p in source.rglob('*.rs')}
assert receipt['files']=={p.relative_to(source).as_posix():sha(p) for p in source.rglob('*') if p.is_file()}
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
target=(root/'target').resolve();assert target.parent==root and target.name=='target'
env=dict(os.environ,CARGO_BUILD_JOBS='1',RUST_TEST_THREADS='1',CARGO_TARGET_DIR=str(target),
    CARGO_INCREMENTAL='0',CARGO_PROFILE_RELEASE_DEBUG='0',RAYON_NUM_THREADS='1',CARGO_NET_OFFLINE='true',
    RUSTC=compiler,RUSTDOC=str(Path(compiler).with_name('rustdoc')),RUSTFLAGS='-C target-cpu=native')
env.pop('CARGO_ENCODED_RUSTFLAGS',None)
command=['cargo','build','--release','--locked','--offline','-j','1','--example','study_fixed',
         '--example','study_protocols','--example','perf_baseline']
clean=['cargo','clean','--release','--locked','--offline','-p','slither-sim','--target-dir',str(target)]
with (root/arm/'native-build.log').open('x') as log:
    # Independently pre-staged trees can be older than the shared artifact.
    # Rebuild this package per arm; retain cached dependencies in this sole
    # task-owned target. Copied proof binaries live outside it.
    subprocess.run(clean,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
    subprocess.run(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
bins=root/arm/'bin';bins.mkdir()
for binary in ['study_fixed','study_protocols','perf_baseline']:
    shutil.copy2(root/'target/release/examples'/binary,bins/binary)
(root/arm/'pins.json').write_text(json.dumps({**receipt,'releaseBuild':True,'traceOnly':False,
    'command':command,'cleanCommand':clean,'rustflags':env['RUSTFLAGS'],'toolchain':subprocess.check_output([compiler,'--version'],text=True).strip(),
    'binaries':{p.name:sha(p) for p in bins.iterdir()}},indent=2)+'\n')
print('NATIVE BUILT',arm,flush=True)
