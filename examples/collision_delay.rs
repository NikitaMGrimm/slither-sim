//! Local-only counterfactual collision study; never changes hosted behavior.
//! Each snake moves in its own Simulator; `strikes` scores their joined histories.
use serde_json::json;
use slither_sim::{
    Action, Config, InitialPopulation, Point, Simulator, Snake, WorldOptions, physics,
};
use std::f64::consts::FRAC_PI_2;

const DT: f64 = 0.007;

fn history(point: Point, heading: f64, steps: usize) -> Vec<Snake> {
    let mut config = Config::sandbox();
    config.radius = 10000.;
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    let mut sim = Simulator::new(WorldOptions {
        seed: 7,
        config,
        population: InitialPopulation::Dormant {
            ids: vec!["only".into()],
        },
    })
    .unwrap();
    sim.respawn(0, point, heading, 7).unwrap();
    let action = [Some(Action {
        heading,
        boost: false,
    })];
    let mut states = vec![sim.snakes()[0].clone()];
    for _ in 0..steps {
        sim.step(&action).unwrap();
        states.push(sim.snakes()[0].clone());
    }
    states
}

fn contact(probe: &[Snake], reference: &[Snake], rewind_steps: usize, whole_state: bool) -> bool {
    for k in rewind_steps.max(1)..probe.len() {
        let pidx = if whole_state { k - rewind_steps } else { k };
        if pidx == 0 {
            continue;
        }
        let p = &probe[pidx];
        let r = &reference[k - rewind_steps];
        let line = [physics::head_tip(r.head, r.heading, r.segments), r.head]
            .into_iter()
            .chain(r.body.iter().copied());
        if physics::strikes(
            physics::head_tip(probe[pidx - 1].head, p.heading, p.segments),
            physics::head_tip(p.head, p.heading, p.segments),
            p.heading,
            p.segments,
            r.segments,
            line,
        ) {
            return true;
        }
    }
    false
}

fn main() {
    let reference = history(Point { x: 0., y: 0. }, 0., 1000);
    let speed = reference[500].speed;
    for repeat in 0..2 {
        let crossing = 400 + repeat * 17;
        let tail = reference[crossing].body.last().unwrap();
        for delay_steps in [0, 21, 64] {
            for whole_state in [false, true] {
                for step in 0..6 {
                    let offset = -240. + 20. * (step * 2 + repeat) as f64;
                    let probe = history(
                        Point {
                            x: tail.x + offset,
                            y: -reference[crossing].head.x,
                        },
                        FRAC_PI_2,
                        1000,
                    );
                    let hit = contact(&probe, &reference, delay_steps, whole_state);
                    println!(
                        "{}",
                        json!({ "synthetic": true, "repeat": repeat, "step": step,
                            "geometry": "finite-tail-crossing", "offset": offset,
                            "segments": 7, "delaySteps": delay_steps,
                            "delayMs": delay_steps as f64 * DT * 1000.,
                            "wholeState": whole_state, "speed": speed,
                            "predictedShift": -speed * delay_steps as f64 * DT,
                            "contact": hit,
                            "crossingStep": crossing, "tailAtCrossing": tail,
                        })
                    );
                }
            }
        }
    }
}

#[test]
fn counterfactual_peer_rewind_is_detectable_but_whole_state_rewind_is_not() {
    let reference = history(Point { x: 0., y: 0. }, 0., 1000);
    let crossing = 400;
    let probe = history(
        Point {
            x: reference[crossing].body.last().unwrap().x - 100.,
            y: -reference[crossing].head.x,
        },
        FRAC_PI_2,
        1000,
    );
    assert!(!contact(&probe, &reference, 0, false));
    assert!(contact(&probe, &reference, 64, false));
    assert!(!contact(&probe, &reference, 64, true));
}

#[test]
fn small_rewind_can_be_hidden_by_anchor_phase() {
    let reference = history(Point { x: 0., y: 0. }, 0., 1000);
    for (crossing, expected) in [(400, true), (417, false)] {
        let probe = history(
            Point {
                x: reference[crossing].body.last().unwrap().x - 80.,
                y: -reference[crossing].head.x,
            },
            FRAC_PI_2,
            1000,
        );
        assert!(!contact(&probe, &reference, 0, false));
        assert_eq!(contact(&probe, &reference, 21, false), expected);
    }
}
