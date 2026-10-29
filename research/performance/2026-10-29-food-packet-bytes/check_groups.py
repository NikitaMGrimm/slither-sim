"""Group storage/old-view controls, source restore, then the offline Rust group."""
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
run(['cargo','test','--locked','--lib','client::'],'client-group-oracles')
file=source/'src/client/simulation/food.rs';before=file.read_bytes()
storage='shared_groups_preserve_packet_boundaries_and_outlive_the_cache'
for label,needle,replacement,test in [
    ('lost-packet-boundary','            .map(Vec::as_slice)\n    }','            .skip(1).map(Vec::as_slice)\n    }',storage),
    ('copied-recipient-groups','self.groups.push(FoodGroup::Shared(packets));','self.groups.push(FoodGroup::Shared(Rc::new((*packets).clone())));',storage),
    ('ignored-held-listing','if Arc::ptr_eq(previous, &diff.previous) && Arc::ptr_eq(current, &diff.current) {','if Arc::ptr_eq(current, &diff.current) {','shared_diffs_match_uncached_lifetimes_views_and_older_recipients'),
]:
    try:
        text=before.decode();assert text.count(needle)==1,(label,text.count(needle))
        file.write_text(text.replace(needle,replacement),newline='\n')
        run(['cargo','test','--locked','--lib',test],label,True)
    finally:file.write_bytes(before)
run(['cargo','test','--locked','--lib','client::'],'restored-client-oracles')
run(['python3','tools/check.py','rust'],'rust-group')
(r/'required-checks-pass.json').write_text(json.dumps({'passed':True,'checks':rows},indent=2)+'\n')
print('GROUP CHECKS COMPLETE',flush=True)
