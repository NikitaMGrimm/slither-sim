"""Measurement-only markers and own-child syscall counter; no socket change."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,tarfile
r=Path(__file__).resolve().parent
source=r/'source';source.mkdir()
with tarfile.open(r/'base.tar') as f:f.extractall(source,filter='data')
f=source/'src/lib.rs';f.write_text(f.read_text()+'\nmod socket_counts;\n')
(source/'src/socket_counts.rs').write_text('''
unsafe extern "C" { fn write(fd:i32,bytes:*const u8,len:usize)->isize; }
pub fn event(id:usize){unsafe{write(-700,id as *const u8,0);}}
pub fn tick(tick:u64){unsafe{write(-701,tick as *const u8,0);}}
''')
f=source/'src/client/server.rs';text=f.read_text()
for a,b in [
 ('        self.inner.write(bytes)','        crate::socket_counts::event(1);\n        let result=self.inner.write(bytes);\n        crate::socket_counts::event(3);\n        result'),
 ('        match self.socket.write(Message::Binary(bytes)) {','        crate::socket_counts::event(0);\n        match self.socket.write(Message::Binary(bytes)) {'),
 ('        match self.socket.flush() {','        crate::socket_counts::event(2);\n        match self.socket.flush() {'),
]:
    assert text.count(a)==1,a
    text=text.replace(a,b)
f.write_text(text)
f=source/'src/client/simulation/broadcast.rs';text=f.read_text()
needle='        let end = (self.tick + 1) as f64 * self.world.config().dt;'
assert text.count(needle)==1
text=text.replace(needle,'        crate::socket_counts::tick(self.tick+1);\n'+needle)
f.write_text(text)
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/slither-maintenance-round2-target',CARGO_BUILD_JOBS='1',CARGO_PROFILE_RELEASE_DEBUG='0',CARGO_INCREMENTAL='0',RAYON_NUM_THREADS='1',RUSTC=compiler,RUSTDOC=str(Path(compiler).with_name('rustdoc')),RUSTFLAGS='-C target-cpu=native',CARGO_NET_OFFLINE='true')
env.pop('CARGO_ENCODED_RUSTFLAGS',None)
os.sched_setaffinity(0,{0})
command=['cargo','build','--release','--locked','--offline','--bin','slither-sim']
with (r/'build.log').open('x') as log:subprocess.run(command,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
shutil.copy2(Path(env['CARGO_TARGET_DIR'])/'release/slither-sim',r/'host')
subprocess.run(['gcc','-O2','-Wall','-Wextra','-Werror',str(r/'syscall_counts.c'),'-o',str(r/'counter')],check=True)
sha=lambda p:hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
(r/'pins.json').write_text(json.dumps({'runtime':'23e987f37b061ac80307404c24f6b7fecc71a770','archiveSha256':sha(r/'base.tar'),'hostSha256':sha(r/'host'),'counterSha256':sha(r/'counter'),'sources':{str(f.relative_to(source)):sha(f) for f in source.rglob('*.rs')},'command':command,'toolchain':subprocess.check_output([compiler,'--version'],text=True).strip()},indent=2)+'\n')
print('SOCKET DIAGNOSTIC BUILT',flush=True)
