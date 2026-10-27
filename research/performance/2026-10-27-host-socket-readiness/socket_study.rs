//! Diagnostic counters only, in an archived host; never production state.
use std::{cell::RefCell,fs,io,sync::atomic::{AtomicBool,Ordering}};
static ACTIVE:AtomicBool=AtomicBool::new(false);
pub fn active(value:bool){ACTIVE.store(value,Ordering::Relaxed);}
const NAMES:[&str;15]=["read_attempts","read_success","read_would_block","read_bytes","write_attempts","write_success","write_would_block","write_bytes","stream_flush_calls","frames_offered","game_bytes_offered","gateway_polls","deliveries","sessions_tested","readiness_polls"];
struct Counts([u64;15]);
impl Drop for Counts {
 fn drop(&mut self){if self.0[11]>0 {let value=serde_json::json!({"names":NAMES,"values":self.0});fs::write(std::env::var("STUDY_SOCKET_COUNTS").unwrap(),serde_json::to_vec(&value).unwrap()).unwrap();}}
}
thread_local!{static DATA:RefCell<Counts>=const{RefCell::new(Counts([0;15]))};}
pub(crate) fn event(index:usize,value:u64){if ACTIVE.load(Ordering::Relaxed){DATA.with(|d|d.borrow_mut().0[index]+=value);}}
pub(crate) fn read(result:&io::Result<usize>){
 if ACTIVE.load(Ordering::Relaxed){DATA.with(|d|{let mut d=d.borrow_mut();d.0[0]+=1;match result {Ok(n)=>{d.0[1]+=1;d.0[3]+=*n as u64},Err(e) if e.kind()==io::ErrorKind::WouldBlock=>d.0[2]+=1,Err(_)=>{}}});}
}
pub(crate) fn write(result:&io::Result<usize>){
 if ACTIVE.load(Ordering::Relaxed){DATA.with(|d|{let mut d=d.borrow_mut();d.0[4]+=1;match result {Ok(n)=>{d.0[5]+=1;d.0[7]+=*n as u64},Err(e) if e.kind()==io::ErrorKind::WouldBlock=>d.0[6]+=1,Err(_)=>{}}});}
}
