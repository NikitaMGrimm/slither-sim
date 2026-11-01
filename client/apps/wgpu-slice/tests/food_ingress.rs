//! Authored synthetic food flood; allocation counts avoid wall-clock test gates.
#[path = "support/announcement_tape.rs"]
mod host;
#[path = "../../../tests/support/protocol13.rs"]
mod wire;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct Counting;
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
    static BYTES: Cell<usize> = const { Cell::new(0) };
    static FAIL_AT_LEAST: Cell<usize> = const { Cell::new(usize::MAX) };
}
fn fail_allocation(bytes: usize) -> bool {
    FAIL_AT_LEAST
        .try_with(|limit| {
            if bytes >= limit.get() {
                limit.set(usize::MAX);
                true
            } else {
                false
            }
        })
        .unwrap_or(false)
}
fn charge(bytes: usize) {
    if ENABLED.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNT.try_with(|n| n.set(n.get() + 1));
        let _ = BYTES.try_with(|n| n.set(n.get() + bytes));
    }
}
// Instrument only this test's thread and forward the allocator contract to System.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        charge(layout.size());
        if fail_allocation(layout.size()) {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        charge(layout.size());
        if fail_allocation(layout.size()) {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        charge(size);
        if fail_allocation(size) {
            return std::ptr::null_mut();
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn ready(protocol: u8, initial_count: u16) -> slither_wgpu_slice::game::LocalGame {
    let mut g = host::negotiating();
    let mut setup = wire::setup();
    setup[23] = protocol;
    g.receive(g.generation(), &wire::bundle(&[setup, wire::snake(7)]), 4.)
        .unwrap();
    host::pictures(&mut g);
    let mut initial = vec![b'F'];
    if protocol == 19 {
        initial.extend([10, 10]);
    }
    for i in 0..initial_count {
        if protocol == 13 {
            initial.extend(&wire::food(b'f', 7, 1000 + i % 300, 1000 + i / 300, 20)[1..]);
        } else {
            initial.extend([7, (i % 256) as u8, (i / 256) as u8, 20]);
        }
    }
    g.receive(g.generation(), &initial, 10.).unwrap();
    assert!(host::pump(&mut g, 300.).contains(&vec![251]));
    g
}

fn food(protocol: u8, code: u8, i: u16) -> Vec<u8> {
    if protocol == 13 {
        wire::food(code, 8, 2000 + i % 300, 2000 + i / 300, 25)
    } else if code == b'F' {
        vec![code, 20, 20, 8, (i % 256) as u8, (i / 256) as u8, 25]
    } else {
        vec![code, 20, 20, (i % 256) as u8, (i / 256) as u8, 8, 25]
    }
}

fn own_move(protocol: u8) -> Vec<u8> {
    if protocol == 13 {
        wire::anchor(b'g', 7, &[4, 176, 4, 76], None)
    } else {
        vec![b'=', 0, 0, 4, 176, 4, 76]
    }
}

#[test]
fn retained_food_is_not_recopied_by_read_only_world_snapshots() {
    let game = ready(13, 6000);
    let mut snapshots = Vec::with_capacity(3);
    let food_bytes = std::mem::size_of_val(game.client().foods().foods());
    BYTES.set(0);
    ENABLED.set(true);
    for _ in 0..3 {
        snapshots.push(game.client().clone());
    }
    ENABLED.set(false);
    let allocated = BYTES.get();
    // Snake channels still copy. The three transaction snapshots together
    // must allocate less than one full retained-food presentation array.
    assert!(
        allocated < food_bytes,
        "snapshots={allocated}, food={food_bytes}"
    );
    for snapshot in snapshots {
        assert_eq!(&snapshot, game.client());
    }
    println!("6000 foods, 3 snapshots: allocated_bytes={allocated}, food_bytes={food_bytes}");
}

#[test]
fn received_history_is_not_recopied_by_ingress_snapshots() {
    for protocol in [13, 19] {
        let mut bytes = Vec::new();
        for count in [64, 6000] {
            let game = ready(protocol, count);
            let mut snapshots = Vec::with_capacity(3);
            BYTES.set(0);
            ENABLED.set(true);
            for _ in 0..3 {
                snapshots.push(game.client().clone());
            }
            ENABLED.set(false);
            bytes.push(BYTES.get());
            for snapshot in snapshots {
                assert_eq!(&snapshot, game.client());
            }
        }
        assert!(
            bytes[1] <= bytes[0] + 4096,
            "protocol={protocol}, bytes={bytes:?}"
        );
    }
}

#[test]
fn frozen_received_snapshots_survive_fast_and_full_ingress_and_rollback() {
    for protocol in [13, 19] {
        let mut game = ready(protocol, 64);
        let frozen = game.client().clone();
        let frozen_debug = format!("{frozen:?}");
        let received = frozen.received().clone();
        let motion = if protocol == 13 {
            vec![b'3', 0, 7, 32, 64]
        } else {
            vec![b'3', 32, 64]
        };
        let growth = if protocol == 13 {
            wire::anchor(b'n', 7, &[4, 177, 4, 77], Some(0x123456))
        } else {
            vec![b'+', 0, 7, 4, 177, 4, 77, 0x12, 0x34, 0x56]
        };
        let consume = if protocol == 13 {
            wire::eaten(1000, 1000, Some(7))
        } else {
            vec![b'<', 10, 10, 0, 0, 0, 7]
        };
        let packets = [
            motion,
            food(protocol, b'f', 0),
            own_move(protocol),
            growth,
            consume,
            vec![b'h', 0, 7, 0x12, 0x34, 0x57],
            wire::snake(300),
        ];
        for (i, packet) in packets.iter().enumerate() {
            game.receive(game.generation(), packet, 301. + i as f64)
                .unwrap();
            assert_eq!(frozen.received(), &received);
            assert_eq!(format!("{frozen:?}"), frozen_debug);
        }
        let before = game.client().clone();
        let before_debug = format!("{before:?}");
        let invalid = wire::bundle(&[own_move(protocol), vec![b'h', 0, 7, 0]]);
        assert!(game.receive(game.generation(), &invalid, 320.).is_err());
        assert_eq!(format!("{:?}", game.client()), before_debug);
        assert_eq!(game.client(), &before);
        let mut reset = game.client().clone();
        reset.reset();
        assert!(reset.received().setup.is_none());
        assert_eq!(frozen.received(), &received);
        assert_eq!(format!("{frozen:?}"), frozen_debug);
    }
}

#[test]
fn failed_shared_food_detach_preserves_the_staged_packet_and_snapshot() {
    for protocol in [13, 19] {
        let mut game = ready(protocol, 6000);
        let frozen = game.client().clone();
        let packet = food(protocol, b'f', 0);
        FAIL_AT_LEAST.set(std::mem::size_of_val(frozen.foods().foods()));
        let result = game.receive(game.generation(), &packet, 301.);
        let pending = FAIL_AT_LEAST.replace(usize::MAX);
        assert_eq!(pending, usize::MAX, "detach reservation was not reached");
        assert!(result.is_err());
        assert_eq!(game.client(), &frozen);
        // The failed packet cannot advance serials, caches, counters or session
        // controls; retrying it after the injected failure succeeds once.
        game.receive(game.generation(), &packet, 302.).unwrap();
        assert_eq!(
            game.client().received().frames_applied,
            frozen.received().frames_applied + 1
        );
        assert_eq!(
            game.client().foods().foods().len(),
            frozen.foods().foods().len() + 1
        );
        assert_eq!(frozen.foods().foods().len(), 6000);
    }
}

#[test]
fn singleton_motion_allocation_is_independent_of_retained_food() {
    for protocol in [13, 19] {
        let mut bytes = Vec::new();
        for initial_count in [64, 6000] {
            let mut game = ready(protocol, initial_count);
            let packet = if protocol == 13 {
                vec![b'3', 0, 7, 32, 64]
            } else {
                vec![b'3', 32, 64]
            };
            BYTES.set(0);
            ENABLED.set(true);
            for i in 0..100 {
                game.receive(game.generation(), &packet, 301. + f64::from(i))
                    .unwrap();
            }
            ENABLED.set(false);
            bytes.push(BYTES.get());
            assert_eq!(
                game.client().received().foods.len(),
                usize::from(initial_count)
            );
            assert_eq!(
                game.client().foods().foods().len(),
                usize::from(initial_count)
            );
            assert_eq!(
                game.client().received().snakes[&7].wanted_heading,
                slither_protocol::scalar::angle8(64)
            );
        }
        assert!(
            bytes[1] <= bytes[0] * 2 + 64 * 1024,
            "protocol={protocol}, bytes={bytes:?}"
        );
        println!("protocol={protocol}, 100 motion messages, 64/6000 food bytes={bytes:?}");
    }
}

#[test]
fn motion_fallback_and_decode_failure_preserve_the_session_and_world() {
    let mut game = ready(13, 64);
    let ignored = game.client().received().ignored_commands;
    // Protocol 13 ignores this family; the singleton optimization declines it.
    game.receive(game.generation(), &[b'd', 0, 7], 301.)
        .unwrap();
    assert_eq!(game.client().received().ignored_commands, ignored + 1);
    let before = game.client().clone();
    assert!(game.receive(game.generation(), b"E", 302.).is_err());
    assert_eq!(game.client(), &before);
    game.receive(game.generation(), &[b'3', 0, 7, 32, 64], 303.)
        .unwrap();
    assert_eq!(
        game.client().received().snakes[&7].wanted_heading,
        slither_protocol::scalar::angle8(64)
    );
    assert_eq!(game.phase(), slither_client::session_policy::Phase::Playing);
}

fn factory(
    spawn: &slither_client::state::FoodSpawnInput,
    _: f64,
) -> Result<
    slither_client::presentation::FoodFactory,
    slither_client::presentation::ClientPresentationError,
> {
    use slither_client::presentation::{
        BodyAsset, BodyAssetStatus, FoodFactory, FoodSource, food_palette,
    };
    let source = FoodSource::new(
        BodyAsset {
            handle: 1,
            revision: 1,
            width: 20,
            height: 16,
            status: BodyAssetStatus::Ready,
        },
        20.,
        16.,
        10.,
        8.,
    )
    .unwrap();
    Ok(FoodFactory {
        random_phase_unit: 0.,
        random_wobble_unit: 0.5,
        palette: food_palette(spawn.raw_color),
        image_count: 1,
        bodies: vec![source],
        outlines: vec![source],
        glows: vec![source],
    })
}

fn individual_food_ingress(protocol: u8, code: u8) {
    let mut counts = Vec::new();
    let mut core_counts = Vec::new();
    let mut bytes = Vec::new();
    let mut core_bytes = Vec::new();
    for initial_count in [64u16, 6000] {
        let mut g = ready(protocol, initial_count);
        let before = g.client().received().snakes[&7].head;
        let packet = food(protocol, code, 0);
        // The factory API used by other frontends must also avoid world copies.
        let mut core = g.client().clone();
        // Warm one append so vector growth is not mistaken for per-record copying.
        core.apply_frame_with_factories(
            &packet,
            slither_client::protocol::ElapsedHeader::Absent,
            |_| Ok(slither_client::presentation::PairedFaceGeometry::default()),
            factory,
        )
        .unwrap();
        COUNT.set(0);
        BYTES.set(0);
        ENABLED.set(true);
        for _ in 0..100 {
            core.apply_frame_with_factories(
                &packet,
                slither_client::protocol::ElapsedHeader::Absent,
                |_| Ok(slither_client::presentation::PairedFaceGeometry::default()),
                factory,
            )
            .unwrap();
        }
        ENABLED.set(false);
        core_counts.push(COUNT.get());
        core_bytes.push(BYTES.get());
        assert_eq!(core.received().foods.len(), usize::from(initial_count) + 1);
        assert_eq!(core.foods().foods().len(), usize::from(initial_count) + 101);
        g.receive(g.generation(), &packet, 301.).unwrap();
        let frames = g.client().received().frames_applied;
        COUNT.set(0);
        BYTES.set(0);
        ENABLED.set(true);
        for _ in 0..100 {
            g.receive(g.generation(), &packet, 301.).unwrap();
        }
        ENABLED.set(false);
        counts.push(COUNT.get());
        bytes.push(BYTES.get());
        assert_eq!(g.client().received().frames_applied, frames + 100);
        assert_eq!(
            g.client().received().foods.len(),
            usize::from(initial_count) + 1
        );
        assert_eq!(
            g.client().foods().foods().len(),
            usize::from(initial_count) + 101
        );
        assert_eq!(
            g.client()
                .received()
                .foods
                .values()
                .filter(|f| f.color == 8)
                .count(),
            1
        );
        assert!(g.champion_session().ping_pending);
        g.receive(g.generation(), &own_move(protocol), 302.)
            .unwrap();
        g.receive(g.generation(), b"p", 303.).unwrap();
        assert_ne!(g.client().received().snakes[&7].head, before);
        assert!(!g.champion_session().ping_pending);
        assert!(host::pump(&mut g, 600.).contains(&vec![251]));
        let snapshot = g.client().received().clone();
        assert!(g.receive(g.generation(), b"f", 601.).is_err());
        assert_eq!(g.client().received(), &snapshot);
        g.receive(g.generation(), &wire::board(), 602.).unwrap();
        assert!(g.failure.is_none());
    }
    assert!(
        counts[1] <= counts[0] + 100,
        "LocalGame world copies: {counts:?}"
    );
    assert!(
        core_counts[1] <= core_counts[0] + 100,
        "core factory world copies: {core_counts:?}"
    );
    assert!(
        bytes[1] <= bytes[0] + 131_072,
        "LocalGame food-vector copies: {bytes:?}"
    );
    assert!(
        core_bytes[1] <= core_bytes[0] + 131_072,
        "core factory food-vector copies: {core_bytes:?}"
    );
}

#[test]
fn v13_single_food_flood_keeps_allocation_cost_independent_of_world_size() {
    for code in b"Ffb" {
        individual_food_ingress(13, *code);
    }
}

#[test]
fn v19_single_food_flood_keeps_allocation_cost_independent_of_world_size() {
    for code in b"Ffb" {
        individual_food_ingress(19, *code);
    }
}

#[test]
fn empty_v19_sector_fallback_preserves_controls_and_lifecycle_admission() {
    use slither_client::{
        presentation::ClientPresentationError, protocol::ElapsedHeader, session_policy::Phase,
    };
    let empty = [b'F', 20, 20];
    let mut negotiating = host::negotiating();
    assert!(!negotiating.champion_session().connected);
    assert_eq!(
        negotiating
            .receive(negotiating.generation(), &empty, 3.)
            .unwrap_err(),
        "Expected Setup after login"
    );
    assert_eq!(negotiating.phase(), Phase::Setup);
    assert_eq!(negotiating.client().received().frames_applied, 0);

    let mut g = ready(19, 64);
    let mut core = g.client().clone();
    assert!(
        core.try_apply_single_food::<ClientPresentationError, _>(
            &empty,
            ElapsedHeader::Absent,
            |_, _| panic!("an empty sector must not invoke the food factory"),
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(&core, g.client());
    let mut expected = g.client().received().clone();
    let foods = g.client().foods().clone();
    let session = g.champion_session();
    assert!(session.connected && session.ping_pending);
    g.receive(g.generation(), &empty, 301.).unwrap();
    expected.frames_applied += 1;
    expected.commands_applied += 1;
    expected.last_elapsed_ms = None;
    assert_eq!(g.client().received(), &expected);
    assert_eq!(g.client().foods(), &foods);
    assert_eq!(
        format!("{:?}", g.champion_session()),
        format!("{session:?}")
    );
    g.receive(g.generation(), b"p", 302.).unwrap();
    assert!(!g.champion_session().ping_pending);
    g.receive(g.generation(), &own_move(19), 303.).unwrap();
    g.receive(g.generation(), &wire::board(), 304.).unwrap();
    assert!(host::pump(&mut g, 600.).contains(&vec![251]));

    // A closed transport can retain Setup until its later world-reset frame.
    g.closed(g.generation());
    assert_eq!(g.phase(), Phase::Disconnected);
    assert!(!g.champion_session().connected);
    assert!(g.client().received().setup.is_some());
    let closed = g.client().received().clone();
    g.receive(g.generation(), &empty, 601.).unwrap();
    assert_eq!(g.client().received(), &closed);
    assert_eq!(g.phase(), Phase::Disconnected);
}

fn drain_18852(protocol: u8) {
    let mut g = ready(protocol, 6000);
    let frames = g.client().received().frames_applied;
    let before = g.client().received().snakes[&7].head;
    let started = std::time::Instant::now();
    for i in 0..18_852u16 {
        g.receive(g.generation(), &food(protocol, b'f', i), 301.)
            .unwrap();
        g.service_ingress().unwrap();
        host::effects(&mut g);
        host::pictures(&mut g);
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
    assert_eq!(g.client().received().frames_applied, frames + 18_852);
    assert_eq!(g.client().received().foods.len(), 24_852);
    assert_eq!(g.client().foods().foods().len(), 24_852);
    assert!(g.champion_session().ping_pending);
    g.receive(g.generation(), &own_move(protocol), 302.)
        .unwrap();
    g.receive(g.generation(), b"p", 303.).unwrap();
    assert_ne!(g.client().received().snakes[&7].head, before);
    assert!(!g.champion_session().ping_pending);
    println!(
        "ingress_drain protocol={protocol} messages=18852 initial_foods=6000 received_foods=24852 presentation_foods=24852 errors=0 elapsed_ms={elapsed_ms:.3}"
    );
}

#[test]
#[ignore = "manual CPU-only ingress timing; no network or GPU"]
fn v13_drain_18852_authored_food_records() {
    drain_18852(13);
}

#[test]
#[ignore = "manual CPU-only ingress timing; no network or GPU"]
fn v19_drain_18852_authored_food_records() {
    drain_18852(19);
}
