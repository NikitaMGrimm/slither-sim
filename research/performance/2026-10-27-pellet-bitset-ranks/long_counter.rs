//! Research-only per-tick user instructions and rare pellet-index rebuild cost.
use std::{cell::RefCell, io, os::raw::{c_int,c_long,c_void}};
unsafe extern "C" {fn syscall(n:c_long,...) -> c_long; fn read(fd:c_int,p:*mut c_void,n:usize)->isize; fn close(fd:c_int)->c_int;}
#[derive(serde::Serialize)]
struct Row {tick:usize, counts:[u64;3], alive:usize, births:usize, food:usize, retained:usize, radius:f64, maps:usize}
#[derive(serde::Serialize)]
struct Rebuild {tick:usize, old:usize, food:usize, counts:[u64;3],started:bool,completed:bool,progress:usize}
struct Probe {fd:c_int,path:String,tick:usize,rows:Vec<Row>,rebuilds:Vec<Rebuild>}
impl Probe {
 fn read(&self)->[u64;3] {let mut a=[0u64;3];assert_eq!(unsafe{read(self.fd,a.as_mut_ptr().cast(),24)},24);a}
 fn new()->Option<Self> {
  let path=std::env::var("LONG_COUNTS").ok()?;
  let mut a=[0u64;16];a[0]=128u64<<32;a[1]=1;a[4]=3;a[5]=(1<<5)|(1<<6);
  let fd=unsafe{syscall(298,a.as_ptr(),0i32,-1i32,-1i32,0u64)} as c_int;
  assert!(fd>=0,"perf_event_open: {}",io::Error::last_os_error());
  Some(Self{fd,path,tick:0,rows:Vec::with_capacity(60000),rebuilds:Vec::new()})
 }
}
thread_local! {static P:RefCell<Option<Probe>>=RefCell::new(Probe::new());}
pub fn read_now()->Option<[u64;3]> {P.with_borrow(|p|p.as_ref().map(Probe::read))}
pub fn begin(tick:usize)->Option<[u64;3]> {P.with_borrow_mut(|p|p.as_mut().map(|p|{p.tick=tick;p.read()}))}
pub fn end(start:Option<[u64;3]>)->[u64;3] {match (start,read_now()) {(Some(a),Some(b))=>[b[0]-a[0],b[1]-a[1],b[2]-a[2]],_=>[0;3]}}
pub fn rebuild(start:Option<[u64;3]>,old:usize,food:usize,started:bool,completed:bool,progress:usize) {
 let counts=end(start);
 P.with_borrow_mut(|p|if let Some(p)=p {p.rebuilds.push(Rebuild{tick:p.tick,old,food,counts,started,completed,progress});});
}
pub fn row(tick:usize,counts:[u64;3],alive:usize,births:usize,food:usize,retained:usize,radius:f64,maps:usize) {
 P.with_borrow_mut(|p|if let Some(p)=p {p.rows.push(Row{tick,counts,alive,births,food,retained,radius,maps});});
}
pub fn finish() {P.with_borrow_mut(|p|if let Some(p)=p.take() {
 std::fs::write(&p.path,serde_json::to_vec(&serde_json::json!({"rows":p.rows,"rebuilds":p.rebuilds})).unwrap()).unwrap();
 unsafe{close(p.fd)};
});}
