//! Allocation gate for radius evolution after reusable tick buffers are warm.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {
    // libtest can allocate channel state on its main thread after this test
    // starts. The empty-world radius controller runs entirely on this thread.
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}
fn record_allocation() {
    ALLOCATIONS.with(|count| {
        if let Some(value) = count.get() {
            count.set(Some(value + 1));
        }
    });
}
struct Count;
unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Count = Count;
#[test]
fn warmed_radius_controller_and_samples_allocate_nothing() {
    let config = slither_sim::Config {
        dynamic_radius: true,
        food_count: 0,
        ambient_replenish_per_second: 0.,
        radius_updates: (1..=512)
            .map(|i| slither_sim::RadiusUpdate {
                time: i as f64 * 0.007,
                radius: 900. + (i % 3) as f64,
            })
            .collect(),
        ..slither_sim::Config::sandbox()
    };
    let mut world = slither_sim::Simulator::new(slither_sim::WorldOptions {
        config,
        ..Default::default()
    })
    .unwrap();
    world.step(&[]).unwrap();
    ALLOCATIONS.with(|count| count.set(Some(0)));
    for _ in 0..511 {
        world.step(&[]).unwrap();
        std::hint::black_box(world.rules());
    }
    assert_eq!(ALLOCATIONS.with(|count| count.replace(None)), Some(0));
    assert_eq!(world.config().radius, 1000.);
    assert_ne!(world.radius(), 1000.);
}

#[test]
fn allocation_probe_detects_allocations_on_the_tracked_thread() {
    ALLOCATIONS.with(|count| count.set(Some(0)));
    let allocated = std::hint::black_box(vec![0u8; std::hint::black_box(128)]);
    let count = ALLOCATIONS.with(|count| count.replace(None)).unwrap();
    drop(allocated);
    assert!(count > 0);
}
