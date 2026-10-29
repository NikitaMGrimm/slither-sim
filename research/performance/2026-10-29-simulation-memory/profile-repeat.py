import os,subprocess,json,hashlib,shutil
from pathlib import Path
r=Path(__file__).resolve().parent
assert not (r/'PROFILE-DONE').exists()
p=r/'profile';p.mkdir()
text=(r/'source/src/study_events.c').read_text();assert text.count('1000003:1009;')==1
(p/'events.c').write_text(text.replace('1000003:1009;','1000003:10007;'))
subprocess.run(['cc','-O2','-c',str(p/'events.c'),'-o',str(p/'events.o')],check=True)
subprocess.run(['ar','rcs',str(p/'libmemdiag.a'),str(p/'events.o')],check=True)
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/slither-simulation-memory-target',CARGO_BUILD_JOBS='2',CARGO_PROFILE_RELEASE_DEBUG='1',CARGO_INCREMENTAL='0',RUSTC=compiler,RUSTDOC=str(Path(compiler).with_name('rustdoc')),RUSTFLAGS=f'-L native={p} -l static=memdiag',CARGO_NET_OFFLINE='true')
env.pop('CARGO_ENCODED_RUSTFLAGS',None)
cmd=['cargo','build','--release','--locked','--offline','-j2','--example','study_host']
with (p/'build.log').open('w') as log:subprocess.run(cmd,cwd=r/'source',env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
shutil.copy2('/tmp/slither-simulation-memory-target/release/examples/study_host',r/'study_host-profile')
(p/'build.json').write_text(json.dumps({'amendmentCommit':'9c060bdb','period':10007,'binarySha256':hashlib.file_digest((r/'study_host-profile').open('rb'),'sha256').hexdigest(),'command':cmd,'rustflags':env['RUSTFLAGS']},indent=2))
driver=(r/'run.py').read_text().replace("exe=r/'study_host'","exe=r/'study_host-profile'").replace("label='legacy' if legacy else 'stock'","label='profile-legacy' if legacy else 'profile-stock'")
driver=driver.replace("jobs=[(event,rep,False) for rep in range(3) for event in [1,2,3,5,6,7]]+[(event,0,True) for event in [1,3,5,6]]","jobs=[(event,0,True) for event in [3,6]]")
driver=driver.replace("r/'runs.json'","r/'profile-runs.json'").replace("r/'DONE'","r/'PROFILE-DONE'")
(r/'profile-run.py').write_text(driver)
subprocess.run(['python3',str(r/'profile-run.py')],check=True)
print('LOWER-RATE PROFILES COMPLETE',flush=True)
