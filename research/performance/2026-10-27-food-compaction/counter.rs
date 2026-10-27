use std::{cell::RefCell,io,os::raw::{c_int,c_long,c_void}};
unsafe extern "C" {fn syscall(n:c_long,...)->c_long;fn read(fd:c_int,p:*mut c_void,n:usize)->isize;}
struct Counter {fd:c_int,tick:usize,last:[u64;3],values:Vec<[u64;4]>,sub:[u64;16],calls:[u64;16],rows:Vec<serde_json::Value>}
impl Counter {fn read(&self)->[u64;3]{let mut v=[0;3];assert_eq!(unsafe{read(self.fd,v.as_mut_ptr().cast(),24)},24);v}}
thread_local! {static C:RefCell<Counter>=RefCell::new({let mut a=[0u64;16];a[0]=128u64<<32;a[1]=1;a[4]=3;a[5]=(1<<5)|(1<<6);let fd=unsafe{syscall(298,a.as_ptr(),0i32,-1i32,-1i32,0u64)} as c_int;assert!(fd>=0,"{}",io::Error::last_os_error());Counter{fd,tick:0,last:[0;3],values:vec![],sub:[0;16],calls:[0;16],rows:vec![]}});}
pub fn begin(t:usize){C.with_borrow_mut(|c|{c.tick=t;c.values.clear();c.sub=[0;16];c.calls=[0;16];c.last=c.read();});}
pub fn mark(stage:u64){C.with_borrow_mut(|c|{let v=c.read();c.values.push([stage,v[0]-c.last[0],v[1]-c.last[1],v[2]-c.last[2]]);c.last=v;});}
pub fn end(alive:usize){C.with_borrow_mut(|c|{if c.tick>=120 && c.tick!=720 && alive==500{c.rows.push(serde_json::json!({"tick":c.tick,"phases":c.values,"sub":c.sub,"calls":c.calls}));}if c.tick==1079{std::fs::write(std::env::var("PICKUP_PROFILE").unwrap(),serde_json::to_vec(&c.rows).unwrap()).unwrap();}});}

pub fn sample_start()->[u64;3]{C.with_borrow(|c|c.read())}
pub fn sub(stage:usize,start:[u64;3]){C.with_borrow_mut(|c|{let end=c.read();assert!(end[2]-start[2]>=end[1]-start[1]);c.sub[stage]+=end[0]-start[0];c.calls[stage]+=1;});}

pub struct Scope {stage:usize,start:[u64;3]}
impl Drop for Scope {fn drop(&mut self){sub(self.stage,self.start);}}
pub fn scope(stage:usize)->Scope{Scope{stage,start:sample_start()}}
