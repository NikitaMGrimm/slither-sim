//! Cross-revision allocation gate for warmed, inline presentation only.
#![cfg(feature = "perf-internals")]
use slither_sim::{
    Config, InitialPopulation, Placement, Simulator, WorldOptions,
    client::{SimulationConfig, simulation::RawWorld},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}
fn allocated() {
    ALLOCATIONS.with(|count| {
        if let Some(value) = count.get() {
            count.set(Some(value + 1));
        }
    });
}
struct Count;
unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated();
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Count = Count;

#[test]
fn warmed_inline_presentation_allocations() {
    let config = SimulationConfig {
        agents: 16,
        minimap_interval_seconds: 0.07,
        leaderboard_interval_seconds: 0.091,
        world: Config {
            radius: 16000.,
            food_count: 128,
            ambient_replenish_per_second: 0.,
            ..Config::sandbox()
        },
        ..Default::default()
    };
    let mut world = Simulator::new(WorldOptions {
        seed: 7,
        config: config.world.clone(),
        population: InitialPopulation::Placed {
            ids: (0..config.agents).map(|i| i.to_string()).collect(),
            placement: Placement::default(),
        },
    })
    .unwrap();
    for slot in 0..16 {
        world.retire(slot).unwrap();
        world
            .respawn(
                slot,
                slither_sim::Point {
                    x: (slot % 4) as f64 * 2500. - 3750.,
                    y: (slot / 4) as f64 * 2500. - 3750.,
                },
                0.,
                26,
            )
            .unwrap();
    }
    let mut raw = RawWorld::new(7, config, world).unwrap();
    for _ in 0..8 {
        raw.broadcast().unwrap();
        raw.drain().unwrap();
    }
    let mut counts = Vec::new();
    for _ in 0..32 {
        ALLOCATIONS.with(|count| count.set(Some(0)));
        raw.broadcast().unwrap();
        counts.push(ALLOCATIONS.with(|count| count.replace(None)).unwrap());
        raw.drain().unwrap();
    }
    println!("presentation allocations per broadcast: {counts:?}");
    // Pinned 216a3352, same fixture/seam and counted allocator calls.
    assert_eq!(counts, [7; 32]);
    let mut tick_counts = Vec::new();
    for _ in 0..32 {
        ALLOCATIONS.with(|count| count.set(Some(0)));
        raw.tick().unwrap();
        tick_counts.push(ALLOCATIONS.with(|count| count.replace(None)).unwrap());
        raw.drain().unwrap();
    }
    println!("continuation allocations per tick: {tick_counts:?}");
    assert_eq!(
        tick_counts,
        [
            176, 8, 9, 8, 8, 8, 8, 8, 8, 95, 8, 8, 144, 8, 8, 8, 8, 8, 8, 100, 8, 8, 8, 8, 8, 144,
            8, 8, 8, 97, 8, 8
        ]
    );
    assert_eq!(raw.world().snakes().iter().filter(|s| s.alive).count(), 16);
}

#[test]
fn allocation_probe_detects_a_deliberate_allocation() {
    ALLOCATIONS.with(|count| count.set(Some(0)));
    let allocation = std::hint::black_box(vec![0u8; std::hint::black_box(128)]);
    let count = ALLOCATIONS.with(|count| count.replace(None)).unwrap();
    drop(allocation);
    assert!(count > 0);
}
