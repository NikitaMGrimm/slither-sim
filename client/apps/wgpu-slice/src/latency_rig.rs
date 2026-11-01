//! Bounded scalar copies of actual prepared pictures. Measurement builds only.
//! Clock and interpretation limits: client/docs/latency-rig.md.
use crate::{Result, latency};
use slither_client::presentation::{AnimatedClient, PreparedAccessorySnake};
use std::{
    cell::RefCell,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const CAPACITY: usize = 32_768;
const POSE_CAPACITY: usize = 262_144;
thread_local! { static JOURNAL: RefCell<Option<Journal>> = const { RefCell::new(None) }; }

pub fn unix_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos()
}

#[derive(Clone, Copy, Default)]
pub struct Head {
    pub id: u16,
    pub token: u64,
    pub own: bool,
    pub painter: [f64; 2],
    pub corrected: [f64; 2],
    pub packet: [f64; 2],
    pub heading: f64,
    pub speed: f64,
    pub boost: bool,
}
#[derive(Clone, Copy, Default)]
pub struct Sample {
    pub frame: u64,
    pub generation: u32,
    pub trace_ns: u64,
    pub unix_before: u128,
    pub unix_after: u128,
    pub zoom: f64,
    pub middle: bool,
    pub heads: [Option<Head>; 2],
}
struct Journal {
    selected: [Option<(u16, u64)>; 2],
    middle: bool,
    rows: Vec<Sample>,
    dropped: u64,
    clock_step_ns: u128,
    poses: Vec<PoseReceipt>,
    pose_dropped: u64,
}

#[derive(Clone, Copy, Default)]
struct PoseReceipt {
    generation: u32,
    message: u64,
    command: u32,
    id: u16,
    kind: u8,
}

pub fn start() {
    let mut clock_step_ns = u128::MAX;
    let mut previous = unix_ns();
    for _ in 0..10_000 {
        let next = unix_ns();
        if next > previous {
            clock_step_ns = clock_step_ns.min(next - previous);
        }
        previous = next;
    }
    let mut rows = vec![Sample::default(); CAPACITY];
    for row in &mut rows {
        row.frame = std::hint::black_box(1);
    }
    std::hint::black_box(&mut rows);
    rows.clear();
    let mut poses = vec![PoseReceipt::default(); POSE_CAPACITY];
    for pose in &mut poses {
        pose.message = std::hint::black_box(1);
    }
    poses.clear();
    JOURNAL.with_borrow_mut(|j| {
        assert!(j.is_none(), "one measurement per process");
        *j = Some(Journal {
            selected: [None; 2],
            middle: false,
            rows,
            dropped: 0,
            clock_step_ns: if clock_step_ns == u128::MAX {
                0
            } else {
                clock_step_ns
            },
            poses,
            pose_dropped: 0,
        });
    });
}
/// Wire message IDs join the original transport receipt and apply timestamps.
/// Position anchors and heading/speed reports are intentionally separate classes.
pub fn received(generation: u32, message: u64, bytes: &[u8], own: Option<u16>) -> Result<()> {
    JOURNAL.with_borrow_mut(|journal| {
        let Some(journal) = journal else {
            return Ok(());
        };
        let frame = slither_client::protocol::read_frame(
            bytes,
            slither_client::protocol::ElapsedHeader::Absent,
        )
        .map_err(|e| e.to_string())?;
        for command in frame.commands() {
            let Some(pose) = slither_client::protocol::latency::pose_packet(command) else {
                continue;
            };
            let Some(id) = pose.id.or(own) else {
                continue;
            };
            if journal.poses.len() == journal.poses.capacity() {
                journal.pose_dropped += 1;
            } else {
                journal.poses.push(PoseReceipt {
                    generation,
                    message,
                    command: command.index as u32,
                    id,
                    kind: pose.kind as u8,
                });
            }
        }
        Ok(())
    })
}
pub fn select(own: Option<(u16, u64)>, reference: Option<(u16, u64)>, middle: bool) {
    JOURNAL.with_borrow_mut(|j| {
        if let Some(j) = j {
            j.selected = [own, reference];
            j.middle = middle;
        }
    });
}
pub(crate) fn snapshot(
    frame: u64,
    prepared: &[PreparedAccessorySnake],
    client: &AnimatedClient,
) -> Option<Sample> {
    JOURNAL.with_borrow(|j| {
        let j = j.as_ref()?;
        let before = unix_ns();
        let trace_ns = latency::stamp();
        let after = unix_ns();
        let center = client
            .received()
            .setup
            .as_ref()
            .map_or(0., |s| s.grid_center);
        let heads = j.selected.map(|selected| {
            let (id, token) = selected?;
            let snake = &prepared
                .iter()
                .find(|p| p.snake.metrics.id == id && p.snake.token.serial() == token)?
                .snake;
            let m = snake.metrics;
            // Retained dead/replaced appearances cannot borrow a live packet's pose.
            if !m.alive || client.snake(id).is_none_or(|s| !s.metrics().alive) {
                return None;
            }
            let packet = client.received().snakes.get(&id)?;
            let first = snake.body_geometry.as_ref()?.samples.first()?;
            Some(Head {
                id,
                token,
                own: snake.own,
                painter: [f64::from(first.x) - center, f64::from(first.y) - center],
                corrected: [m.corrected_head.x - center, m.corrected_head.y - center],
                packet: [packet.head.x, packet.head.y],
                heading: m.corrected_heading,
                speed: m.speed_per_32ms / 0.032,
                boost: m.speed_per_32ms > m.boost_threshold_speed,
            })
        });
        Some(Sample {
            frame,
            generation: latency::current_generation(),
            trace_ns,
            unix_before: before,
            unix_after: after,
            zoom: client.camera().zoom(),
            middle: j.middle,
            heads,
        })
    })
}
pub(crate) fn commit(sample: Option<Sample>) {
    if let Some(sample) = sample {
        JOURNAL.with_borrow_mut(|j| {
            let j = j.as_mut().expect("attached");
            if j.rows.len() == j.rows.capacity() {
                j.dropped += 1;
            } else {
                j.rows.push(sample);
            }
        });
    }
}
pub fn finish(path: &Path, complete: bool) -> Result<()> {
    let j = JOURNAL
        .with_borrow_mut(Option::take)
        .ok_or("no rig recorder")?;
    let complete = complete && j.dropped == 0 && j.pose_dropped == 0;
    let mut poses = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.with_extension("poses.jsonl"))
            .map_err(|e| e.to_string())?,
    );
    writeln!(
        poses,
        "{{\"format\":\"client-latency-poses-v1\",\"complete\":{complete},\"dropped\":{}}}",
        j.pose_dropped
    )
    .map_err(|e| e.to_string())?;
    for p in &j.poses {
        writeln!(
            poses,
            "{{\"generation\":{},\"message\":\"{}\",\"command\":{},\"id\":{},\"kind\":{}}}",
            p.generation, p.message, p.command, p.id, p.kind
        )
        .map_err(|e| e.to_string())?;
    }
    writeln!(poses, "{{\"end\":true,\"records\":{}}}", j.poses.len()).map_err(|e| e.to_string())?;
    poses.flush().map_err(|e| e.to_string())?;
    let mut out = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?,
    );
    let result = (|| -> std::io::Result<()> {
        writeln!(
            out,
            "{{\"format\":\"client-latency-heads-v1\",\"clock_source\":\"system-time-unix+process-instant\",\"observed_min_positive_clock_step_ns\":\"{}\",\"complete\":{},\"dropped\":{}}}",
            j.clock_step_ns,
            complete && j.dropped == 0,
            j.dropped
        )?;
        for s in &j.rows {
            write!(
                out,
                "{{\"frame\":\"{}\",\"generation\":{},\"trace_ns\":\"{}\",\"unix_before_ns\":\"{}\",\"unix_after_ns\":\"{}\",\"zoom\":{},\"middle\":{},\"heads\":[",
                s.frame, s.generation, s.trace_ns, s.unix_before, s.unix_after, s.zoom, s.middle
            )?;
            for (i, h) in s.heads.iter().flatten().enumerate() {
                if i != 0 {
                    write!(out, ",")?;
                }
                write!(
                    out,
                    "[{},\"{}\",{},{},{},{},{},{},{},{},{},{}]",
                    h.id,
                    h.token,
                    h.own,
                    h.painter[0],
                    h.painter[1],
                    h.corrected[0],
                    h.corrected[1],
                    h.packet[0],
                    h.packet[1],
                    h.heading,
                    h.speed,
                    h.boost
                )?;
            }
            writeln!(out, "]}}")?;
        }
        writeln!(out, "{{\"end\":true,\"records\":{}}}", j.rows.len())?;
        out.flush()
    })();
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_is_explicit_and_export_never_overwrites() {
        JOURNAL.with_borrow_mut(|j| {
            *j = Some(Journal {
                selected: [None; 2],
                middle: false,
                rows: Vec::with_capacity(1),
                dropped: 0,
                clock_step_ns: 0,
                poses: Vec::with_capacity(1),
                pose_dropped: 0,
            })
        });
        commit(Some(Sample::default()));
        commit(Some(Sample::default()));
        received(1, 1, &[b'g', 0, 9], None).unwrap();
        received(1, 2, &[b'g', 0, 9], None).unwrap();
        let path = std::env::temp_dir().join(format!(
            "slither-rig-{}-{}.jsonl",
            std::process::id(),
            unix_ns()
        ));
        finish(&path, true).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"complete\":false,\"dropped\":1"));
        let pose_text = std::fs::read_to_string(path.with_extension("poses.jsonl")).unwrap();
        assert!(pose_text.contains("\"complete\":false,\"dropped\":1"));
        assert!(pose_text.contains("\"message\":\"1\""));
        assert!(!pose_text.contains("\"message\":\"2\""));
        start();
        assert!(finish(&path, true).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        std::fs::remove_file(path.with_extension("poses.jsonl")).unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
