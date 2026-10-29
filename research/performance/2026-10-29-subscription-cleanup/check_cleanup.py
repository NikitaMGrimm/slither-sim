"""Full-sweep oracles and meaningful mutation controls, then offline Rust checks."""
import json,os,subprocess,tarfile
from pathlib import Path
r=Path(__file__).resolve().parent;out=r/'checks';source=out/'source';source.mkdir(parents=True)
with tarfile.open(r/'candidate.tar') as f:f.extractall(source,filter='data')
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/slither-maintenance-round2-target',CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_NET_OFFLINE='true',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTC=compiler,RUSTDOC=str(Path(compiler).with_name('rustdoc')),RUSTFLAGS='-C target-cpu=native')
env.pop('CARGO_ENCODED_RUSTFLAGS',None);os.sched_setaffinity(0,{0});rows=[]
def run(command,label,failure=False):
    with (out/(label+'.log')).open('x') as f:p=subprocess.run(command,cwd=source,env=env,stdout=f,stderr=subprocess.STDOUT)
    log=(out/(label+'.log')).read_text()
    assert (p.returncode!=0 and 'test result: FAILED' in log) if failure else p.returncode==0,(label,p.returncode)
    rows.append({'label':label,'command':command,'expectedFailure':failure,'exit':p.returncode});(out/'results.json').write_text(json.dumps(rows,indent=2)+'\n');print(label,p.returncode,flush=True)
run(['cargo','test','--locked','--lib','client::visibility::tests'],'visibility-oracles')
f=source/'src/client/visibility.rs';before=f.read_bytes()
for label,needle,replacement in [
    ('wrong-stencil-touch','note_oldest(&mut self.oldest, sector, self.touched);','note_oldest(&mut self.oldest, sector, now);'),
    ('missing-reentry-refresh','if refresh_oldest {','if refresh_oldest && false {'),
    ('rounded-deadline','now - oldest > RETENTION_SECONDS','now > oldest + RETENTION_SECONDS'),
]:
    try:
        text=before.decode();assert text.count(needle)==1
        f.write_text(text.replace(needle,replacement),newline='\n')
        run(['cargo','test','--locked','--lib','client::visibility::tests'],label,True)
    finally:f.write_bytes(before)
run(['cargo','test','--locked','--lib','client::visibility::tests'],'restored-visibility')
if '--focused-only' not in __import__('sys').argv:
    run(['python3','tools/check.py','rust'],'rust-group')
    (r/'required-checks-pass.json').write_text(json.dumps({'passed':True,'checks':rows},indent=2)+'\n')
