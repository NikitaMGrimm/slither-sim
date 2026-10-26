//! Allocation gate for radius evolution after reusable tick buffers are warm.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
static TRACK: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
struct Count;
unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
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
    ALLOCATIONS.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    for _ in 0..511 {
        world.step(&[]).unwrap();
    }
    TRACK.store(false, Ordering::Relaxed);
    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0);
    assert_eq!(world.config().radius, 1000.);
    assert_ne!(world.radius(), 1000.);
}
