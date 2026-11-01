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
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        charge(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        charge(size);
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
