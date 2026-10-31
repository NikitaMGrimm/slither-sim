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

#[test]
fn v13_frames_views_iterators_and_copy_loop_allocate_zero() {
    use slither_protocol::{DecodeAdmission::*, challenge, v13};
    let appearance = common::appearance();
    let mut setup = common::setup();
    setup[23] = 13;
    let packets = [
        &setup[..],
        &appearance[..],
        &b"g\0\x01\0\0\xff\xff"[..],
        b"N\0\x01\0\xff\xff\xff\xff",
        b"e\0\x01\0\x01\x02",
        b"F\xff\0\0\xff\xff\x20\0\x12\x34\xab\xcd\x01",
        b"c\0\0\xff\xff\0\0",
        b"l\x01\0\x01\0\x02\0\x03\xff\xff\xff\x09\x02\0\xff",
        b"L\x02\0\x02\x40\x40",
        b"j\0\x01\0\x01\0\x01\x32\0\0\x01\0\0\x02\0\x03",
        b"s\0\x01\0",
        b"y\0\x01\0\x02",
        b"p",
        b"v\0",
    ];
    let mut bundle = vec![0, 0];
    for p in packets {
        bundle.extend((p.len() as u16).to_be_bytes());
        bundle.extend(p);
    }
    let encoded = common::encode_source(
        b"var a='ABCDEFGHIJKLMNOPQRSTUVWX';for(var i=0;i<24;i++)idba[i]=a.charCodeAt(i);",
    );
    for admission in [Sim, ClientReceived, ClientPresentation] {
        for (bytes, header) in packets
            .into_iter()
            .map(|p| (p, Header::Absent))
            .chain([(&bundle[..], Header::TwoBytes)])
        {
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
                            match v13::decode_command(c, admission).unwrap() {
                                v13::Message::SnakeAppearance(a) => {
                                    for d in a.body.deltas() {
                                        black_box(d);
                                    }
                                }
                                v13::Message::FoodBatch(f) => {
                                    for food in f.foods() {
                                        black_box(food);
                                    }
                                }
                                v13::Message::Leaderboard(l) => {
                                    for row in l.rows() {
                                        black_box((
                                            row.name(),
                                            row.raw_fraction(),
                                            row.raw_color(),
                                        ));
                                    }
                                }
                                v13::Message::Minimap(m) => {
                                    for cell in m.selected(2).unwrap() {
                                        black_box(cell);
                                    }
                                }
                                m => {
                                    black_box(m);
                                }
                            }
                        }
                    }
                    black_box(challenge::copy_loop(black_box(&encoded)).unwrap());
                }
            });
        }
    }
}

#[test]
fn v13_errors_and_general_challenge_errors_allocate_zero() {
    use slither_protocol::{DecodeAdmission::*, challenge, v13};
    let bad_literal =
        common::encode_source(b"var a='A\\';for(var i=0;i<1;i++)idba[i]=a.charCodeAt(i);");
    let bad_count =
        common::encode_source(b"var a='A';for(var i=0;i<2;i++)idba[i]=a.charCodeAt(i);");
    zero_allocations(|| {
        for admission in [Sim, ClientReceived, ClientPresentation] {
            for p in [
                &b"G"[..],
                b"n\0\0",
                b"F\0",
                b"f\0",
                b"c\0",
                b"5\0\0\0",
                b"s\0",
                b"M\0\x01\xff",
                b"l\0\0\0\0\0\0",
            ] {
                black_box(v13::decode(black_box(p), admission).unwrap_err());
                black_box(
                    v13::decode_command(
                        CommandRef {
                            index: 1,
                            offset: 42,
                            bytes: p,
                        },
                        admission,
                    )
                    .unwrap_err(),
                );
            }
        }
        for p in [&b"!a"[..], b"zz", b"a", &bad_literal, &bad_count] {
            black_box(challenge::copy_loop(black_box(p)).unwrap_err());
        }
    });
}
