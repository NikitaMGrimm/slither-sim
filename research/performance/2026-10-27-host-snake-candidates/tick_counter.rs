//! Research-only Linux x86-64 current-thread user instructions, outside runtime.
use std::{fs,io,os::raw::{c_int,c_long,c_void}};
unsafe extern "C" {
    fn syscall(number:c_long,...) -> c_long;
    fn read(fd:c_int,buffer:*mut c_void,size:usize) -> isize;
    fn close(fd:c_int) -> c_int;
}
pub(super) struct Counter {fd:c_int,rows:Vec<[u64;5]>,path:String}
impl Counter {
    pub(super) fn from_env()->Option<Self> {
        let path=std::env::var("STUDY_TICK_COUNTS").ok()?;
        assert!(cfg!(all(target_os="linux",target_arch="x86_64")));
        let mut attr=[0u64;16];attr[0]=128u64<<32;attr[1]=1;attr[4]=3;attr[5]=(1<<5)|(1<<6);
        let fd=unsafe{syscall(298,attr.as_ptr(),0i32,-1i32,-1i32,0u64)} as c_int;
        assert!(fd>=0,"perf_event_open: {}",io::Error::last_os_error());
        Some(Self{fd,rows:Vec::with_capacity(1080),path})
    }
    pub(super) fn read(&self)->[u64;3] {
        let mut v=[0u64;3];assert_eq!(unsafe{read(self.fd,v.as_mut_ptr().cast(),24)},24);v
    }
    pub(super) fn record(&mut self,tick:usize,start:[u64;3],end:[u64;3],playing:usize) {
        self.rows.push([tick as u64,end[0]-start[0],end[1]-start[1],end[2]-start[2],playing as u64]);
    }
}
impl Drop for Counter {
    fn drop(&mut self) {
        fs::write(&self.path,serde_json::to_vec(&self.rows).unwrap()).unwrap();
        unsafe{close(self.fd)};
    }
}
