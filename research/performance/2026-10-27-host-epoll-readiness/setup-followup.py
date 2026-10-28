"""Build symmetric diagnostic binaries under the original shared lock."""
from pathlib import Path
import fcntl,hashlib,importlib.util,json,os,shutil,subprocess
if not __debug__:raise SystemExit('run without -O')
r=Path(__file__).resolve().parent;old=r.parent/'epoll-20261027'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def load(name,p):
 s=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
lock=(old/'.bench.lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
os.sched_setaffinity(0,{1})
for name in ['archives','harness','.bench.lock']:
 if not (r/name).exists():(r/name).symlink_to((old/name).resolve())
for name in ['bin','sources','targets']:(r/name).mkdir(exist_ok=True)
shutil.copyfile(old/'run.sh',r/'run.sh');os.chmod(r/'run.sh',0o755)
text=(old/'bench.py').read_text()
text=text.replace('overlay.worker_overlay(bins / "run_trial.py", worker)','followup = module("followup", ROOT / "followup-probes.py")\n        followup.worker_overlay(bins / "run_trial.py", worker, overlay)')
(r/'bench.py').write_text(text,newline='\n')
(r/'diagnostics.py').symlink_to((old/'diagnostics.py').resolve())
probes=load('probes',r/'followup-probes.py')
refs=['4532a76f22958ae3abcca801e90edd30ed9f466f','e10a20abb4f2b2fc072560787e20d918954a31ba']
for ref in refs:
 s=r/'sources'/ref;t=r/'targets'/ref;b=r/'bin'/ref
 if s.exists() or b.exists():raise RuntimeError('preserve existing build; inspect before recovery')
 shutil.copytree(old/'sources'/ref,s);probes.instrument(s);b.mkdir()
 env=dict(os.environ,CARGO_BUILD_JOBS='1',CARGO_TARGET_DIR=str(t),CARGO_PROFILE_RELEASE_DEBUG='1',CARGO_NET_OFFLINE='true',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1')
 command=['cargo','build','--release','--locked','--bin','slither-sim']
 if ref==refs[0]:command+=['--example','host_capacity_load']
 with (b/'build.log').open('x') as log:subprocess.run(command,cwd=s,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 shutil.copy2(t/'release/slither-sim',b/'slither-sim');shutil.copy2(s/'scripts/run_trial.py',b/'run_trial.py')
 if ref==refs[0]:shutil.copy2(t/'release/examples/host_capacity_load',r/'bin/host_capacity_load')
 pins={'commit':ref,'binarySha256':sha(b/'slither-sim'),'workerSha256':sha(b/'run_trial.py'),'probeSha256':sha(r/'followup-probes.py'),'planSha256':sha(r/'PLAN.md'),
 'sourceHashes':{str(p.relative_to(s)):sha(p) for p in [s/'src/main.rs',s/'src/client/server.rs',s/'src/client/simulation/remote.rs',s/'examples/host_capacity_load.rs']}}
 (b/'pins.json').write_text(json.dumps(pins,indent=2)+'\n')
 print('BUILT',ref,flush=True)
 if t.resolve().parent!=(r/'targets').resolve():raise RuntimeError('escaped target')
 shutil.rmtree(t)
compat=r/'bin/4c2471ba9afff3609faae964cdaad49e3209da68';compat.mkdir()
(compat/'pins.json').write_text(json.dumps({'loadBinarySha256':sha(r/'bin/host_capacity_load')})+'\n')
print('READY',flush=True)
