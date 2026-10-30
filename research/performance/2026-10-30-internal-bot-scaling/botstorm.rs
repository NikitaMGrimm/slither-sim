// Appended to the frozen profile.rs helpers by build_botstorm.py; no src/ edits.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StormSample {
    #[serde(flatten)]
    base: Sample,
    deaths: usize,
    center_cpu: f64,
    center_wall: f64,
}

impl Scene {
    fn storm_new(n: usize) -> Self {
        let radius = 32400.0 * (n as f64 / 12000.0).sqrt();
        let mut config = Config::official(n);
        config.radius = radius;
        config.dynamic_radius = false;
        config.size_to_population = false;
        // Overriding radius alone does not rescale explicit ecology targets.
        // These are the unchanged official_food/official_prey density formulas
        // in src/simulator.rs, applied at this fixed comparison radius.
        let sectors = std::f64::consts::PI * radius * radius / (300.0 * 300.0);
        config.food_count = (7.0 * sectors) as usize;
        config.ambient_replenish_per_second = 0.1024 * sectors;
        config.prey.count = (std::f64::consts::PI * radius * radius / 10_000_000.0)
            .round()
            .max(1.0) as usize;
        let world = Simulator::new(WorldOptions {
            seed: 7,
            config,
            population: InitialPopulation::Placed {
                ids: (0..n).map(|i| i.to_string()).collect(),
                placement: Placement::Admitted {
                    attempts: 64.try_into().unwrap(),
                    preferred_clearance: 600.0,
                },
            },
        })
        .unwrap();
        let meta = Meta {
            random: 7 ^ 0x626f7473,
            tick: 0,
            retry: vec![0; n],
            actions: world
                .snakes()
                .iter()
                .map(|snake| Some(Action { heading: snake.heading, boost: false }))
                .collect(),
        };
        Self::prepare(world, meta, false)
    }

    fn storm_tick(&mut self, profiled: bool) -> StormSample {
        let total = Instant::now();
        let mut base = Sample { tick: self.meta.tick, ..Default::default() };
        let wall = Instant::now();
        let c = cpu(3);
        self.admission(&mut base);
        base.admission_cpu = cpu(3) - c;
        base.admission_wall = wall.elapsed().as_secs_f64();
        let wall = Instant::now();
        let c = cpu(3);
        for (slot, (snake, action)) in self.world.snakes().iter().zip(&mut self.meta.actions).enumerate() {
            if snake.alive && (self.meta.tick + slot as u64).is_multiple_of(6) {
                *action = Some(Action { heading: (-snake.head.y).atan2(-snake.head.x), boost: false });
                base.decisions += 1;
            }
        }
        let center_cpu = cpu(3) - c;
        let center_wall = wall.elapsed().as_secs_f64();
        let deaths = self.world.deaths().len();
        let wall = Instant::now();
        let c = cpu(3);
        if profiled {
            let mut profile = StepProfile::default();
            self.world.step_profiled(&self.meta.actions, &mut profile).unwrap();
            base.profile = Some(profile);
        } else {
            self.world.step(&self.meta.actions).unwrap();
        }
        base.world_cpu = cpu(3) - c;
        base.world_wall = wall.elapsed().as_secs_f64();
        self.meta.tick += 1;
        base.total_wall = total.elapsed().as_secs_f64();
        StormSample { base, deaths: self.world.deaths().len() - deaths, center_cpu, center_wall }
    }
}

fn main() {
    pin(0);
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new(&args[2]);
    assert_eq!(fs::read_dir("/proc/self/task").unwrap().count(), 1);
    if args[1] == "views" || args[1] == "grid" {
        // Read-only workload census, never the center-seeking policy path.
        let mut scene = Scene::load(root, false);
        let before = serde_json::to_vec(&scene.world.checkpoint().unwrap()).unwrap();
        let mut food = Vec::new();
        let mut prey = Vec::new();
        let mut opponents = Vec::new();
        let mut anchors = Vec::new();
        let mut lo = [i64::MAX; 2];
        let mut hi = [i64::MIN; 2];
        let mut area = 0usize;
        let mut regular = true;
        let mut circles = 0usize;
        for snake in scene.world.snakes().iter().filter(|s| s.alive) {
            let (center, radius) = slither_sim::physics::pickup_circle(
                snake.head, snake.heading, snake.speed, snake.segments);
            regular &= radius >= 0.0 && center.x.abs() + radius <= 1048576.0
                && center.y.abs() + radius <= 1048576.0;
            let lower = [((center.x-radius-1e-9)/128.0).floor() as i64,
                         ((center.y-radius-1e-9)/128.0).floor() as i64];
            let upper = [((center.x+radius+1e-9)/128.0).floor() as i64,
                         ((center.y+radius+1e-9)/128.0).floor() as i64];
            for axis in 0..2 { lo[axis]=lo[axis].min(lower[axis]);hi[axis]=hi[axis].max(upper[axis]); }
            area=area.saturating_add(((upper[0]-lower[0]+1)*(upper[1]-lower[1]+1)) as usize);
            circles+=1;
        }
        let cells = if circles==0 { 0 } else { ((hi[0]-lo[0]+1)*(hi[1]-lo[1]+1)) as usize };
        let food_limit = scene.world.food().len().saturating_mul(4).max(4096);
        let phases = if args[1] == "views" { 6 } else { 0 };
        for phase in 0..phases {
            scene.world.observe_selected_into(&scene.masks[phase], &mut scene.observations[phase]).unwrap();
            for observation in scene.observations[phase].iter().flatten() {
                food.push(observation.food.len());
                prey.push(observation.prey.len());
                opponents.push(observation.opponents.len());
                anchors.push(observation.opponents.iter().map(|snake|snake.body.len()).sum::<usize>());
            }
        }
        let summarize = |mut values:Vec<usize>| {
            values.sort_unstable();
            let q=|fraction:f64|values.get(((values.len().saturating_sub(1)) as f64*fraction).ceil() as usize).copied();
            json!({"n":values.len(),"mean":values.iter().sum::<usize>() as f64/values.len().max(1) as f64,
                   "p50":q(0.5),"p90":q(0.9),"p99":q(0.99),"max":q(1.0)})
        };
        assert_eq!(before,serde_json::to_vec(&scene.world.checkpoint().unwrap()).unwrap());
        fs::write(&args[3],serde_json::to_vec_pretty(&json!({"food":summarize(food),"prey":summarize(prey),
            "opponents":summarize(opponents),"opponentAnchors":summarize(anchors),
            "pickupGridSnapshot":{"circles":circles,"cellSize":128,"rectangleCells":cells,
                "totalCoverCells":area,"fixedCellLimit":1048576,"foodRelativeCellLimit":food_limit,
                "regularGeometry":regular,
                "fitsDenseGridGuards":regular && cells<=(1048576usize).min(food_limit) && area<=1048576,
                "scope":"read-only checkpoint pickup circles; actual step builds after movement and deaths"},
            "scope":if args[1]=="views" { "read-only checkpoint census, all six selected masks; no policy decisions or physics" }
                else { "read-only checkpoint pickup-grid guards; no observations, decisions or physics" }})).unwrap()).unwrap();
    } else if args[1] == "warm" {
        fs::create_dir_all(root).unwrap();
        let n = args[3].parse().unwrap();
        let construction = Instant::now();
        let construction_cpu = cpu(3);
        let mut scene = Scene::storm_new(n);
        let began = Instant::now();
        eprintln!("{}", json!({"kind":"initial", "census":census(&scene.world),
            "constructionSeconds":construction.elapsed().as_secs_f64(),
            "constructionCpuSeconds":cpu(3)-construction_cpu}));
        let mut deaths = 0;
        let mut admitted = 0;
        let mut refused = 0;
        for _ in 0..257143 {
            let sample = scene.storm_tick(false);
            deaths += sample.deaths;
            admitted += sample.base.admitted;
            refused += sample.base.refused;
            if scene.meta.tick.is_multiple_of(1000) {
                eprintln!("{}", json!({"kind":"warm","tick":scene.meta.tick,
                    "census":census(&scene.world), "deathsPerVirtualSecond":deaths as f64/7.0,
                    "admissionsPerVirtualSecond":admitted as f64/7.0,"refused":refused,"last":sample}));
                deaths = 0; admitted = 0; refused = 0;
                if root.join("STOP_WARM").exists() || began.elapsed().as_secs() > 7200 { break; }
            }
        }
        scene.save(root);
        eprintln!("{}",json!({"kind":"ready","tick":scene.meta.tick,"census":census(&scene.world)}));
    } else if args[1] == "measure" {
        let mut scene = Scene::load(root, false);
        let ticks: usize = args[3].parse().unwrap();
        let profiled = args[4] == "profile";
        let output = Path::new(&args[5]);
        let initial = census(&scene.world);
        let wall = Instant::now();
        let c = cpu(3);
        let samples: Vec<_> = (0..ticks).map(|_| scene.storm_tick(profiled)).collect();
        let process_cpu = cpu(3) - c;
        let elapsed = wall.elapsed().as_secs_f64();
        let final_census = census(&scene.world);
        let final_state = serde_json::to_vec(&scene.world.checkpoint().unwrap()).unwrap();
        fs::write(output, serde_json::to_vec(&json!({"ticks":ticks,"profiled":profiled,
            "initial":initial,"final":final_census,"samples":samples,
            "wallSeconds":elapsed,"processCpuSeconds":process_cpu,
            "finalStateSha256":hex(Sha256::digest(final_state))})).unwrap()).unwrap();
    } else { panic!("unknown botstorm mode"); }
}
