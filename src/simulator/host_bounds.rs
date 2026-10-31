//! Reuse the engine's anchor extrema for read-only host visibility preparation.
use super::*;

impl Simulator {
    pub(crate) fn presentation_bounds(&self) -> Vec<[f64; 4]> {
        self.snakes
            .iter()
            .enumerate()
            .map(|(i, snake)| {
                // Geometry edits invalidate these extrema. Dead/dormant slots,
                // cold restores and pre-step broadcasts still use the oracle.
                let anchors = self
                    .buffers
                    .body_bounds
                    .get(i)
                    .copied()
                    .flatten()
                    .or_else(|| snake_body_bounds(&snake.body));
                let mut bounds = Bounds::between(snake.head, snake.head);
                if let Some(body) = anchors {
                    bounds.include(body.min);
                    bounds.include(body.max);
                }
                [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(world: &Simulator) {
        let expected: Vec<_> = world
            .snakes()
            .iter()
            .map(|snake| {
                snake.body.iter().fold(
                    [snake.head.x, snake.head.y, snake.head.x, snake.head.y],
                    |[x0, y0, x1, y1], p| [x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)],
                )
            })
            .collect();
        assert_eq!(world.presentation_bounds(), expected);
    }

    #[test]
    fn presentation_matches_linear_bounds_through_ticks_and_public_edits() {
        let mut world = Simulator::new(WorldOptions {
            seed: 7,
            config: Config {
                radius: 32000.,
                food_count: 0,
                ambient_replenish_per_second: 0.,
                prey: PreyConfig {
                    count: 0,
                    ..Default::default()
                },
                ..Config::sandbox()
            },
            population: InitialPopulation::Dormant {
                ids: vec!["0".into(), "1".into()],
            },
        })
        .unwrap();
        exact(&world);
        world
            .respawn_body(
                0,
                vec![Point { x: 500., y: 400. }, Point { x: 458., y: 400. }],
                0.,
            )
            .unwrap();
        exact(&world);
        for tick in 0..128 {
            world
                .step(&[
                    Some(Action {
                        heading: tick as f64 * 0.08,
                        boost: tick % 2 == 0,
                    }),
                    None,
                ])
                .unwrap();
            exact(&world);
            exact(&world.clone());
        }
        let head = world.snakes()[0].head;
        let body = vec![head, Point { x: 20., y: -30. }, Point { x: -60., y: 90. }];
        world
            .correct_pose(0, head, vec![head, body[1]], 0.)
            .unwrap();
        exact(&world);
        world.correct_mass(0, body.clone(), 0.).unwrap();
        exact(&world);
        world
            .import_life(
                0,
                ReplayLife {
                    head,
                    body: body.clone(),
                    heading: 0.,
                    fam: 0.,
                    kills: 0,
                    alive: true,
                    speed: InitialSpeed::Known(SpeedState::idle(base_speed(3))),
                },
            )
            .unwrap();
        exact(&world);
        world
            .step(&[
                Some(Action {
                    heading: 0.,
                    boost: false,
                }),
                None,
            ])
            .unwrap();
        exact(&world);
        world.retire(0).unwrap();
        exact(&world);
        world.respawn_body(0, body, 0.).unwrap();
        exact(&world);
        let restored = Simulator::from_checkpoint(
            &serde_json::to_vec(&world.checkpoint().unwrap()).unwrap(),
            Default::default(),
        )
        .unwrap();
        exact(&restored);
    }
}
