//! Diagnostic-only Linux x86-64 user instructions; no production state.
use std::{cell::RefCell,fs,io,os::raw::{c_int,c_long,c_void}};
unsafe extern "C" {fn syscall(n:c_long,...)->c_long;fn read(fd:c_int,p:*mut c_void,n:usize)->isize;fn close(fd:c_int)->c_int;}
const N:usize=16;
const NAMES:[&str;N]=["world_step","shared_food","reports","bounds","grid","prey_bounds","candidates","snake_updates","recipient_food","recipient_prey","framing","legacy","board","map","hud","broadcast"];
#[derive(serde::Serialize)]
struct Row{tick:usize,instructions:u64,enabled:u64,running:u64,playing:usize,phases:[u64;N],calls:[u64;N]}
struct Recorder{fd:c_int,active:bool,phases:[u64;N],calls:[u64;N],rows:Vec<Row>}
impl Recorder{
 fn new()->Self{assert!(cfg!(all(target_os="linux",target_arch="x86_64")));let mut attr=[0u64;16];attr[0]=128u64<<32;attr[1]=1;attr[4]=3;attr[5]=(1<<5)|(1<<6);let fd=unsafe{syscall(298,attr.as_ptr(),0i32,-1i32,-1i32,0u64)} as c_int;assert!(fd>=0,"{}",io::Error::last_os_error());Self{fd,active:false,phases:[0;N],calls:[0;N],rows:Vec::with_capacity(1080)}}
 fn stamp(&self)->[u64;3]{let mut v=[0u64;3];assert_eq!(unsafe{read(self.fd,v.as_mut_ptr().cast(),24)},24);v}
}
impl Drop for Recorder{fn drop(&mut self){if !self.rows.is_empty(){let value=serde_json::json!({"phases":NAMES,"rows":self.rows});fs::write(std::env::var("STUDY_PHASES").unwrap(),serde_json::to_vec(&value).unwrap()).unwrap();}unsafe{close(self.fd)};}}
thread_local!{static DATA:RefCell<Recorder>=RefCell::new(Recorder::new());}
pub(crate) struct Tick([u64;3]);
impl Tick{
 pub(crate) fn start()->Self{DATA.with(|d|{let mut d=d.borrow_mut();d.active=true;d.phases=[0;N];d.calls=[0;N];Self(d.stamp())})}
 pub(crate) fn finish(self,tick:usize,playing:usize){DATA.with(|d|{let mut d=d.borrow_mut();let end=d.stamp();d.active=false;let row=Row{tick,playing,instructions:end[0]-self.0[0],enabled:end[1]-self.0[1],running:end[2]-self.0[2],phases:d.phases,calls:d.calls};d.rows.push(row);});}
}
pub(crate) struct Scope{phase:usize,start:Option<u64>}
impl Scope{pub(crate) fn new(phase:usize)->Self{Self{phase,start:DATA.with(|d|{let d=d.borrow();d.active.then(||d.stamp()[0])})}}}
impl Drop for Scope{fn drop(&mut self){if let Some(start)=self.start{DATA.with(|d|{let mut d=d.borrow_mut();if d.active{let end=d.stamp()[0];d.phases[self.phase]+=end-start;d.calls[self.phase]+=1;}});}}}
