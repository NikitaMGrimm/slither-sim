use slither_sim::*;

#[test]
fn capture_telemetry_identifies_the_eater_without_changing_the_simulation() {
    let mut world = Simulator::new(
        42,
        Config {
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Config::default()
        },
    )
    .unwrap();
    world.add("a", 0, 2).unwrap();
    world.add("b", 1, 2).unwrap();
    let s = &world.snakes[1];
    // Place both items at the center of snake b's pickup circle, ahead of its head.
    let (head, _) = physics::pickup_circle(s.head, s.heading, s.speed, s.segments);
    world.food.push(Food {
        x: head.x,
        y: head.y,
        size: Some(5.),
        value: 1.,
        origin: Some(Origin::Ambient),
    });
    world.prey.push(Prey {
        id: 51,
        position: head,
        heading: 0.,
        target_heading: 0.,
        speed: 120.,
        size: 6.,
    });
    let mut traced = world.clone();
    assert!(world.pickup_events.is_none());
    traced.pickup_events = Some(vec![]);
    for tick in 0..200 {
        let actions = [Some(Action::default()), Some(Action::default())];
        world.step(&actions).unwrap();
        traced.step(&actions).unwrap();
        assert_eq!(world.snakes, traced.snakes);
        assert_eq!(world.food, traced.food);
        assert_eq!(world.prey, traced.prey);
        assert_eq!(world.deaths, traced.deaths);
        let events = traced.pickup_events.as_ref().unwrap();
        if tick == 0 {
            assert_eq!(events.len(), 2);
            assert!(events.iter().all(|e| e.eater == 1));
            assert!(matches!(&events[0].item, PickupItem::Food(f) if f.point() == head));
            assert!(matches!(&events[1].item, PickupItem::Prey(p) if p.id == 51));
        } else {
            assert!(
                events.is_empty(),
                "events must not repeat on subsequent steps"
            );
        }
    }
}
