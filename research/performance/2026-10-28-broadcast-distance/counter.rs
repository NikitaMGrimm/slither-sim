//! Research-only current-thread instructions, Linux x86-64; no system changes.
use std::{cell::RefCell, os::raw::{c_int,c_long,c_void}};
unsafe extern "C" {
    fn syscall(n:c_long,...) -> c_long;
    fn read(fd:c_int,p:*mut c_void,n:usize)->isize;
    fn close(fd:c_int)->c_int;
    fn dlsym(handle:*mut c_void,name:*const u8)->*mut c_void;
}
struct Counter {fd:c_int,sums:[u64;16],calls:[u64;16],scope:Option<unsafe extern "C" fn(u32)->u32>}
thread_local! {static C:RefCell<Counter>=RefCell::new({
    let mut a=[0u64;16]; a[0]=128u64<<32; a[1]=1; a[4]=3; a[5]=(1<<5)|(1<<6);
    let fd=unsafe{syscall(298,a.as_ptr(),0i32,-1i32,-1i32,0u64)} as c_int;
    assert!(fd>=0,"perf_event_open: {}",std::io::Error::last_os_error());
    let p=unsafe{dlsym(std::ptr::null_mut(),c"study_scope".as_ptr().cast())};
    let scope=if p.is_null(){None}else{Some(unsafe{std::mem::transmute::<*mut c_void,unsafe extern "C" fn(u32)->u32>(p)})};
    Counter{fd,sums:[0;16],calls:[0;16],scope}
});}
pub fn read_counter()->[u64;3] { C.with_borrow(|c|{let mut v=[0;3];assert_eq!(unsafe{read(c.fd,v.as_mut_ptr().cast(),24)},24);v}) }
pub struct Guard {id:usize,start:[u64;3],old:u32}
pub fn enter(id:usize)->Guard {
    let old=C.with_borrow(|c|c.scope.map_or(0,|f|unsafe{f(id as u32)}));
    Guard{id,start:read_counter(),old}
}
impl Drop for Guard {fn drop(&mut self){
    let b=read_counter();let a=self.start;
    assert!(b[1]==a[1] || (b[2]-a[2]) as f64/(b[1]-a[1]) as f64>=0.99,"PMU multiplexing");
    C.with_borrow_mut(|c|{c.sums[self.id]+=b[0]-a[0];c.calls[self.id]+=1;if let Some(f)=c.scope{unsafe{f(self.old)};}});
}}
pub fn result()->serde_json::Value {C.with_borrow(|c|serde_json::json!({"instructions":c.sums.to_vec(),"calls":c.calls.to_vec()}))}
pub fn finish(){
    if let Ok(path)=std::env::var("STUDY_COUNTS"){std::fs::write(path,serde_json::to_vec(&result()).unwrap()).unwrap();}
    C.with_borrow(|c|{unsafe{close(c.fd)};});
}
