//! Replies that cannot be allocated: a reset keeps the previous session and a
//! step ends it. Its own test binary, since it replaces the global allocator.
use slither_sim::transport::Session;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
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

/// Tests share the allocator's target size, so they run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

const RESET: &[u8] = br#"{"op":"reset","seed":7,"agents":1,"config":{"radius":1000,"dynamicRadius":false,"foodCount":0,"ambientReplenishPerSecond":0,"prey":{"count":0}}}"#;

/// `request` on `session`, failing its reply allocation when it is as large as
/// `twin`'s. `worlds_reply` reserves its members plus 64 bytes and one per
/// frame: for one world, 8 bytes more than the reply minus its fixed wrapper.
fn failing_reply(session: &mut Session, request: &[u8], twin: &[u8]) -> String {
    let wrapper = br#"{"protocol":"slither-sim-json-v6","result":{"worlds":[]}}"#.len();
    FAILED.store(0, Ordering::SeqCst);
    FAIL_SIZE.store(twin.len() - wrapper + 65, Ordering::SeqCst);
    let reply = session.request(request);
    FAIL_SIZE.store(0, Ordering::SeqCst);
    assert_ne!(
        FAILED.load(Ordering::SeqCst),
        0,
        "the reply allocation was not reached"
    );
    String::from_utf8(reply).unwrap()
}

#[test]
fn a_reset_whose_reply_cannot_be_allocated_keeps_the_previous_session() {
    let _serial = SERIAL.lock().unwrap();
    let mut session = Session::new();
    let reply = session.request(RESET);
    assert_eq!(session.worlds(), 1);
    let before = session.request(br#"{"op":"snapshot"}"#);
    let failed = failing_reply(&mut session, RESET, &reply);
    assert!(
        failed.contains("The reply does not fit in memory"),
        "{failed}"
    );
    assert!(failed.contains("\"sessionEnded\":false"), "{failed}");
    assert_eq!(session.worlds(), 1);
    assert_eq!(session.request(br#"{"op":"snapshot"}"#), before);
    assert!(session.frame(0).is_some());
}

#[test]
fn a_step_whose_reply_cannot_be_allocated_ends_the_session() {
    let _serial = SERIAL.lock().unwrap();
    let step = br#"{"op":"step","actions":[{"heading":[0.5],"boost":[false]}]}"#;
    // An identical session predicts the reply size of the failing step.
    let mut twin = Session::new();
    twin.request(RESET);
    let reply = twin.request(step);
    let mut session = Session::new();
    session.request(RESET);
    let failed = failing_reply(&mut session, step, &reply);
    // The worlds advanced, so a retry must not step them again.
    assert!(
        failed.contains("The reply does not fit in memory"),
        "{failed}"
    );
    assert!(failed.contains("the session has ended"), "{failed}");
    assert!(failed.contains("\"sessionEnded\":true"), "{failed}");
    assert_eq!(session.worlds(), 0);
    let retry = String::from_utf8(session.request(step)).unwrap();
    assert!(retry.contains("Reset first"), "{retry}");
    assert!(retry.contains("\"sessionEnded\":false"), "{retry}");
    session.request(RESET);
    assert_eq!(session.worlds(), 1);
    assert!(
        String::from_utf8(session.request(step))
            .unwrap()
            .contains("\"result\":")
    );
}
