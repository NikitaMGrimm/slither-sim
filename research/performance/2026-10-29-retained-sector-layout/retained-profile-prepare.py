"""Adapt the retained #269 diagnostic builder; never alter production sources."""
from pathlib import Path
import os,sys,subprocess,json,tarfile,hashlib,shutil
r=Path(__file__).resolve().parent
subprocess.run([sys.executable,str(r/'donor-build.py'),'--prepare-only'],check=True)
s=r/'source'
def swap(p,before,after):
 text=p.read_text();assert text.count(before)==1,(p.name,before)
 p.write_text(text.replace(before,after))
swap(s/'src/client/simulation/snakes.rs',
 '[self.by_slot.len(),self.by_slot.capacity(),self.slots.len(),self.slots.capacity()]',
 '[self.entries.len(),self.entries.capacity(),self.entries.len(),self.entries.capacity()]')
swap(s/'src/client/simulation/study_host.rs',
 '"knownSnake":size_of::<super::snakes::KnownSnake>()',
 '"knownSlotEntry":size_of::<(usize,super::snakes::KnownSnake)>(),"knownSnake":size_of::<super::snakes::KnownSnake>()')
swap(s/'src/study_events.c','a.sample_period=event==1?1000003:1009;',
 'a.sample_period=event==1?1000003:event==3?10007:1009;')
subprocess.run(['cc','-O2','-c',str(s/'src/study_events.c'),'-o',str(r/'events.o')],check=True)
subprocess.run(['ar','rcs',str(r/'libmemdiag.a'),str(r/'events.o')],check=True)
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/slither-retained-layout-target',CARGO_BUILD_JOBS='2',CARGO_PROFILE_RELEASE_DEBUG='1',CARGO_INCREMENTAL='0',RUSTC=compiler,RUSTDOC=str(Path(compiler).with_name('rustdoc')),RUSTFLAGS=f'-L native={r} -l static=memdiag',CARGO_NET_OFFLINE='true')
env.pop('CARGO_ENCODED_RUSTFLAGS',None);assert not env.get('LD_PRELOAD')
cmd=['cargo','build','--release','--locked','--offline','-j2','--example','study_host']
with (r/'build.log').open('x') as out:subprocess.run(cmd,cwd=s,env=env,stdout=out,stderr=subprocess.STDOUT,check=True)
shutil.copy2(Path(env['CARGO_TARGET_DIR'])/'release/examples/study_host',r/'study_host')
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
with tarfile.open(r/'main.tar') as t:runtime=t.pax_headers['comment']
digest=hashlib.sha256()
for p in sorted(s.rglob('*.rs'),key=lambda p:p.relative_to(s).as_posix()):digest.update(p.relative_to(s).as_posix().encode()+b'\0'+p.read_bytes()+b'\0')
(r/'build.json').write_text(json.dumps({'runtime':runtime,'archiveSha256':sha(r/'main.tar'),'binarySha256':sha(r/'study_host'),'generatedRustAggregateSha256':digest.hexdigest(),'collectorSha256':sha(s/'src/study_events.c'),'command':cmd,'compiler':subprocess.check_output([compiler,'--version'],text=True).strip(),'portableCodegen':True,'debugSymbols':True},indent=2)+'\n')
subprocess.run(['cc','-O2','-DPROBE',str(s/'src/study_events.c'),'-o',str(r/'event-probe')],check=True)
with (r/'events.json').open('x') as out:subprocess.run([str(r/'event-probe')],stdout=out,check=True)
print('Fresh attribution binary and event controls ready; no attribution collected.',flush=True)
