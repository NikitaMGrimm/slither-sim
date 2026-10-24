use slither_sim::{Action, Config, Food, Observation, Point, Simulator, physics, reference};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    f64::consts::PI,
};

struct CountAllocations;
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn allocated() {
    if COUNTING.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

unsafe impl GlobalAlloc for CountAllocations {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc_zeroed(layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountAllocations = CountAllocations;

fn fixture() -> Observation {
    let mut world = Simulator::new(1, Config::sandbox()).unwrap();
    world.add("self", 0, 1).unwrap();
    let mut observation = world.observe(0).unwrap();
    observation.me.head = Point::default();
    observation.me.heading = 0.0;
    observation.me.body = vec![Point { x: -42.0, y: 0.0 }];
    observation.food.clear();
    observation.prey.clear();
    observation
}

fn food(x: f64, y: f64) -> Food {
    Food {
        x,
        y,
        value: 1.0,
        size: None,
        origin: None,
    }
}

#[test]
fn food_is_reselected_and_turn_cost_rejects_a_closer_pellet_behind() {
    let mut observation = fixture();
    observation.food = vec![food(-10.0, 0.0), food(100.0, 0.0)];
    assert_eq!(reference::decide(&observation), Action::default());
    observation.food = vec![food(100.0, 100.0)];
    assert!(reference::decide(&observation).heading > 0.0);
    observation.food = vec![food(100.0, -100.0)];
    assert!(reference::decide(&observation).heading < 0.0);
}

#[test]
fn equal_food_costs_keep_observation_order() {
    let mut observation = fixture();
    observation.food = vec![food(150.0, 50.0), food(150.0, -50.0)];
    assert!(reference::decide(&observation).heading > 0.0);
    observation.food.reverse();
    assert!(reference::decide(&observation).heading < 0.0);
}

#[test]
fn boundary_overrides_outward_food_with_a_turn_toward_the_arena() {
    let mut observation = fixture();
    observation.me.head.x = 850.0;
    observation.food = vec![food(980.0, 0.0)];
    let action = reference::decide(&observation);
    assert!(action.heading.cos() < 0.0, "{action:?}");
    assert!(!action.boost);
}

#[test]
fn body_edge_penalty_changes_an_otherwise_straight_food_route() {
    let mut observation = fixture();
    observation.food = vec![food(200.0, 0.0)];
    assert_eq!(reference::decide(&observation).heading, 0.0);
    let mut opponent = observation.me.clone();
    opponent.id = "opponent".into();
    opponent.head = Point { x: 75.0, y: 300.0 };
    opponent.body = vec![Point { x: 75.0, y: 250.0 }, Point { x: 75.0, y: -250.0 }];
    observation.opponents.push(opponent);
    assert!(reference::decide(&observation).heading.abs() > 0.5);
}

#[test]
fn nearby_head_penalty_changes_an_otherwise_straight_food_route() {
    let mut observation = fixture();
    observation.food = vec![food(200.0, 0.0)];
    let mut opponent = observation.me.clone();
    opponent.head = Point { x: 95.0, y: 0.0 };
    opponent.body.clear();
    observation.opponents.push(opponent);
    assert!(reference::decide(&observation).heading.abs() > 0.5);
}

#[test]
fn nonzero_arena_center_and_repeated_body_points_are_supported() {
    let mut observation = fixture();
    observation.me.head = Point {
        x: 1850.0,
        y: 1000.0,
    };
    observation.arena.center = Point {
        x: 1000.0,
        y: 1000.0,
    };
    let mut opponent = observation.me.clone();
    opponent.head = Point {
        x: 1800.0,
        y: 1100.0,
    };
    opponent.body = vec![opponent.head; 3];
    observation.opponents.push(opponent);
    let action = reference::decide(&observation);
    assert!(action.heading.is_finite() && action.heading.cos() < 0.0);
}

#[test]
fn empty_food_and_large_snakes_return_repeatable_nonboosting_actions() {
    let mut observation = fixture();
    observation.me.segments = 430;
    observation.me.speed = physics::base_speed(430);
    observation.me.radius = physics::BODY_RADIUS * physics::scale(430);
    observation.me.heading = PI;
    let first = reference::decide(&observation);
    for _ in 0..32 {
        assert_eq!(reference::decide(&observation), first);
    }
    assert!(first.heading.is_finite() && !first.boost);
}

#[test]
fn decision_does_not_allocate_or_mutate_the_observation() {
    let mut observation = fixture();
    observation.food = vec![food(100.0, 50.0); 128];
    let mut opponent = observation.me.clone();
    opponent.head = Point { x: 80.0, y: 120.0 };
    opponent.body = vec![Point { x: 80.0, y: 120.0 }; 430];
    observation.opponents.push(opponent);
    let original = observation.clone();
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    for _ in 0..32 {
        std::hint::black_box(reference::decide(std::hint::black_box(&observation)));
    }
    COUNTING.with(|counting| counting.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(observation, original);
}
