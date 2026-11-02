//! Authored synthetic inputs. Reader allocation guarantees exclude owned state,
//! presentation events, and the explicitly copied browser ABI.
use slither_client::{
    presentation::AnimatedClient,
    protocol::{self, ElapsedHeader, hud, minimap},
    state::ClientState,
};
use slither_protocol::{DecodeAdmission, v19};
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
// Forward the allocator contract to System; instrumentation is test-only.
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

fn allocations(f: impl FnOnce()) -> usize {
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
    COUNT.get()
}

fn setup() -> Vec<u8> {
    let mut p = vec![b'a', 0, 3, 232];
    for n in [430_u16, 100, 20] {
        p.extend(n.to_be_bytes());
    }
    p.push(48);
    for n in [539_u16, 40, 1400, 33, 28, 430] {
        p.extend(n.to_be_bytes());
    }
    p.extend([19, 10]);
    p
}
fn appearance() -> Vec<u8> {
    let mut p = vec![b's', 0, 7, 0, 0, 0, b'1', 0, 0, 0, 1, 64, 0, 0, 0, 5];
    p.extend([0, 19, 136, 0, 19, 136, 0, 0, 0]);
    p.extend([0, 19, 136, 0, 19, 136, 129, 127, 0, 0]);
    p
}
fn bundle(commands: &[&[u8]]) -> Vec<u8> {
    commands
        .iter()
        .flat_map(|p| {
            (p.len() as u16)
                .to_be_bytes()
                .into_iter()
                .chain(p.iter().copied())
        })
        .collect()
}

#[test]
fn bare_bundled_and_repeated_reader_passes_allocate_zero() {
    assert!(
        allocations(|| {
            black_box(Vec::<u8>::with_capacity(123));
        }) > 0
    );
    let appearance = appearance();
    let setup = setup();
    let board = b"l\x01\0\x01\0\x02\0\x03\xff\xff\xff\x09\x02\0\xff";
    let food = b"F\x01\x02\xff\x03\x04\x05\x08\x09\x0a\x0b";
    let map = b"L\x02\0\x02\x40\x40";
    let bundle = bundle(&[&setup, &appearance, board, food, map]);
    let mut timed = vec![0x12, 0x34];
    timed.extend(&bundle);
    for (bytes, header) in [
        (appearance.as_slice(), ElapsedHeader::Absent),
        (bundle.as_slice(), ElapsedHeader::Absent),
        (timed.as_slice(), ElapsedHeader::TwoBytes),
    ] {
        assert_eq!(
            allocations(|| {
                for _ in 0..10 {
                    let frame = protocol::read_frame(black_box(bytes), header).unwrap();
                    let slices = protocol::decode_frame(black_box(bytes), header).unwrap();
                    black_box(slices.commands.iter().count());
                    for _ in 0..2 {
                        for command in frame.commands() {
                            black_box((command.index, command.offset));
                            match v19::decode_command(command, DecodeAdmission::ClientPresentation)
                                .unwrap()
                            {
                                v19::Message::SnakeAppearance(raw) => {
                                    for delta in raw.body.deltas() {
                                        black_box(delta);
                                    }
                                }
                                v19::Message::FoodSector(raw) => {
                                    for food in raw.foods() {
                                        black_box(food);
                                    }
                                }
                                _ => {}
                            }
                            if command.bytes[0] == b'l' {
                                for row in hud::parse_leaderboard(command.bytes).unwrap().rows() {
                                    black_box((row.count(), row.fractional_mass(), row.name()));
                                }
                            }
                            if command.bytes[0] == b'L' {
                                black_box(minimap::parse_minimap(command.bytes).unwrap());
                            }
                        }
                    }
                }
            }),
            0
        );
    }
}

#[test]
fn invalid_envelopes_and_selected_message_tails_allocate_zero() {
    let mut invalid = appearance();
    invalid.push(1);
    for bytes in [&b""[..], &b"\0"[..], &b"\0\x01p\0"[..], &b"a"[..]] {
        assert_eq!(
            allocations(|| {
                assert!(protocol::read_frame(black_box(bytes), ElapsedHeader::Absent).is_err());
            }),
            0
        );
    }
    assert_eq!(
        allocations(|| {
            let frame = protocol::read_frame(&invalid, ElapsedHeader::Absent).unwrap();
            assert!(
                v19::decode_command(
                    frame.commands().next().unwrap(),
                    DecodeAdmission::ClientPresentation
                )
                .is_err()
            );
            assert!(hud::parse_leaderboard(b"l\0\0\0\0\0\0").is_err());
            assert!(minimap::parse_minimap(b"M\0\x02\xff").is_err());
        }),
        0
    );
}

#[test]
fn measure_owned_state_and_presentation_separately_from_reader() {
    let setup = setup();
    let appearance = appearance();
    let packet = bundle(&[&setup, &appearance]);
    let mut state = ClientState::default();
    let received = allocations(|| {
        black_box(state.apply_frame(&packet, ElapsedHeader::Absent).unwrap());
    });
    let mut state = ClientState::default();
    let events = allocations(|| {
        black_box(
            state
                .apply_frame_with_events(&packet, ElapsedHeader::Absent)
                .unwrap(),
        );
    });
    let mut client = AnimatedClient::new(0.).unwrap();
    let presentation = allocations(|| {
        black_box(client.apply_frame(&packet, ElapsedHeader::Absent).unwrap());
    });
    eprintln!(
        "owned allocations, synthetic Setup + appearance: received={received}, received_with_events={events}, presentation_inclusive={presentation}"
    );
    assert_eq!(state.frames_applied, 1);
    assert_eq!(client.received().frames_applied, 1);
}

#[test]
fn read_only_control_probe_allocates_nothing_in_a_populated_world() {
    use slither_client::presentation::ClientPresentationError;
    let mut client = AnimatedClient::new(0.).unwrap();
    client
        .apply_frame(&bundle(&[&setup(), &appearance()]), ElapsedHeader::Absent)
        .unwrap();
    let snakes: Vec<_> = (100..140_u16)
        .map(|id| {
            let mut snake = appearance();
            snake[1..3].copy_from_slice(&id.to_be_bytes());
            snake
        })
        .collect();
    for snake in snakes {
        client
            .apply_frame(&bundle(&[&snake]), ElapsedHeader::Absent)
            .unwrap();
    }
    assert_eq!(client.received().snakes.len(), 41);
    let before = client.clone();
    let view = before.control_view();
    for now in [0., 8., 80., 7.] {
        let mut calls = 0;
        assert_eq!(
            allocations(|| {
                let result = client.advance_with_control::<ClientPresentationError, _>(
                    now,
                    true,
                    |_, actual| {
                        calls += 1;
                        assert_eq!(actual, view);
                        Err(ClientPresentationError::Limit)
                    },
                );
                assert_eq!(result, Err(ClientPresentationError::Limit));
            }),
            0
        );
        assert_eq!(calls, 1);
        assert_eq!(client, before);
    }
    let mut expected = before;
    assert_eq!(
        client.advance(8., false).unwrap(),
        expected.advance(8., false).unwrap()
    );
    assert_eq!(client, expected);
}
