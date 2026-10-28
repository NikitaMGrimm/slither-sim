//! Research-only Linux x86-64 current-thread user PMU brackets.
use std::{cell::RefCell, io, os::raw::{c_int, c_long, c_void}};
unsafe extern "C" {
    fn syscall(n: c_long, ...) -> c_long;
    fn read(fd: c_int, p: *mut c_void, n: usize) -> isize;
}
struct Counter { fd: c_int, active: bool, sums: [u64; 24], calls: [u64; 24], last: [u64; 3] }
thread_local! { static C: RefCell<Counter> = RefCell::new({
    let mut attr = [0u64; 16];
    attr[0] = 128u64 << 32; attr[1] = 1; attr[4] = 3;
    attr[5] = (1 << 5) | (1 << 6);
    let fd = unsafe { syscall(298, attr.as_ptr(), 0i32, -1i32, -1i32, 0u64) } as c_int;
    assert!(fd >= 0, "perf_event_open: {}", io::Error::last_os_error());
    Counter { fd, active: false, sums: [0;24], calls: [0;24], last: [0;3] }
}); }
pub fn read_counter() -> [u64; 3] {
    C.with_borrow(|c| { let mut v = [0;3];
        assert_eq!(unsafe { read(c.fd, v.as_mut_ptr().cast(), 24) }, 24); v })
}
pub fn start() -> Option<[u64;3]> { C.with_borrow(|c| c.active).then(read_counter) }
pub fn add(stage: usize, start: Option<[u64;3]>) {
    if let Some(a) = start { let b = read_counter();
        assert!(b[1] == a[1] || (b[2]-a[2]) as f64 / (b[1]-a[1]) as f64 >= 0.99);
        C.with_borrow_mut(|c| { c.sums[stage] += b[0]-a[0]; c.calls[stage] += 1; });
    }
}
pub fn reset(active: bool) { C.with_borrow_mut(|c| { c.active=active; c.sums=[0;24]; c.calls=[0;24]; }); }
pub fn record(stage: usize, value: u64) { C.with_borrow_mut(|c| { if c.active {c.sums[stage]+=value; c.calls[stage]+=1;} }); }
pub fn mark(stage: usize) {
    if C.with_borrow(|c| c.active) {
        let b = read_counter();
        C.with_borrow_mut(|c| { c.sums[stage] += b[0]-c.last[0]; c.calls[stage]+=1; c.last=b; });
    }
}
pub fn begin() { if C.with_borrow(|c| c.active) { let b=read_counter(); C.with_borrow_mut(|c| c.last=b); } }
pub fn result() -> serde_json::Value { C.with_borrow(|c| serde_json::json!({"instructions": c.sums.to_vec(), "calls": c.calls.to_vec()})) }
