//! A reset whose reply cannot be allocated keeps the previous session. Its own
//! test binary, since it replaces the global allocator.
use slither_sim::transport::Session;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fails the first allocation of exactly `FAIL_SIZE` bytes, then none.
struct FailOnce;
static FAIL_SIZE: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for FailOnce {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        if size != 0
            && FAIL_SIZE
                .compare_exchange(size, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            FAILED.store(size, Ordering::SeqCst);
            return std::ptr::null_mut();
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: FailOnce = FailOnce;

#[test]
fn a_reset_whose_reply_cannot_be_allocated_keeps_the_previous_session() {
    let request = br#"{"op":"reset","seed":7,"agents":1,"config":{"radius":1000,"dynamicRadius":false,"foodCount":0,"ambientReplenishPerSecond":0,"prey":{"count":0}}}"#;
    let mut session = Session::new();
    let reply = session.request(request);
    assert_eq!(session.worlds(), 1);
    let before = session.request(br#"{"op":"snapshot"}"#);
    // `worlds_reply` reserves its members plus 64 bytes and one per frame, so
    // an identical reset reserves 8 bytes more than the bytes outside `head`
    // and the frames of this reply.
    let wrapper = br#"{"protocol":"slither-sim-json-v2","result":{"worlds":[]}}"#.len();
    FAIL_SIZE.store(reply.len() - wrapper + 65, Ordering::SeqCst);
    let failed = session.request(request);
    FAIL_SIZE.store(0, Ordering::SeqCst);
    assert_ne!(
        FAILED.load(Ordering::SeqCst),
        0,
        "the reply allocation was not reached"
    );
    let failed = String::from_utf8(failed).unwrap();
    assert!(
        failed.contains("The reply does not fit in memory"),
        "{failed}"
    );
    assert_eq!(session.worlds(), 1);
    assert_eq!(session.request(br#"{"op":"snapshot"}"#), before);
    assert!(session.frame(0).is_some());
}
