//! Thread-local counting excludes other test/harness threads. Inputs are built
//! before measurement; both prepasses and complete repeated iteration are inside.
mod common;
use slither_protocol::{
    DecodeAdmission::ClientPresentation,
    frame::*,
    v19::{self, Message},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
};
struct Counting;
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}
fn charge() {
    if ENABLED.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNT.try_with(|n| n.set(n.get() + 1));
    }
}
// This test forwards the allocator contract unchanged to System. Count alloc,
// alloc_zeroed and realloc, not dealloc; production contains no unsafe code.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        charge();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        charge();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        charge();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;
fn zero_allocations(f: impl FnOnce()) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ENABLED.set(false);
        }
    }
    COUNT.set(0);
    ENABLED.set(true);
    let reset = Reset;
    f();
    drop(reset);
    assert_eq!(COUNT.get(), 0);
}
#[test]
fn counter_detects_real_allocations() {
    COUNT.set(0);
    ENABLED.set(true);
    black_box(Vec::<u8>::with_capacity(123));
    ENABLED.set(false);
    assert!(COUNT.get() > 0);
}
#[test]
fn bare_and_bundled_frames_messages_and_iterators_allocate_zero() {
    let appearance = common::appearance();
    let hud = b"l\x01\0\x01\0\x02\0\x03\xff\xff\xff\x09\x02\0\xff";
    let food = b"F\x01\x02\xff\x03\x04\x05\x08\x09\x0a\x0b";
    let minimap = b"L\x02\0\x02\x40\x40";
    let mut bundle = vec![0, 0];
    for p in [&appearance[..], hud, food, minimap] {
        bundle.extend((p.len() as u16).to_be_bytes());
        bundle.extend(p);
    }
    for (bytes, header) in [
        (&appearance[..], Header::Absent),
        (&bundle[..], Header::TwoBytes),
    ] {
        zero_allocations(|| {
            for _ in 0..20 {
                let frame = decode_frame(
                    black_box(bytes),
                    header,
                    Limits::CLIENT,
                    FrameAdmission::Bounded,
                )
                .unwrap();
                for _ in 0..2 {
                    for c in frame.commands() {
                        match v19::decode_command(c, ClientPresentation).unwrap() {
                            Message::SnakeAppearance(a) => {
                                for d in a.body.deltas() {
                                    black_box(d);
                                }
                            }
                            Message::FoodSector(f) => {
                                for food in f.foods() {
                                    black_box(food);
                                }
                            }
                            Message::Leaderboard(l) => {
                                for row in l.rows() {
                                    black_box((row.name(), row.fractional_mass(), row.raw_color()));
                                }
                            }
                            Message::Minimap(m) => {
                                for cell in m.selected(2).unwrap() {
                                    black_box(cell);
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        });
    }
}
#[test]
fn invalid_frames_messages_hud_and_runs_allocate_zero() {
    zero_allocations(|| {
        for p in [&[][..], &[0], &[0, 0], &[0, 1, b'p', 0], &[0, 1, 0]] {
            black_box(
                decode_frame(p, Header::Absent, Limits::CLIENT, FrameAdmission::Bounded)
                    .unwrap_err(),
            );
        }
        for p in [&b"s\0"[..], b"F\0\0\0", b"l\0\0\0\0\0\0", b"M\0\x01\xff"] {
            black_box(v19::decode(p, ClientPresentation).unwrap_err());
        }
    });
}
