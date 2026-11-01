//! Bounded host-only scalar truth. See docs/raw-server.md for the unstable format.
use crate::Simulator;
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    mem::size_of,
    net::SocketAddr,
    path::Path,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) const DEFAULT_SECONDS: f64 = 60.;
const MAX_SECONDS: f64 = 180.;
const MAX_BYTES: usize = 256 << 20;
const MAX_TICKS: usize = 1_000_000;

pub(crate) fn duration(seconds: f64) -> Result<Duration, String> {
    if !seconds.is_finite() || seconds <= 0. || seconds > MAX_SECONDS {
        return Err("--latency-seconds must be finite, positive and at most 180".into());
    }
    let duration = Duration::try_from_secs_f64(seconds).map_err(|e| e.to_string())?;
    if duration.is_zero() {
        return Err("--latency-seconds must be at least one nanosecond".into());
    }
    Ok(duration)
}

pub(crate) fn loopback(bind: SocketAddr) -> Result<(), String> {
    if !bind.ip().is_loopback() {
        return Err("--latency-truth requires a loopback bind".into());
    }
    Ok(())
}

pub(crate) fn validate(
    path: &Path,
    bind: SocketAddr,
    seconds: Duration,
    dt: f64,
    slots: usize,
) -> Result<(), String> {
    loopback(bind)?;
    bounds(seconds, dt, slots)?;
    if path.try_exists().map_err(|e| e.to_string())? {
        return Err("--latency-truth needs a fresh file".into());
    }
    Ok(())
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn QueryPerformanceCounter(value: *mut i64) -> i32;
    fn QueryPerformanceFrequency(value: *mut i64) -> i32;
}

fn qpc_frequency() -> Result<Option<i64>, &'static str> {
    #[cfg(windows)]
    {
        let mut value = 0;
        // SAFETY: the API writes one initialized, live i64 and retains no pointer.
        if unsafe { QueryPerformanceFrequency(&mut value) } == 0 || value <= 0 {
            return Err("QueryPerformanceFrequency failed");
        }
        Ok(Some(value))
    }
    #[cfg(not(windows))]
    Ok(None)
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Stamp {
    monotonic_ns: u128,
    unix_ns: u128,
    qpc: Option<i64>,
}

impl Stamp {
    pub(crate) fn now(epoch: Instant) -> Result<Self, &'static str> {
        // Preserve the raw readings: UNIX time can jump. Instant supplies spacing,
        // and QPC, when present, is the common Windows cross-process counter.
        let monotonic_ns = epoch.elapsed().as_nanos();
        let unix_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "SystemTime is before the UNIX epoch")?
            .as_nanos();
        #[cfg(windows)]
        let qpc = {
            let mut value = 0;
            // SAFETY: as for QueryPerformanceFrequency above.
            if unsafe { QueryPerformanceCounter(&mut value) } == 0 {
                return Err("QueryPerformanceCounter failed");
            }
            Some(value)
        };
        #[cfg(not(windows))]
        let qpc = None;
        Ok(Self {
            monotonic_ns,
            unix_ns,
            qpc,
        })
    }
}

#[derive(Clone, Copy, Default)]
struct Tick {
    tick: u64,
    before: Stamp,
    after: Stamp,
    simulation_time: f64,
    wall_radius: f64,
    pit_radius: Option<f64>,
}

// A tuple on export: no body, food, names, controls or encoded packets.
#[derive(Clone, Copy, Default, Serialize)]
struct Head(u16, u64, bool, f64, f64, f64, f64, bool);

struct Buffer {
    ticks: Vec<Tick>,
    heads: Vec<Head>,
    slots: usize,
    len: usize,
    duration_reached: bool,
    loss: Option<&'static str>,
}

fn bounds(seconds: Duration, dt: f64, slots: usize) -> Result<(usize, usize), String> {
    duration(seconds.as_secs_f64())?;
    if !dt.is_finite() || dt <= 0. || !(1..=u16::MAX as usize).contains(&slots) {
        return Err("Invalid latency truth tick interval or slot count".into());
    }
    // Two spare ticks cover rounding and a tick crossing the finite boundary.
    // Adaptive host ticks are never shorter than this configured interval.
    let ticks = (seconds.as_secs_f64() / dt).ceil();
    if ticks > (MAX_TICKS - 2) as f64 {
        return Err("Latency truth exceeds the 1000000 tick cap".into());
    }
    let ticks = ticks as usize + 2;
    let bytes = slots
        .checked_mul(size_of::<Head>())
        .and_then(|heads| heads.checked_add(size_of::<Tick>()))
        .and_then(|row| row.checked_mul(ticks))
        .filter(|&bytes| bytes <= MAX_BYTES)
        .ok_or("Latency truth exceeds the 256 MiB scalar buffer cap")?;
    Ok((ticks, bytes))
}

fn touched<T: Copy + Default>(len: usize) -> Result<Vec<T>, String> {
    let mut values = Vec::new();
    values.try_reserve_exact(len).map_err(|e| e.to_string())?;
    values.resize(len, T::default());
    // Force real writes to every element before host startup, even when a zero
    // allocation could otherwise defer physical page commitment until recording.
    for value in &mut values {
        // SAFETY: each pointer is unique, aligned and points to initialized T.
        unsafe { std::ptr::write_volatile(value, T::default()) };
    }
    Ok(values)
}

impl Buffer {
    fn new(seconds: Duration, dt: f64, slots: usize) -> Result<Self, String> {
        let (ticks, _) = bounds(seconds, dt, slots)?;
        Ok(Self {
            ticks: touched(ticks)?,
            heads: touched(ticks * slots)?,
            slots,
            len: 0,
            duration_reached: false,
            loss: None,
        })
    }

    fn record(
        &mut self,
        tick: u64,
        before: Stamp,
        after: Stamp,
        world: &Simulator,
        lives: impl ExactSizeIterator<Item = u64>,
    ) -> Result<(), &'static str> {
        let failure = if self.len == self.ticks.len() {
            Some("scalar buffer exhausted")
        } else if world.snakes().len() != self.slots || lives.len() != self.slots {
            Some("slot count changed")
        } else if tick != self.len as u64 + 1
            || after.monotonic_ns < before.monotonic_ns
            || self.len > 0 && before.monotonic_ns < self.ticks[self.len - 1].after.monotonic_ns
        {
            Some("tick or monotonic clock discontinuity")
        } else {
            None
        };
        if let Some(reason) = self.loss.or(failure) {
            self.loss = Some(reason);
            return Err(reason);
        }
        let start = self.len * self.slots;
        for (slot, (snake, life)) in world.snakes().iter().zip(lives).enumerate() {
            self.heads[start + slot] = Head(
                slot as u16,
                life,
                snake.alive,
                snake.head.x,
                snake.head.y,
                snake.heading,
                snake.speed,
                world.is_boosting(slot),
            );
        }
        self.ticks[self.len] = Tick {
            tick,
            before,
            after,
            simulation_time: world.time(),
            wall_radius: world.radius(),
            pit_radius: world.pit_radius(),
        };
        self.len += 1;
        Ok(())
    }
}

pub(crate) struct Recorder {
    buffer: Option<Buffer>,
    returned: Sender<Buffer>,
    seconds: Duration,
    epoch: Instant,
}

impl Recorder {
    pub(crate) fn start(&mut self, epoch: Instant) {
        self.epoch = epoch;
    }

    pub(crate) fn finished(&mut self, now: Instant) -> bool {
        if now.saturating_duration_since(self.epoch) < self.seconds {
            return false;
        }
        self.buffer.as_mut().unwrap().duration_reached = true;
        true
    }

    pub(crate) fn stamp(&mut self) -> Result<Stamp, &'static str> {
        Stamp::now(self.epoch).inspect_err(|&reason| {
            self.buffer.as_mut().unwrap().loss = Some(reason);
        })
    }

    pub(crate) fn record(
        &mut self,
        tick: u64,
        before: Stamp,
        after: Stamp,
        world: &Simulator,
        lives: impl ExactSizeIterator<Item = u64>,
    ) -> Result<(), &'static str> {
        self.buffer
            .as_mut()
            .unwrap()
            .record(tick, before, after, world, lives)
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        // One handoff at teardown, including unwind. Never used in a tick.
        if let Some(buffer) = self.buffer.take() {
            let _ = self.returned.send(buffer);
        }
    }
}

pub(crate) struct Export {
    file: File,
    returned: Receiver<Buffer>,
}

fn json_line(output: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    output.write_all(b"\n")
}

impl Export {
    pub(crate) fn create(
        path: &Path,
        seconds: Duration,
        dt: f64,
        slots: usize,
    ) -> Result<(Self, Recorder), String> {
        let buffer = Buffer::new(seconds, dt, slots)?;
        let frequency = qpc_frequency().map_err(str::to_owned)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // A crash or failed startup leaves an explicitly incomplete prefix.
        json_line(
            &mut file,
            &serde_json::json!({
                "type": "metadata", "schema": "host-latency-truth-v1", "complete": false,
                "clock_source": if cfg!(windows) { "system-time-unix+instant+windows-qpc" }
                                else { "system-time-unix+instant" },
                "qpc_frequency": frequency.map(|n| n.to_string()),
                "duration_seconds": seconds.as_secs_f64(), "slot_count": slots,
                "tick_capacity": buffer.ticks.len(), "configured_dt": dt,
                "buffer_bytes": bounds(seconds, dt, slots)?.1,
            }),
        )
        .and_then(|()| file.flush())
        .map_err(|e| e.to_string())?;
        let (returned, receiver) = mpsc::channel();
        Ok((
            Self {
                file,
                returned: receiver,
            },
            Recorder {
                buffer: Some(buffer),
                returned,
                seconds,
                epoch: Instant::now(),
            },
        ))
    }

    /// Only called after all owned host threads have joined.
    pub(crate) fn finish(self, host_ok: bool) -> Result<(), String> {
        let buffer = self
            .returned
            .try_recv()
            .map_err(|_| "Latency truth buffer was not returned")?;
        let mut output = BufWriter::new(self.file);
        write_buffer(&mut output, &buffer, host_ok)
            .and_then(|()| output.flush())
            .map_err(|e| format!("Latency truth export incomplete: {e}"))?;
        if !host_ok || !buffer.duration_reached || buffer.loss.is_some() {
            return Err(format!(
                "Latency truth incomplete: {}",
                reason(&buffer, host_ok)
            ));
        }
        Ok(())
    }
}

fn reason(buffer: &Buffer, host_ok: bool) -> &'static str {
    buffer.loss.unwrap_or(if !host_ok {
        "host failure"
    } else if !buffer.duration_reached {
        "stopped before duration"
    } else {
        "duration reached"
    })
}

fn write_buffer(output: &mut impl Write, buffer: &Buffer, host_ok: bool) -> io::Result<()> {
    #[derive(Serialize)]
    struct Row<'a> {
        r#type: &'static str,
        tick: u64,
        unix_before_ns: String,
        unix_after_ns: String,
        monotonic_before_ns: String,
        monotonic_after_ns: String,
        qpc_before: Option<String>,
        qpc_after: Option<String>,
        simulation_time: f64,
        wall_radius: f64,
        pit_radius: Option<f64>,
        heads: &'a [Head],
    }
    for (i, tick) in buffer.ticks[..buffer.len].iter().enumerate() {
        json_line(
            output,
            &Row {
                r#type: "tick",
                tick: tick.tick,
                unix_before_ns: tick.before.unix_ns.to_string(),
                unix_after_ns: tick.after.unix_ns.to_string(),
                monotonic_before_ns: tick.before.monotonic_ns.to_string(),
                monotonic_after_ns: tick.after.monotonic_ns.to_string(),
                qpc_before: tick.before.qpc.map(|n| n.to_string()),
                qpc_after: tick.after.qpc.map(|n| n.to_string()),
                simulation_time: tick.simulation_time,
                wall_radius: tick.wall_radius,
                pit_radius: tick.pit_radius,
                heads: &buffer.heads[i * buffer.slots..(i + 1) * buffer.slots],
            },
        )?;
    }
    json_line(
        output,
        &serde_json::json!({
            "type": "summary", "complete": host_ok && buffer.duration_reached && buffer.loss.is_none(),
            "reason": reason(buffer, host_ok), "ticks": buffer.len, "heads": buffer.len * buffer.slots,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, InitialPopulation, Placement, WorldOptions};
    use std::sync::atomic::{AtomicU64, Ordering};

    pub(crate) fn new_path() -> std::path::PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "slither-truth-{}-{}-{}.jsonl",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn world() -> Simulator {
        let mut world = Simulator::new(WorldOptions {
            config: Config::sandbox(),
            population: InitialPopulation::Placed {
                ids: vec!["0".into(), "1".into()],
                placement: Placement::TestRing,
            },
            ..WorldOptions::default()
        })
        .unwrap();
        world.retire(1).unwrap();
        world
    }

    fn stamp(n: u128) -> Stamp {
        Stamp {
            monotonic_ns: n,
            unix_ns: 1_800_000_000_000_000_000 + n,
            qpc: Some(n as i64),
        }
    }

    fn rows(buffer: &Buffer, host_ok: bool) -> Vec<serde_json::Value> {
        let mut output = Vec::new();
        write_buffer(&mut output, buffer, host_ok).unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn duration_and_byte_caps_are_checked_before_allocation() {
        for seconds in [0., -1., f64::NAN, f64::INFINITY, 180.001, 1e-12] {
            assert!(duration(seconds).is_err());
        }
        let seconds = duration(180.).unwrap();
        assert!(bounds(seconds, 0.007, 121).unwrap().1 < MAX_BYTES);
        assert!(bounds(seconds, 0.007, u16::MAX as usize).is_err());
        assert!(bounds(seconds, 1e-12, 1).is_err());
        for slots in [0, usize::MAX] {
            assert!(bounds(seconds, 0.007, slots).is_err());
        }
        for dt in [0., -1., f64::NAN] {
            assert!(bounds(seconds, dt, 1).is_err());
        }
    }

    #[test]
    fn raw_scalars_all_slots_and_lossless_clocks_export_after_recording() {
        let world = world();
        let slots = world.snakes().len();
        let mut buffer = Buffer::new(duration(0.1).unwrap(), 0.007, slots).unwrap();
        let pointers = (buffer.ticks.as_ptr(), buffer.heads.as_ptr());
        buffer
            .record(
                1,
                stamp(3),
                stamp(7),
                &world,
                (0..slots).map(|n| n as u64 + 4),
            )
            .unwrap();
        assert_eq!(pointers, (buffer.ticks.as_ptr(), buffer.heads.as_ptr()));
        buffer.duration_reached = true;
        let rows = rows(&buffer, true);
        let row = &rows[0];
        assert_eq!(row["unix_before_ns"], "1800000000000000003");
        assert_eq!(row["monotonic_after_ns"], "7");
        assert_eq!(row["qpc_before"], "3");
        assert_eq!(row["simulation_time"], world.time());
        assert_eq!(row["wall_radius"], world.radius());
        assert_eq!(row["heads"].as_array().unwrap().len(), slots);
        for (slot, snake) in world.snakes().iter().enumerate() {
            assert_eq!(
                row["heads"][slot],
                serde_json::json!([
                    slot,
                    slot + 4,
                    snake.alive,
                    snake.head.x,
                    snake.head.y,
                    snake.heading,
                    snake.speed,
                    world.is_boosting(slot)
                ])
            );
        }
        assert_eq!(rows[1]["complete"], true);
        assert_eq!(rows[1]["ticks"], 1);
        assert_eq!(rows[1]["heads"], slots);
    }

    #[test]
    fn overflow_retains_prefix_and_cannot_become_complete() {
        let world = world();
        let slots = world.snakes().len();
        let mut buffer = Buffer::new(duration(0.001).unwrap(), 1., slots).unwrap();
        for i in 0..buffer.ticks.len() {
            buffer
                .record(
                    i as u64 + 1,
                    stamp(i as u128 * 2),
                    stamp(i as u128 * 2 + 1),
                    &world,
                    (0..slots).map(|_| 0),
                )
                .unwrap();
        }
        let retained = buffer.len;
        assert_eq!(
            buffer.record(
                retained as u64 + 1,
                stamp(10),
                stamp(11),
                &world,
                (0..slots).map(|_| 1)
            ),
            Err("scalar buffer exhausted")
        );
        buffer.duration_reached = true;
        let rows = rows(&buffer, true);
        assert_eq!(rows.len(), retained + 1);
        assert_eq!(rows.last().unwrap()["complete"], false);
        assert_eq!(rows.last().unwrap()["reason"], "scalar buffer exhausted");
    }

    #[test]
    fn discontinuities_invalidate_instead_of_sampling() {
        let world = world();
        let slots = world.snakes().len();
        for (tick, before, after, count) in [(2, 0, 1, slots), (1, 2, 1, slots), (1, 0, 1, 0)] {
            let mut buffer = Buffer::new(duration(0.1).unwrap(), 0.007, slots).unwrap();
            assert!(
                buffer
                    .record(
                        tick,
                        stamp(before),
                        stamp(after),
                        &world,
                        (0..count).map(|_| 0)
                    )
                    .is_err()
            );
            assert_eq!(buffer.len, 0);
            assert_eq!(rows(&buffer, true)[0]["complete"], false);
        }
    }

    #[test]
    fn early_stop_host_failure_and_export_failure_are_incomplete() {
        let world = world();
        let slots = world.snakes().len();
        let mut buffer = Buffer::new(duration(0.1).unwrap(), 0.007, slots).unwrap();
        assert_eq!(rows(&buffer, true)[0]["reason"], "stopped before duration");
        buffer.duration_reached = true;
        assert_eq!(rows(&buffer, false)[0]["reason"], "host failure");
        let mut short = [0u8; 10];
        assert!(write_buffer(&mut short.as_mut_slice(), &buffer, true).is_err());
    }

    #[test]
    fn fresh_file_prefix_and_teardown_handoff() {
        let path = new_path();
        let (export, recorder) = Export::create(&path, duration(0.001).unwrap(), 0.007, 1).unwrap();
        assert!(Export::create(&path, duration(0.001).unwrap(), 0.007, 1).is_err());
        let prefix = std::fs::read_to_string(&path).unwrap();
        assert_eq!(prefix.lines().count(), 1);
        let metadata: serde_json::Value = serde_json::from_str(prefix.trim()).unwrap();
        assert_eq!(metadata["schema"], "host-latency-truth-v1");
        assert_eq!(metadata["complete"], false);
        assert!(
            metadata["clock_source"]
                .as_str()
                .unwrap()
                .starts_with("system-time-unix+instant")
        );
        drop(recorder);
        assert!(
            export
                .finish(true)
                .unwrap_err()
                .contains("stopped before duration")
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        let summary: serde_json::Value =
            serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(summary["complete"], false);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn clock_samples_supply_monotonic_spacing_and_optional_qpc() {
        let epoch = Instant::now();
        let before = Stamp::now(epoch).unwrap();
        let after = Stamp::now(epoch).unwrap();
        assert!(after.monotonic_ns >= before.monotonic_ns);
        assert!(before.unix_ns > 0);
        assert_eq!(before.qpc.is_some(), cfg!(windows));
        assert_eq!(qpc_frequency().unwrap().is_some(), cfg!(windows));
    }

    #[test]
    fn step_hook_preserves_world_and_uses_current_slot_life_on_wire_reuse() {
        use crate::client::simulation::{PlayerAppearance, RawSimulator, SimulationConfig};
        let config: SimulationConfig = serde_json::from_value(serde_json::json!({
            "agents": 2, "world": {"preset":"sandbox", "foodCount":0, "ambientReplenishPerSecond":0}
        }))
        .unwrap();
        let mut control = RawSimulator::new(7, config.clone(), 2).unwrap();
        let mut recorded = RawSimulator::new(7, config, 2).unwrap();
        let path = new_path();
        let (export, recorder) = Export::create(&path, duration(1.).unwrap(), 0.007, 2).unwrap();
        recorded.set_latency_truth(recorder);
        for raw in [&mut control, &mut recorded] {
            assert!(raw.connect(0, PlayerAppearance::default()).unwrap());
            raw.advance_tick_with(0, &mut |_| Ok(())).unwrap();
            raw.disconnect(0).unwrap();
            raw.advance_tick_with(0, &mut |_| Ok(())).unwrap();
            assert!(raw.connect(0, PlayerAppearance::default()).unwrap());
            raw.advance_tick_with(0, &mut |_| Ok(())).unwrap();
        }
        assert_eq!(
            serde_json::to_value(control.fixture_world().snakes()).unwrap(),
            serde_json::to_value(recorded.fixture_world().snakes()).unwrap()
        );
        drop(recorded);
        let buffer = export.returned.try_recv().unwrap();
        assert_eq!(buffer.len, 3);
        let rows = rows(&buffer, true);
        for (i, (life, alive)) in [(1, true), (1, false), (2, true)].into_iter().enumerate() {
            assert_eq!(rows[i]["heads"][0][0], 0);
            assert_eq!(rows[i]["heads"][0][1], life);
            assert_eq!(rows[i]["heads"][0][2], alive);
            assert_eq!(rows[i]["heads"][1][0], 1);
            assert_eq!(rows[i]["heads"][1][1], 0);
            assert_eq!(rows[i]["heads"][1][2], false);
        }
        drop(export);
        std::fs::remove_file(path).unwrap();
    }
}
