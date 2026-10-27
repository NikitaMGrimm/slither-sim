//! Synthetic core replay: official/Battledome presets with occupied seats leaving.
use crate::{Action, Config, EventRecording, InitialPopulation, Placement, Simulator, WorldOptions};
use std::{hint::black_box, io::{self, Write}};
mod tick_counter;

pub fn run(case: &str, trace: bool) {
    let (count, remaining, batch, every, mut config) = match case {
        "official500-shrink" => (500, 100, 4, 12, Config::official(500)),
        "battledome80-shrink" => (80, 16, 1, 24, Config::battledome()),
        _ => panic!("unknown shrink case"),
    };
    // Start at the preset's occupied-seat radius, before ecology generation.
    // The time-zero sample only initializes that state; all subsequent shrink
    // comes from the normal controller as occupied slots are retired.
    config.radius_updates = vec![crate::RadiusUpdate { time: 0., radius: config.seat_radius(count) }];
    let mut world = Simulator::new(WorldOptions {
        seed: 47, config,
        population: InitialPopulation::Placed {
            ids: (0..count).map(|slot| slot.to_string()).collect(),
            placement: Placement::TestRing,
        },
    }).unwrap();
    world.record_events(EventRecording { pickups: true, food_births: true });
    let mut counter = tick_counter::Counter::from_env();
    let metadata_path = std::env::var_os("SHRINK_METADATA");
    let mut metadata = Vec::new();
    let mut output = io::BufWriter::new(io::stdout().lock());
    if trace {
        serde_json::to_writer(&mut output, &serde_json::json!({"kind":"initial", "case":case, "checkpoint":world.checkpoint().unwrap()})).unwrap();
        output.write_all(b"\n").unwrap();
    }
    let mut next_retire = 0;
    for tick in 0..3200 {
        if tick >= 96 && (tick-96)%every == 0 {
            for _ in 0..batch {
                if next_retire < count-remaining {
                    world.retire(next_retire).unwrap();
                    next_retire += 1;
                }
            }
        }
        let radius = world.radius();
        let food_len = world.food().len();
        let points = metadata_path.as_ref().map(|_| world.food().iter().map(|f| f.point()).collect::<Vec<_>>());
        let actions: Vec<_> = world.snakes().iter().enumerate().map(|(slot, snake)| snake.alive.then(|| Action {
            heading: snake.head.y.atan2(snake.head.x) + std::f64::consts::FRAC_PI_2,
            boost: (tick/72+slot)%5==0,
        })).collect();
        let start = counter.as_ref().map(tick_counter::Counter::read);
        world.step(&actions).unwrap();
        let end = counter.as_ref().map(tick_counter::Counter::read);
        let alive = world.snakes().iter().filter(|snake| snake.alive).count();
        assert!(alive>0,"fixture lost every eater at tick {tick}");
        if let Some(counter)=&mut counter { counter.record(tick,start.unwrap(),end.unwrap(),alive); }
        if let Some(points)=points {
            let changed = world.radius()!=radius;
            let retention = (world.config().food_only_inside_pit && changed)
                || (world.config().dynamic_radius && world.radius()<radius);
            let culled = if retention {
                let edge = world.food_radius();
                points.iter().filter(|point| !point.distance_lt(crate::Point::default(),edge)).count()
            } else {0};
            metadata.push(serde_json::json!({"tick":tick,"radiusBefore":radius,"radiusAfter":world.radius(),"foodBefore":food_len,"foodAfter":world.food().len(),"culled":culled,"occupied":world.occupied(),"alive":alive}));
        }
        if trace {
            let pickups: Vec<_> = world.pickup_events().unwrap_or(&[]).iter().map(|event| {
                let item = match &event.item {
                    crate::PickupItem::Food(food) => serde_json::json!({"kind":"food","food":food}),
                    crate::PickupItem::Prey(prey) => serde_json::json!({"kind":"prey","prey":prey}),
                };
                serde_json::json!({"eater":event.eater,"item":item})
            }).collect();
            let births: Vec<_> = world.food_spawn_events().unwrap_or(&[]).iter()
                .map(|event| serde_json::json!({"source":event.source,"food":event.food})).collect();
            serde_json::to_writer(&mut output,&serde_json::json!({"kind":"tick","tick":tick,
                "checkpoint":world.checkpoint().unwrap(),"deaths":world.deaths(),
                "pickups":pickups,"births":births})).unwrap();
            output.write_all(b"\n").unwrap();
        }
        black_box(world.food().len());
    }
    assert_eq!(next_retire,count-remaining);
    assert_eq!(world.occupied(),remaining);
    let endpoint=serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
    let checksum=endpoint.iter().fold(0xcbf2_9ce4_8422_2325_u64,|h,&b|(h^u64::from(b)).wrapping_mul(0x100_0000_01b3));
    let summary=serde_json::json!({"kind":"report","case":case,"ticks":3200,"initialSeats":count,
        "retired":next_retire,"occupied":world.occupied(),"alive":world.snakes().iter().filter(|s|s.alive).count(),
        "radius":world.radius(),"food":world.food().len(),"endpointChecksum":checksum});
    if trace {serde_json::to_writer(&mut output,&summary).unwrap();output.write_all(b"\n").unwrap();}
    output.flush().unwrap();
    if let Some(path)=metadata_path {std::fs::write(path,serde_json::to_vec(&metadata).unwrap()).unwrap();}
    eprintln!("{summary}");
}
