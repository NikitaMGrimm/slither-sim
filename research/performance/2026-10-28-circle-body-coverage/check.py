"""Focused independent oracle and mutation, then required offline Rust group."""
import hashlib,json,os,subprocess,tarfile,time
from pathlib import Path
root=Path(__file__).resolve().parent
while not (root/'base/pins.json').exists():
 if (root/'candidate/build.log').exists() and 'error[' in (root/'candidate/build.log').read_text():raise RuntimeError('candidate build failed')
 time.sleep(.5)
checks=root/'checks';checks.mkdir();source=checks/'source';source.mkdir()
with tarfile.open(root/'candidate.tar') as tar:tar.extractall(source,filter='data')
os.sched_setaffinity(0,{3})
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'),CARGO_BUILD_JOBS='1',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_NET_OFFLINE='true',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTC=compiler,RUSTDOC=str(Path(compiler).with_name('rustdoc')))
rows=[]
def run(command,label,fails=False):
 with (checks/(label+'.log')).open('x') as log:proc=subprocess.run(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
 text=(checks/(label+'.log')).read_text()
 assert (proc.returncode!=0 and 'test result: FAILED' in text) if fails else proc.returncode==0,(label,proc.returncode)
 rows.append({'label':label,'command':command,'expectedFailure':fails,'exit':proc.returncode});(checks/'results.json').write_text(json.dumps(rows,indent=2)+'\n');print(label,proc.returncode,flush=True)
command=['cargo','test','--locked','--lib','body_bounds_match_original_coverage']
run(command,'body-oracle')
run(['cargo','test','--release','--locked','--lib','body_bounds_match_original_coverage'],'body-oracle-release')
f=source/'src/client/simulation/snakes.rs';before=f.read_bytes();needle='if !bounds.near(query, reach) {'
try:
 text=before.decode();assert text.count(needle)==1
 f.write_text(text.replace(needle,'if !bounds.near(query, vision - 1.) {'),newline='\n');run(command,'narrowed-block-mutation',True)
finally:f.write_bytes(before)
assert hashlib.sha256(f.read_bytes()).digest()==hashlib.sha256(before).digest()
run(command,'restored-oracle')

f=source/'src/client/simulation/snakes.rs';before=f.read_bytes()
try:
 text=before.decode();needle='if dx * dx + dy * dy > reach * reach {';assert text.count(needle)==1
 f.write_text(text.replace(needle,'if dx * dx + dy * dy > (vision - 1.) * (vision - 1.) {'),newline='\n')
 run(command,'narrowed-circle-mutation',True)
finally:f.write_bytes(before)
run(command,'restored-circle-oracle')
run(['python3','tools/check.py','rust'],'rust-group')
(root/'required-checks-pass.json').write_text(json.dumps({'passed':True,'checks':rows},indent=2)+'\n')
print('REQUIRED CHECKS PASS',flush=True)
