//! Synthetic collision-collector oracle using the production Simulator API.
//! No sockets or external data. Run: cargo run --locked --example collision_curve
use serde_json::json;
use slither_sim::{Action, Config, InitialPopulation, Point, Simulator, WorldOptions, physics};
use std::f64::consts::PI;

fn fixture(probe: usize, first: usize, offset: f64, head_on: bool) -> Simulator {
    let mut config = Config::sandbox();
    config.radius = 10000.;
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    let mut sim = Simulator::new(WorldOptions {
        seed: 7,
        config,
        population: InitialPopulation::Dormant {
            ids: vec!["0".into(), "1".into()],
        },
    })
    .unwrap();
    for i in [first, 1 - first] {
        let (x, y, heading) = if head_on {
            (
                if i == 0 { -200. } else { 200. },
                0.,
                if i == 0 { 0. } else { PI },
            )
        } else if i == probe {
            (-55., offset, 0.)
        } else {
            (0., 0., 0.)
        };
        sim.respawn(i, Point { x, y }, heading, 7).unwrap();
    }
    sim
}

fn run(probe: usize, first: usize, factor: f64, head_on: bool) -> [bool; 2] {
    let offset = factor * 0.975 * physics::BODY_RADIUS * physics::scale(7);
    let mut sim = fixture(probe, first, offset, head_on);
    let actions = [
        Some(Action {
            heading: 0.,
            boost: false,
        }),
        Some(Action {
            heading: if head_on { PI } else { 0. },
            boost: false,
        }),
    ];
    for _ in 0..430 {
        sim.step(&actions).unwrap();
    }
    [!sim.snakes()[0].alive, !sim.snakes()[1].alive]
}

fn main() {
    for probe in 0..2 {
        for factor in [1.3, 1.1, 0.9, 0.7] {
            let deaths = run(probe, probe, factor, false);
            println!(
                "{}",
                json!({"synthetic": true, "kind": "ladder", "segments": 7,
                "probe": probe, "factor": factor,
                "offset": factor * 0.975 * physics::BODY_RADIUS * physics::scale(7), "deaths": deaths})
            );
        }
    }
    for first in 0..2 {
        println!(
            "{}",
            json!({"synthetic": true, "kind": "head-on", "first": first,
            "deaths": run(0, first, 0., true)})
        );
    }
}

#[test]
fn current_rule_brackets_clearance_and_kills_the_oldest_on_mutual_contact() {
    for probe in 0..2 {
        for first in 0..2 {
            for factor in [1.3, 1.1, 0.9, 0.7] {
                let deaths = run(probe, first, factor, false);
                assert_eq!(
                    deaths[probe],
                    factor < 1.,
                    "probe={probe}, first={first}, factor={factor}"
                );
                assert!(!deaths[1 - probe]);
            }
        }
    }
    assert_eq!(run(0, 0, 0., true), [true, false]);
    assert_eq!(run(0, 1, 0., true), [false, true]);
}
