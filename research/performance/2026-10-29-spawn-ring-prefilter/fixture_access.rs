//! Research-only child of client::simulation; no production API is added.
use super::*;

impl RawSimulator {
    pub(crate) fn study_state(&self) -> serde_json::Value {
        serde_json::json!({
            "world": self.backend.world.checkpoint().unwrap(),
            "spawnRandom": format!("{:?}", self.backend.spawn_random),
            "latencyRandom": format!("{:?}", self.backend.random),
            "foodEpoch": self.backend.food_epoch,
            "nextMinimap": self.backend.next_minimap,
            "nextLeaderboard": self.backend.next_leaderboard,
            "living": self.backend.world.living(),
        })
    }

    pub(crate) fn study_flags(&self) -> [u64; 3] {
        [self.backend.food_epoch, self.backend.next_minimap,
         self.backend.pit_markers.as_ref().map_or(0, |m| m.radius().to_bits())]
    }

    pub(crate) fn study_fences(&mut self, blocked: bool) {
        // Explicitly synthetic stress: seven replay rings cover the shipped
        // spawn band. Hold only these poses outside the counted interval.
        for slot in 1..8 {
            if !blocked {
                self.backend.world.retire(slot).unwrap();
                continue;
            }
            let radius = 2600. + slot as f64 * 125.;
            let body: Vec<_> = (0..=96).map(|i| {
                let angle = TAU * i as f64 / 96.;
                Point { x: radius * angle.cos(), y: radius * angle.sin() }
            }).collect();
            self.backend.world.import_life(slot, crate::ReplayLife {
                head: body[0], speed: crate::InitialSpeed::Infer(crate::physics::base_speed(body.len())),
                body, heading: std::f64::consts::FRAC_PI_2, fam: 0., kills: 0, alive: true,
            }).unwrap();
        }
    }

    pub(crate) fn study_observer(&mut self) {
        self.backend.world.correct_pose(0, Point::default(), vec![Point::default(); 26], 0.).unwrap();
        self.backend.slots[0].control = Action { heading: 0., boost: true };
    }

    pub(crate) fn study_viewers(&mut self, legacy: bool, connect: bool) {
        // Separate synthetic no-backlog and shrinking-marker controls. Hold 80
        // live viewers apart inside the pit; never use these poses as capacity
        // or naturally grown-body evidence.
        for slot in 0..self.backend.slots.len() {
            let head=Point { x:(slot%10) as f64*140.-630., y:(slot/10) as f64*140.-490. };
            if connect {
                let b=&mut self.backend;
                b.world.retire(slot).unwrap();
                b.world.respawn(slot,head,0.,26).unwrap();
                b.slots[slot].living=None;b.slots[slot].life+=1;
                b.slots[slot].control=Action { heading:0.,boost:true };
                b.connect(slot).unwrap();
                if legacy { self.protocol13(slot); }
            }
            let len=self.backend.world.snakes()[slot].body.len();
            self.backend.world.correct_pose(slot,head,vec![head;len],0.).unwrap();
        }
    }
}
