use super::*;
use serde_json::Value;

#[test]
fn population_sized_resets_hold_across_death_and_respawn() {
    const AGENTS: usize = 40;
    let radius = 670. * (AGENTS as f64).sqrt();
    for config in [None, Some(json!({})), Some(json!({"preset": "official"}))] {
        let mut request = json!({"op": "reset", "agents": AGENTS, "worlds": 2, "decisionTicks": 1});
        if let Some(config) = config {
            request["config"] = config;
        }
        let mut session = Session::new();
        let reply: Value =
            serde_json::from_slice(&session.request(&serde_json::to_vec(&request).unwrap()))
                .unwrap();
        assert_eq!(reply["result"]["config"]["radius"], radius);
        assert_eq!(reply["result"]["config"]["foodCount"], 109 * AGENTS);
        let world = &mut session.current.as_mut().unwrap().worlds[0].world;
        assert!(world.config().dynamic_radius);
        assert!(
            (world.config().ambient_replenish_per_second
                - Config::default().ambient_replenish_per_second * AGENTS as f64)
                .abs()
                < 1e-9
        );
        // Inject a boundary death and a real respawn in one reset world; the
        // other world retains its population to catch cross-world sizing.
        world.retire(0).unwrap();
        world
            .respawn(
                0,
                crate::Point {
                    x: radius - 1.,
                    y: 0.,
                },
                0.,
                2,
            )
            .unwrap();
        session
            .step(&[0.; AGENTS * 2], &[false; AGENTS * 2])
            .unwrap();
        assert!(session.frame(0).unwrap().living < AGENTS as u32);
        assert_eq!(session.frame(0).unwrap().radius, radius);
        session.current.as_mut().unwrap().worlds[0]
            .world
            .respawn(0, crate::Point::default(), 0., 2)
            .unwrap();
        session
            .step(&[0.; AGENTS * 2], &[false; AGENTS * 2])
            .unwrap();
        assert_eq!(session.frame(0).unwrap().radius, radius);
        for state in &session.current.as_ref().unwrap().worlds {
            assert_eq!(state.world.config().radius, radius);
        }
    }
}

#[test]
fn fixed_reset_worlds_keep_their_radius_and_food_settings() {
    for config in [
        json!({"preset": "battledome"}),
        json!({"preset": "sandbox"}),
        json!({"radius": 900}),
        json!({"preset": "official", "radius": 900}),
        json!({"sizeToPopulation": false}),
        json!({"radius": 9000, "sizeToPopulation": true}),
    ] {
        let expected: Config = serde_json::from_value(config.clone()).unwrap();
        let mut session = Session::new();
        let reply: Value = serde_json::from_slice(&session.request(
            &serde_json::to_vec(&json!({"op": "reset", "agents": 40, "config": config})).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            reply["result"]["config"],
            serde_json::to_value(expected).unwrap()
        );
    }
}
