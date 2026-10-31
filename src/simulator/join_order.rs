use super::Snake;

/// Collision order is derived scratch: clones and checkpoint restores rebuild it.
#[derive(Default)]
pub(super) struct JoinOrder {
    slots: Vec<usize>,
    joiners: Vec<usize>,
    next: Option<u64>,
}

impl JoinOrder {
    pub(super) fn invalidate(&mut self) {
        self.next = None;
    }

    pub(super) fn update(
        &mut self,
        alive: &[usize],
        snakes: &[Snake],
        joins: &[u64],
        next_join: u64,
    ) -> &[usize] {
        if let Some(previous_next) = self.next.filter(|&n| n <= next_join) {
            // New lives replace their slot's old key with a monotonically newer
            // one. Drop that old entry even if the slot has already respawned.
            self.slots
                .retain(|&i| snakes.get(i).is_some_and(|s| s.alive) && joins[i] < previous_next);
            self.joiners.clear();
            self.joiners
                .extend(alive.iter().copied().filter(|&i| joins[i] >= previous_next));
            self.joiners.sort_unstable_by_key(|&i| joins[i]);
            self.slots.extend_from_slice(&self.joiners);
        }
        // A cold cache, or a private edit reviving a life without a new join,
        // requires the original full sort. Public corrections preserve joins.
        if self.next.is_none()
            || self.slots.len() != alive.len()
            || self.next.is_some_and(|n| n > next_join)
        {
            self.slots.clear();
            self.slots.extend_from_slice(alive);
            self.slots.sort_unstable_by_key(|&i| joins[i]);
        }
        self.next = Some(next_join);
        &self.slots
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Config, InitialPopulation, InitialSpeed, Placement, ReplayLife, Simulator, WorldOptions,
    };

    fn world() -> Simulator {
        Simulator::new(WorldOptions {
            seed: 71,
            config: Config {
                food_count: 0,
                ambient_replenish_per_second: 0.,
                ..Config::sandbox()
            },
            population: InitialPopulation::Placed {
                ids: (0..12).map(|i| format!("s{i}")).collect(),
                placement: Placement::default(),
            },
        })
        .unwrap()
    }
    fn check(world: &mut Simulator) {
        let alive: Vec<_> = world
            .snakes
            .iter()
            .enumerate()
            .filter(|(_, s)| s.alive)
            .map(|(i, _)| i)
            .collect();
        let mut expected = alive.clone();
        expected.sort_unstable_by_key(|&i| world.joins[i]);
        assert_eq!(
            world
                .buffers
                .order
                .update(&alive, &world.snakes, &world.joins, world.next_join),
            expected
        );
    }
    fn rejoin(world: &mut Simulator, i: usize) {
        let mut life = ReplayLife::from_observed(
            world.snakes[i].clone(),
            InitialSpeed::Known(world.speeds[i]),
        );
        life.alive = true;
        world.import_life(i, life).unwrap();
    }
    #[test]
    fn collision_join_order_tracks_lifecycle_and_direct_revival() {
        let mut world = world();
        check(&mut world);
        for i in [10, 2, 10, 0, 8] {
            rejoin(&mut world, i);
        }
        world.retire(1).unwrap();
        world.retire(6).unwrap();
        check(&mut world);
        world.snakes[1].alive = true; // Private fixture edit without a new key.
        check(&mut world);
        let points = world.snakes[2].body.clone();
        world.correct_mass(2, points.clone(), 0.5).unwrap();
        assert!(world.buffers.order.next.is_none());
        check(&mut world);
        world
            .correct_pose(2, world.snakes[2].head, points, 0.2)
            .unwrap();
        assert!(world.buffers.order.next.is_none());
        check(&mut world);
        let i = world.append_dormant("new".into()).unwrap();
        rejoin(&mut world, i);
        check(&mut world);
        for s in &mut world.snakes {
            s.alive = false;
        }
        check(&mut world);
        world.next_join = u64::MAX - 3;
        rejoin(&mut world, 4);
        rejoin(&mut world, 2);
        check(&mut world);
        assert!(
            world
                .import_life(
                    0,
                    ReplayLife::from_observed(
                        world.snakes[0].clone(),
                        InitialSpeed::Known(world.speeds[0])
                    )
                )
                .is_err()
        );
        check(&mut world);
    }
    #[test]
    fn collision_join_order_matches_cold_clone_and_restore_steps() {
        let mut warm = world();
        warm.record_events(crate::EventRecording {
            pickups: true,
            food_births: true,
        });
        for tick in 0..16 {
            rejoin(&mut warm, (tick * 7) % 12);
            warm.retire((tick * 3 + 1) % 12).unwrap();
            let mut cold = warm.clone();
            assert!(cold.buffers.order.next.is_none());
            let bytes = serde_json::to_vec(&warm.checkpoint().unwrap()).unwrap();
            let mut restored =
                Simulator::from_checkpoint(&bytes, crate::RestoreOptions::default()).unwrap();
            restored.record_events(crate::EventRecording {
                pickups: true,
                food_births: true,
            });
            assert!(restored.buffers.order.next.is_none());
            let actions: Vec<_> = warm
                .snakes
                .iter()
                .map(|s| {
                    s.alive.then_some(crate::Action {
                        heading: s.heading,
                        boost: false,
                    })
                })
                .collect();
            for w in [&mut warm, &mut cold, &mut restored] {
                w.step(&actions).unwrap();
            }
            assert_eq!(warm.buffers.order.slots, cold.buffers.order.slots);
            assert_eq!(warm.buffers.order.slots, restored.buffers.order.slots);
            for w in [&cold, &restored] {
                assert_eq!(
                    serde_json::to_vec(&warm.checkpoint().unwrap()).unwrap(),
                    serde_json::to_vec(&w.checkpoint().unwrap()).unwrap()
                );
                assert_eq!(warm.deaths(), w.deaths());
                assert_eq!(warm.pickup_events(), w.pickup_events());
                assert_eq!(warm.food_spawn_events(), w.food_spawn_events());
            }
        }
    }
    #[test]
    fn collision_join_order_random_transitions_match_sort() {
        let mut w = world();
        let mut rng = 0x4312_aced_u64;
        for _ in 0..128 {
            for _ in 0..5 {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                let slot = rng as usize % 12;
                if rng & 4 == 0 {
                    w.retire(slot).unwrap();
                } else {
                    rejoin(&mut w, slot);
                }
            }
            check(&mut w);
        }
    }
    #[test]
    fn collision_join_order_keeps_newest_killer_after_rejoining() {
        let mut warm = world();
        for i in 3..12 {
            warm.retire(i).unwrap();
        }
        // Victim's swept tip reaches the two newer vertical bodies. Both can
        // kill it; join chronology, not slot number, chooses the winner.
        warm.correct_pose(
            0,
            crate::Point::default(),
            vec![
                crate::Point { x: -42., y: 0. },
                crate::Point { x: -84., y: 0. },
            ],
            0.,
        )
        .unwrap();
        for i in [1, 2] {
            warm.correct_pose(
                i,
                crate::Point { x: 28., y: 100. },
                vec![
                    crate::Point { x: 28., y: 0. },
                    crate::Point { x: 28., y: -42. },
                ],
                0.,
            )
            .unwrap();
        }
        check(&mut warm);
        rejoin(&mut warm, 2);
        rejoin(&mut warm, 1);
        let mut cold = warm.clone();
        let actions: Vec<_> = warm
            .snakes
            .iter()
            .map(|s| s.alive.then_some(crate::Action::default()))
            .collect();
        warm.step(&actions).unwrap();
        cold.step(&actions).unwrap();
        assert_eq!(warm.deaths(), cold.deaths());
        assert!(!warm.snakes[0].alive);
        assert_eq!(warm.snakes[1].kills, 1);
        assert_eq!(warm.snakes[2].kills, 0);
        assert_eq!(
            serde_json::to_vec(&warm.checkpoint().unwrap()).unwrap(),
            serde_json::to_vec(&cold.checkpoint().unwrap()).unwrap()
        );
    }

    #[test]
    fn collision_join_order_drops_cached_slots_on_restore_and_clone_from() {
        let small = Simulator::new(WorldOptions {
            seed: 19,
            config: Config::sandbox(),
            population: InitialPopulation::Placed {
                ids: vec!["only".into()],
                placement: Placement::default(),
            },
        })
        .unwrap();
        let bytes = serde_json::to_vec(&small.checkpoint().unwrap()).unwrap();
        let mut restored = world();
        check(&mut restored);
        restored
            .restore(&bytes, crate::RestoreOptions::default())
            .unwrap();
        assert!(restored.buffers.order.next.is_none());
        check(&mut restored);
        let mut cloned = world();
        check(&mut cloned);
        cloned.clone_from(&restored);
        assert!(cloned.buffers.order.next.is_none());
        check(&mut cloned);
    }
}
