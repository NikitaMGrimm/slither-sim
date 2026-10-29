//! Shared measurement overlay. Local validation leaves counters/clocks disabled.
use std::cell::Cell;
thread_local! {
    static EVENTS: Cell<[u64;4]> = const { Cell::new([0;4]) };
    static ENABLED: Cell<bool> = const { Cell::new(false) };
}
fn event(i: usize) { EVENTS.with(|v| { let mut a=v.get(); a[i]+=1; v.set(a); }); }
pub fn search() { event(0); }
pub fn attempt() { event(1); }
pub fn exhaust() { event(2); }
pub fn marker() { event(3); }

#[cfg(target_os="linux")]
fn cpu() -> u64 {
    unsafe extern "C" { fn clock_gettime(clock:i32, value:*mut [i64;2])->i32; }
    let mut time=[0i64;2]; assert_eq!(unsafe { clock_gettime(2,&mut time) },0);
    time[0] as u64*1_000_000_000+time[1] as u64
}
pub fn enable() { ENABLED.with(|v|v.set(true)); }
pub fn begin() -> Option<([u64;3],u64)> {
    EVENTS.with(|v|v.set([0;4]));
    #[cfg(target_os="linux")]
    if ENABLED.with(Cell::get) { return Some((crate::study_counter::read_counter(),cpu())); }
    None
}
pub fn end(start:Option<([u64;3],u64)>) -> serde_json::Value {
    let events=EVENTS.with(Cell::get);
    #[cfg(target_os="linux")]
    if let Some((a,time))=start {
        let ns=cpu()-time; let b=crate::study_counter::read_counter();
        assert!(b[1]==a[1] || (b[2]-a[2]) as f64/(b[1]-a[1]) as f64>=0.99,"PMU multiplexing");
        return serde_json::json!({"events":events,"instructions":b[0]-a[0],"cpuNs":ns,
            "enabledNs":b[1]-a[1],"runningNs":b[2]-a[2]});
    }
    let _=start;
    serde_json::json!({"events":events})
}
