"""Retained-listing ownership: exact packet oracles, mutants, restore, Rust group."""
from pathlib import Path
import json,os,subprocess,tarfile
r=Path(__file__).resolve().parent;out=r/'checks';source=out/'source';source.mkdir(parents=True)
with tarfile.open(r/'candidate.tar') as archive:archive.extractall(source,filter='data')
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/slither-maintenance-round2-target',CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_NET_OFFLINE='true',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTC=compiler,RUSTDOC=str(Path(compiler).with_name('rustdoc')),RUSTFLAGS='-C target-cpu=native')
env.pop('CARGO_ENCODED_RUSTFLAGS',None);os.sched_setaffinity(0,{0});rows=[]
def run(command,label,failure=False):
    with (out/(label+'.log')).open('x') as log:proc=subprocess.run(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
    text=(out/(label+'.log')).read_text()
    assert (proc.returncode!=0 and 'test result: FAILED' in text) if failure else proc.returncode==0,(label,proc.returncode)
    rows.append({'label':label,'command':command,'expectedFailure':failure,'exit':proc.returncode})
    (out/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
    print(label,proc.returncode,flush=True)
suite=['cargo','test','--locked','--lib','client::simulation::food::tests']
run(suite,'food-ownership-oracles')
file=source/'src/client/simulation/food.rs';before=file.read_bytes()
for label,needle,replacement in [
    ('lost-unchanged-pickup','if !unchanged || changes.eaten.contains(&sector) {','if !unchanged {'),
    ('ignored-listing-change','let unchanged = Arc::ptr_eq(&previous, current);','let unchanged = true;'),
    ('ignored-held-listing','if Arc::ptr_eq(previous, &diff.previous) && Arc::ptr_eq(current, &diff.current) {','if Arc::ptr_eq(current, &diff.current) {'),
]:
    try:
        text=before.decode();assert text.count(needle)==1,(label,text.count(needle))
        file.write_text(text.replace(needle,replacement),newline='\n')
        run(suite,label,True)
    finally:file.write_bytes(before)
run(suite,'restored-food-oracles')
run(['python3','tools/check.py','rust'],'rust-group')
(r/'required-checks-pass.json').write_text(json.dumps({'passed':True,'checks':rows},indent=2)+'\n')
print('HELD-LISTING CHECKS COMPLETE',flush=True)
