//! Public HUD path allocation regression; no GPU, window or socket.
#![cfg(not(target_arch = "wasm32"))]
use slither_client::presentation::{BodyBlend, BodyRect};
use slither_wgpu_slice::{
    bank::Bank,
    frame::{DRAW_MEMORY_LIMIT, Draw, Frame, MAX_DRAWS},
    render,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    ptr,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

struct CountingAllocator;
static WATCHED: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());
static LARGEST: AtomicUsize = AtomicUsize::new(0);

// Observe only this test's Draw buffer. All allocations still use System; the
// public consuming finisher may drop the frame before returning its error.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let _ =
            WATCHED.compare_exchange(pointer, ptr::null_mut(), Ordering::SeqCst, Ordering::SeqCst);
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let watched = WATCHED.load(Ordering::SeqCst) == pointer;
        if watched {
            LARGEST.fetch_max(size, Ordering::SeqCst);
        }
        let next = unsafe { System.realloc(pointer, layout, size) };
        if watched && !next.is_null() {
            WATCHED.store(next, Ordering::SeqCst);
        }
        next
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn full_and_nearly_full_hud_reject_before_draw_capacity_exceeds_the_bound() {
    let mut bank = Bank::new().unwrap();
    let draw = Draw {
        corners: None,
        asset: bank.solid([255; 3]),
        source: None,
        repeat_uv: None,
        black_effect: false,
        destination: BodyRect {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        },
        translation: [0.; 2],
        rotation: 0.,
        effective_alpha: 1.,
        blend: BodyBlend::SourceOver,
    };
    for count in [MAX_DRAWS, MAX_DRAWS - 1] {
        let mut frame = Frame {
            identity: 1,
            extent: [960, 540],
            viewport_scale: 1.,
            tick: 0,
            draws: vec![draw.clone(); count],
            snakes: vec![],
        };
        assert_eq!(frame.draws.capacity(), count);
        LARGEST.store(count * size_of::<Draw>(), Ordering::SeqCst);
        WATCHED.store(frame.draws.as_mut_ptr().cast(), Ordering::SeqCst);
        let result = render::finish(frame, &mut bank, "HUD\noverflow");
        WATCHED.store(ptr::null_mut(), Ordering::SeqCst);
        assert_eq!(result.err().as_deref(), Some(DRAW_MEMORY_LIMIT));
        assert!(LARGEST.load(Ordering::SeqCst) <= MAX_DRAWS * size_of::<Draw>());
    }
}
