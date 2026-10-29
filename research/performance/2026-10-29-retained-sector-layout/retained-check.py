"""Prerequisite receipt from actual visibility commands, mutations and restore."""
from pathlib import Path
import tarfile,subprocess,json,os,sys,hashlib
r=Path(__file__).resolve().parent;source=r/'checks/source';source.mkdir(parents=True)
with tarfile.open(r/'candidate.tar') as t:t.extractall(source,filter='data')
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/slither-retained-layout-target',CARGO_BUILD_JOBS='2',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_NET_OFFLINE='true',RUST_TEST_THREADS='1',RAYON_NUM_THREADS='1',RUSTC=compiler,RUSTDOC=str(Path(compiler).with_name('rustdoc')),RUSTFLAGS='')
env.pop('CARGO_ENCODED_RUSTFLAGS',None);rows=[]
def run(label,args,failure=False):
 with (r/'checks'/(label+'.log')).open('x') as log:p=subprocess.run(args,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
 text=(r/'checks'/(label+'.log')).read_text()
 assert (p.returncode!=0 and 'test result: FAILED' in text) if failure else p.returncode==0,(label,p.returncode)
 rows.append({'label':label,'command':args,'expectedFailure':failure,'exit':p.returncode});(r/'checks/results.json').write_text(json.dumps(rows,indent=2)+'\n');print(label,'detected' if failure else 'PASS',flush=True)
suite=['cargo','test','--locked','--offline','-j2','--lib','client::visibility::tests']
run('focused-visibility-oracles',suite)
f=source/'src/client/visibility.rs';before=f.read_bytes()
for label,needle,replacement in [
 ('wrong-outgoing-time','self.set_seen(sector, self.touched);','self.set_seen(sector, now);'),
 ('missing-reentry-refresh','if refresh_oldest {','if false && refresh_oldest {'),
 ('added-deadline','now - oldest > RETENTION_SECONDS','now > oldest + RETENTION_SECONDS'),
 ('misaligned-compaction','self.last_seen[kept] = seen;','self.last_seen[kept] = self.last_seen[0];')]:
 try:
  text=before.decode();assert text.count(needle)==1,label
  f.write_text(text.replace(needle,replacement))
  run(label,suite,True)
 finally:f.write_bytes(before)
assert f.read_bytes()==before
run('restored-visibility-oracles',suite)
run('rust-group',[sys.executable,'tools/check.py','rust'])
(r/'required-checks-pass.json').write_text(json.dumps({'passed':True,'sourceRestored':True,'candidateArchiveSha256':hashlib.file_digest((r/'candidate.tar').open('rb'),'sha256').hexdigest(),'checks':rows},indent=2)+'\n')
print('PRIMARY CHECKS COMPLETE',flush=True)
