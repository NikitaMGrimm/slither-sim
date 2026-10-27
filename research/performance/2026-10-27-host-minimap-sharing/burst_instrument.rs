//! Study-only Linux x86-64 instrumentation, absent from production binaries.
use std::{cell::RefCell, fs, io, os::raw::{c_int,c_long,c_void}, time::{Instant,SystemTime,UNIX_EPOCH}};
unsafe extern "C" {
    fn syscall(number:c_long, ...) -> c_long;
    fn read(fd:c_int, buffer:*mut c_void, size:usize) -> isize;
    fn close(fd:c_int) -> c_int;
    fn clock_gettime(clock:c_int, value:*mut [i64;2]) -> c_int;
}
fn cpu() -> u64 {
    let mut value=[0;2];
    assert_eq!(unsafe { clock_gettime(3,&mut value) },0);
    value[0] as u64*1_000_000_000+value[1] as u64
}
struct Recorder {
    fd:c_int, due:bool, maps:[u64;4], rows:Vec<serde_json::Value>,
}
impl Recorder {
    fn new() -> Self {
        assert!(cfg!(all(target_os="linux",target_arch="x86_64")));
        let mut attr=[0u64;16];
        attr[0]=128u64<<32; // PERF_TYPE_HARDWARE, attr size.
        attr[1]=1; // PERF_COUNT_HW_INSTRUCTIONS.
        attr[4]=3; // TOTAL_TIME_ENABLED | TOTAL_TIME_RUNNING.
        attr[5]=(1<<5)|(1<<6); // Exclude kernel and hypervisor.
        let fd=unsafe { syscall(298,attr.as_ptr(),0i32,-1i32,-1i32,0u64) } as c_int;
        assert!(fd>=0,"perf_event_open: {}",io::Error::last_os_error());
        Self {fd,due:false,maps:[0;4],rows:Vec::new()}
    }
    fn stamp(&self) -> [u64;4] {
        let mut data=[0u64;4];
        assert_eq!(unsafe {read(self.fd,data.as_mut_ptr().cast(),24)},24);
        data[3]=cpu();
        data
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        fs::write(std::env::var("STUDY_TICKS").unwrap(),serde_json::to_vec(&self.rows).unwrap()).unwrap();
        unsafe {close(self.fd)};
    }
}
thread_local! { static DATA:RefCell<Recorder> = RefCell::new(Recorder::new()); }
pub(crate) struct Tick { start:[u64;4], wall:Instant, unix:f64 }
impl Tick {
    pub(crate) fn start() -> Self {
        DATA.with(|data| {
            let mut data=data.borrow_mut();
            data.due=false; data.maps=[0;4];
            Self {start:data.stamp(),wall:Instant::now(),unix:SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64()}
        })
    }
    pub(crate) fn finish(self,tick:u64,waiting:usize) {
        DATA.with(|data| {
            let mut data=data.borrow_mut();
            let end=data.stamp(); let maps=data.maps;
            let row=serde_json::json!({"tick":tick,"unix":self.unix,"due":data.due,
                "instructions":end[0]-self.start[0],"enabled":end[1]-self.start[1],"running":end[2]-self.start[2],
                "cpuNs":end[3]-self.start[3],"wallNs":self.wall.elapsed().as_nanos() as u64,
                "mapInstructions":maps[0],"mapCpuNs":maps[1],"mapCalls":maps[2],"mapWallNs":maps[3],"waiting":waiting});
            data.rows.push(row);
        });
    }
}
pub(crate) fn due(due:bool) { DATA.with(|data| data.borrow_mut().due=due); }
pub(crate) struct Map {start:[u64;4],wall:Instant}
impl Map {
    pub(crate) fn start() -> Self { Self { start:DATA.with(|data| data.borrow().stamp()),wall:Instant::now() } }
}
impl Drop for Map {
    fn drop(&mut self) {
        DATA.with(|data| {
            let mut data=data.borrow_mut(); let end=data.stamp();
            data.maps[0]+=end[0]-self.start[0]; data.maps[1]+=end[3]-self.start[3];
            data.maps[2]+=1; data.maps[3]+=self.wall.elapsed().as_nanos() as u64;
        });
    }
}
