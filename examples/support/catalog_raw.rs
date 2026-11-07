//! Raw recipients: control generation, draining and diagnostics are untimed.
use super::*;
use slither_sim::client::{
    DeliveryProfile, SimulationConfig,
    simulation::{RawReplay, RawWorld},
};

fn fixture(args: &Args, kind: EngineWorld) -> Result<RawWorld, String> {
    let config = kind.config();
    let mut world = runtime::ring(SEED, config.clone(), 500)?;
    for slot in 0..500 {
        let head = Point {
            x: ((slot % 25) as f64 - 12.) * config.radius * 0.055,
            y: ((slot / 25) as f64 - 9.5) * config.radius * 0.055,
        };
        let body = (0..64 + slot % 64)
            .map(|i| {
                let a = i as f64 * TAU / 48.;
                Point {
                    x: head.x - 200. + 200. * a.cos(),
                    y: head.y + 200. * a.sin(),
                }
            })
            .collect::<Vec<_>>();
        world
            .correct_mass(slot, body.clone(), 0.25)
            .map_err(|e| e.to_string())?;
        world
            .correct_pose(slot, head, body, 0.)
            .map_err(|e| e.to_string())?;
    }
    runtime::record_events(&mut world);
    let mut sim = RawWorld::new(
        SEED,
        SimulationConfig {
            agents: 500,
            world: config,
            delivery: DeliveryProfile::Official,
            ..Default::default()
        },
        world,
    )?;
    views(&mut sim, args)?;
    Ok(sim)
}

/// Host-default view splitting after the serial setup broadcast. Workers are
/// started and joined outside the operation timer, once per epoch.
#[cfg(all(slither_raw_view_threads, feature = "parallel"))]
fn views(sim: &mut RawWorld, args: &Args) -> Result<(), String> {
    sim.set_view_threads(args.view_threads, None, None)
}

/// Older runtimes and serial builds prepare every view on this thread.
#[cfg(not(all(slither_raw_view_threads, feature = "parallel")))]
fn views(_: &mut RawWorld, args: &Args) -> Result<(), String> {
    if args.view_threads == 1 {
        Ok(())
    } else {
        Err("this runtime has no raw view threads".into())
    }
}

fn drain(sim: &mut RawWorld, trace: bool, replay: &mut RawReplay) -> Result<usize, String> {
    if trace {
        let start = replay.frames.len();
        sim.record(replay)?;
        Ok(replay.frames[start..].iter().map(|f| f.3.len()).sum())
    } else {
        Ok(sim.drain()?.iter().map(|f| f.2.len()).sum())
    }
}

/// Every recipient must stay connected and alive, or later ticks measure less work.
fn complete(sim: &RawWorld) -> Result<(), String> {
    if sim.composition().0 != 500 || sim.world().living() != 500 {
        return Err("raw fixture lost recipients or living snakes".into());
    }
    Ok(())
}

fn warm(sim: &mut RawWorld, trace: bool, replay: &mut RawReplay) -> Result<(), String> {
    drain(sim, trace, replay)?;
    for _ in 0..144 {
        sim.tick()?;
        drain(sim, trace, replay)?;
    }
    complete(sim)
}

pub(super) fn run(args: &Args, kind: EngineWorld, out: &mut impl Write) -> Result<Value, String> {
    if args.profile {
        return Err("raw host phase profiles are unavailable".into());
    }
    let mut sim = fixture(args, kind)?;
    let mut replay = RawReplay::default();
    warm(&mut sim, args.trace, &mut replay)?;
    let initial = sim.composition();
    let controls = vec![
        Some(Action {
            heading: 0.,
            boost: false
        });
        500
    ];
    let mut meter = Meter::default();
    let mut bytes = 0;
    for iteration in 0..args.iterations {
        if iteration > 0 && iteration % kind.epoch_ticks() == 0 {
            if args.trace {
                sim.finish(&mut replay)?;
                line(
                    out,
                    json!({"kind":"epoch","iteration":iteration,"replay":replay}),
                )?;
                replay = RawReplay::default();
            }
            sim = fixture(args, kind)?;
            warm(&mut sim, args.trace, &mut replay)?;
        }
        sim.controls(&controls)?;
        meter.operation(args.trace, || sim.tick())?;
        bytes += drain(&mut sim, args.trace, &mut replay)?;
        complete(&sim)?;
    }
    if args.trace {
        sim.finish(&mut replay)?;
        line(out, json!({"kind":"end","case":args.case,"replay":replay}))?;
    }
    if bytes == 0 {
        return Err("raw fixture emitted no bytes".into());
    }
    Ok(report(
        args,
        meter,
        "raw-tick",
        json!({"recipients":500,"living":500,
        "initialKnownSnakes":initial.1,"finalKnownSnakes":sim.composition().1,
        "wireBytes":bytes,"warmupTicks":144,"epochTicks":kind.epoch_ticks()}),
    ))
}
