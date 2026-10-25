#![cfg(feature = "networking")]

use slither_sim::{
    Action, Config, Point,
    client::{SimulationConfig, SimulatorBackend},
};

fn config(radius: f64) -> SimulationConfig {
    SimulationConfig {
        agents: 2,
        world: Config {
            radius,
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn living_slot_rejoins_keep_the_new_body_clear_of_other_lives() {
    // This is the local host's exclusion policy, not an exact official threshold.
    for seed in [1, 7, 17, 29] {
        let mut backend = SimulatorBackend::new(seed, config(1500.)).unwrap();
        for slot in [0, 1, 0, 1] {
            let other = 1 - slot;
            let player = backend.world().snakes[other].clone();
            let generation = backend
                .frame_for(slot)
                .unwrap()
                .received
                .connection_generation;
            assert!(backend.world().snakes[slot].alive);

            backend.reconnect_slot(slot).unwrap();

            assert_eq!(backend.world().snakes[other], player);
            let newborn = &backend.world().snakes[slot];
            assert!(newborn.alive);
            assert_eq!(newborn.segments, 2);
            assert_eq!(
                backend
                    .frame_for(slot)
                    .unwrap()
                    .received
                    .connection_generation,
                generation + 1
            );

            let mut others = backend.world().clone();
            others.snakes[slot].alive = false;
            assert!(
                others.spawn_is_clear(newborn.head, newborn.heading, newborn.segments, 600.),
                "seed {seed}, rejoined slot {slot}"
            );
        }
    }
}

#[test]
fn crowded_rejoin_takes_the_most_open_candidate_without_replacing_the_neighbor() {
    let mut backend = SimulatorBackend::new(7, config(500.)).unwrap();
    // Move inward from the constructor's test ring. Once the remaining head is
    // within 175 of center, even the farthest candidate (radius <= 400) is too close.
    backend.advance_ticks(60).unwrap();
    for snake in &backend.world().snakes {
        assert!(snake.alive);
        assert!(snake.head.distance(Point::default()) < 175.);
        assert!(snake.head.distance(Point::default()) + 400. < 600.);
    }

    // No candidate clears 600 units at first; each join still succeeds, out
    // of contact. Later joins may find room beside the outward newborn.
    for (join, slot) in [0, 1, 0, 1].into_iter().enumerate() {
        let other = 1 - slot;
        let neighbor = backend.world().snakes[other].clone();
        let frame = backend.frame_for(slot).unwrap();
        backend.reconnect_slot(slot).unwrap();
        assert_eq!(backend.world().snakes[other], neighbor);
        let newborn = backend.world().snakes[slot].clone();
        assert!(newborn.alive);
        let after = backend.frame_for(slot).unwrap();
        assert_eq!(
            after.received.connection_generation,
            frame.received.connection_generation + 1
        );
        let mut others = backend.world().clone();
        others.snakes[slot].alive = false;
        let mut body = vec![newborn.head];
        body.extend(&newborn.body);
        let margin = others.spawn_margin(&body).unwrap();
        assert!(margin > 0., "join {join} margin {margin}");
        assert!(join > 0 || !others.body_is_clear(&body, 600.));
    }
}

#[test]
fn rejoining_does_not_kill_a_player_continuing_its_motion() {
    for seed in [1, 7, 17, 29] {
        let mut backend = SimulatorBackend::new(seed, config(1500.)).unwrap();
        backend.advance_ticks(60).unwrap();
        let player = backend.world().snakes[0].clone();
        backend.reconnect_slot(1).unwrap();
        assert_eq!(backend.world().snakes[0], player);

        // Advance the actual admitted world using the core, without background
        // steering or auto-respawn hiding a collision. tests/spawn.rs retains the
        // old 120-unit hazard over this same 40-tick reaction window.
        let mut world = backend.world().clone();
        let actions: Vec<_> = world
            .snakes
            .iter()
            .map(|snake| {
                Some(Action {
                    heading: snake.heading,
                    boost: false,
                })
            })
            .collect();
        for tick in 0..40 {
            world.step(&actions).unwrap();
            assert!(
                world.snakes[0].alive,
                "seed {seed}, player died on tick {tick} after rejoin"
            );
        }
        assert!(world.snakes[0].head.distance(player.head) > 40.);
    }
}
