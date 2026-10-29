//! Recipient-only protocol-13 wire adapter. Shared simulation, visibility, report
//! clocks and protocol-19 packet preparation stay unchanged. No adapter is
//! allocated for stock clients or bots.
//!
//! Layout evidence: tests/fixtures/protocol-catalog-13.json and
//! research/fidelity/2026-10-26-protocol13-catalog/REPORT.md (#177).
//! The adapter follows the *encoded* front anchors, including rounding, so an
//! angle-only 19 step becomes the same position in 13 without accumulating a
//! second quantization error. It never feeds presentation state into physics.
use super::{Minimap, encode};
use crate::model::Point;
use std::{collections::BTreeMap, f64::consts::TAU, sync::Arc};

/// One broadcast's full-map conversions. Input bytes and the exact projection
/// and XOR baseline all matter: skipped maps and reconnects can differ.
#[derive(Default)]
pub(crate) struct FullMapCache {
    entries: BTreeMap<(u64, u64, usize), Vec<FullMapPacket>>,
    snapshots: Vec<FullMapSnapshot>,
}

struct FullMapPacket {
    // Retain the identity strongly so a replaced baseline cannot reuse its
    // address while this cache still names it.
    _previous: Option<Arc<Minimap>>,
    input: Vec<u8>,
    snapshot: usize,
    output: Arc<Vec<u8>>,
}

struct FullMapSnapshot {
    source: Arc<Minimap>,
    projected: BTreeMap<(u64, u64), Arc<Vec<u8>>>,
}

impl FullMapCache {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.snapshots.clear();
    }

    #[cfg(test)]
    pub(crate) fn cached_packets(&self) -> usize {
        self.entries.values().map(Vec::len).sum()
    }
}

#[derive(Clone, Debug)]
struct Front {
    source: Point,
    sent: Option<[u16; 2]>,
}

#[derive(Clone, Debug)]
pub(crate) struct Encoder {
    own: u16,
    spacing: f64,
    fronts: BTreeMap<u16, Front>,
    full_map: Option<FullMap>,
}

impl Encoder {
    pub(crate) fn new(own: u16) -> Self {
        Self {
            own,
            spacing: crate::physics::ANCHOR_SPACING,
            fronts: BTreeMap::new(),
            full_map: None,
        }
    }

    pub(crate) fn use_full_minimap(&mut self) {
        self.full_map = Some(FullMap {
            grd: f64::from(encode::GRID_CENTER),
            radius: f64::from(encode::GRID_CENTER),
            previous: None,
        });
    }

    pub(crate) fn packets<P: AsRef<[u8]>>(
        &mut self,
        packets: &[P],
        maps: &mut FullMapCache,
    ) -> Result<Vec<Vec<u8>>, String> {
        let mut out = Vec::with_capacity(packets.len());
        for p in packets {
            let p = p.as_ref();
            match p[0] {
                b'a' => {
                    let mut p = p.to_vec();
                    p[23] = 13;
                    self.spacing = f64::from(p[24]);
                    if let Some(map) = &mut self.full_map {
                        map.grd = u24_at(&p, 1);
                        map.radius = u24_at(&p, 27);
                    }
                    out.push(p);
                }
                b's' if p.len() > 7 => out.push(self.appearance(p)?),
                b's' => {
                    self.fronts.remove(&u16_at(p, 1));
                    out.push(p.to_vec());
                }
                b'd' | b'7' => {
                    let mut q = vec![if p[0] == b'd' { b'e' } else { b'E' }];
                    q.extend(self.own.to_be_bytes());
                    q.extend_from_slice(&p[1..]);
                    out.push(q);
                }
                b'e' | b'E' | b'3' | b'4' | b'5' if p.len() <= 3 => {
                    let mut q = vec![p[0]];
                    q.extend(self.own.to_be_bytes());
                    q.extend_from_slice(&p[1..]);
                    out.push(q);
                }
                b'g' | b'G' | b'n' | b'N' | b'=' | b'+' => out.push(self.anchor(p)?),
                b'F' => {
                    // 13 has no sector header; split before the six-byte tuples
                    // exceed the common frame-length limit (8191 bytes).
                    for chunk in p[3..].chunks(1365 * 4) {
                        let mut q = vec![b'F'];
                        for food in chunk.as_chunks::<4>().0 {
                            let xy = food_xy([p[1], p[2], food[1], food[2]]);
                            q.push(food[0]);
                            q.extend(xy[0].to_be_bytes());
                            q.extend(xy[1].to_be_bytes());
                            q.push(food[3]);
                        }
                        out.push(q);
                    }
                }
                b'f' | b'b' => {
                    let xy = food_xy([p[1], p[2], p[3], p[4]]);
                    let mut q = vec![p[0], p[5]];
                    q.extend(xy[0].to_be_bytes());
                    q.extend(xy[1].to_be_bytes());
                    q.push(p[6]);
                    out.push(q);
                }
                b'c' | b'<' => {
                    let xy = food_xy([p[1], p[2], p[3], p[4]]);
                    let mut q = vec![b'c'];
                    q.extend(xy[0].to_be_bytes());
                    q.extend(xy[1].to_be_bytes());
                    q.extend_from_slice(&p[5..]);
                    out.push(q);
                }
                b'M' | b'V' => {
                    // Client-derived: NTL 9.78's U/u handler disables dynmm,
                    // scaling its own dot by setup grd instead of live radius.
                    // Its Qm>=13 M/V decoder has the same reverse scan and 255
                    // escape as 19. Keep the shared bytes and live projection.
                    // See research/fidelity/2026-10-27-legacy-map-motion/REPORT.md.
                    out.push(if let Some(map) = &mut self.full_map {
                        map.packet(p, maps)?
                    } else {
                        p.to_vec()
                    });
                }
                b'z' => {
                    if let Some(map) = &mut self.full_map {
                        map.radius = u24_at(p, 1);
                    }
                    out.push(p.to_vec());
                }
                // Common catalog families, plus existing local score/record/rules
                // presentation. Never forward a new 19-only layout silently.
                b'e' | b'E' | b'3' | b'4' | b'5' | b'h' | b'r' | b'w' | b'y' | b'j' | b'l'
                | b'k' | b'p' | b'v' | b'S' | b'R' | b'm' | b'~' => out.push(p.to_vec()),
                _ => return Err(format!("No protocol-13 encoding for code {}", p[0])),
            }
        }
        Ok(out)
    }

    fn appearance(&mut self, p: &[u8]) -> Result<Vec<u8>, String> {
        let id = u16_at(p, 1);
        let custom = 23 + usize::from(p[22]);
        let body = custom + 2 + usize::from(p[custom]);
        let mut last = Point {
            x: u24_at(p, body) / 5.,
            y: u24_at(p, body + 3) / 5.,
        };
        for delta in p[body + 6..p.len() - 2].as_chunks::<2>().0 {
            last.x += (f64::from(delta[0]) - 127.) / 2.;
            last.y += (f64::from(delta[1]) - 127.) / 2.;
        }
        let angle = f64::from(u16_at(p, p.len() - 2)) * TAU / 65536.;
        let front = Point {
            x: last.x + angle.cos() * self.spacing,
            y: last.y + angle.sin() * self.spacing,
        };
        let mut q = p[..p.len() - 2].to_vec();
        for delta in [front.x - last.x, front.y - last.y] {
            let byte = (delta * 2.).round() + 127.;
            if !(0. ..=255.).contains(&byte) {
                return Err("Protocol-13 appearance step exceeds half-unit byte".into());
            }
            q.push(byte as u8);
        }
        // The appearance front is on a half-unit grid; the first integer
        // anchor uses absolute g/n to establish an unambiguous relative base.
        self.fronts.insert(
            id,
            Front {
                source: front,
                sent: None,
            },
        );
        Ok(q)
    }

    fn anchor(&mut self, p: &[u8]) -> Result<Vec<u8>, String> {
        let grow = matches!(p[0], b'n' | b'N' | b'+');
        let own = matches!(p[0], b'G' | b'N')
            || p[0] == b'=' && p.len() == 7
            || p[0] == b'+' && p.len() == 10;
        let (id, at) = if own {
            (self.own, 1)
        } else {
            (u16_at(p, 1), 3)
        };
        let front = self
            .fronts
            .get_mut(&id)
            .ok_or("Protocol-13 anchor before appearance")?;
        let absolute = matches!(p[0], b'=' | b'+');
        let point = if absolute {
            Point {
                x: f64::from(u16_at(p, at + 2)),
                y: f64::from(u16_at(p, at + 4)),
            }
        } else {
            let angle = f64::from(u16_at(p, at)) * TAU / 65536.;
            Point {
                x: front.source.x + angle.cos() * self.spacing,
                y: front.source.y + angle.sin() * self.spacing,
            }
        };
        let xy = [coordinate(point.x)?, coordinate(point.y)?];
        let delta = front.sent.map(|sent| {
            [
                i32::from(xy[0]) - i32::from(sent[0]),
                i32::from(xy[1]) - i32::from(sent[1]),
            ]
        });
        // Absolute corrections also reset the browser's quantized base. Ordinary
        // ~42-unit steps fit the relative form; large corrections use g/n.
        let relative =
            !absolute && delta.is_some_and(|delta| delta.iter().all(|d| (-128..=127).contains(d)));
        let mut q = vec![match (relative, grow) {
            (true, false) => b'G',
            (true, true) => b'N',
            (false, false) => b'g',
            (false, true) => b'n',
        }];
        q.extend(id.to_be_bytes());
        if relative {
            q.extend(delta.expect("relative base").map(|d| (d + 128) as u8));
        } else {
            q.extend(xy[0].to_be_bytes());
            q.extend(xy[1].to_be_bytes());
        }
        if grow {
            q.extend_from_slice(&p[p.len() - 3..]);
        }
        *front = Front {
            source: point,
            sent: Some(xy),
        };
        Ok(q)
    }
}

fn u16_at(p: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([p[at], p[at + 1]])
}
fn u24_at(p: &[u8], at: usize) -> f64 {
    f64::from(u32::from_be_bytes([0, p[at], p[at + 1], p[at + 2]]))
}
fn coordinate(value: f64) -> Result<u16, String> {
    if value.is_finite() && (0. ..=65535.).contains(&value.round()) {
        Ok(value.round() as u16)
    } else {
        Err("Protocol-13 coordinate outside u16".into())
    }
}

/// Project the shared 300/256 food grid onto the legacy integer grid. The step
/// is >1, so rounding cannot merge adjacent keys; births and removals use the
/// same projection, including decorative pit pellets.
fn food_xy(key: [u8; 4]) -> [u16; 2] {
    let cell = |sector: u8, offset: u8| {
        u16::from(sector) * encode::SECTOR_SIZE
            + ((u32::from(offset) * u32::from(encode::SECTOR_SIZE) + 128) / 256) as u16
    };
    [cell(key[0], key[2]), cell(key[1], key[3])]
}

/// Only [99,3] opts into this client-derived compatibility projection. NTL's
/// dynamic-map-off dot uses grd, and its decoder has no extended runs or XOR
/// base. Reproject occupied live cells into that frame, retaining coverage when
/// several source cells collapse into one. No stock recipient uses this state.
#[derive(Clone, Debug)]
struct FullMap {
    grd: f64,
    radius: f64,
    previous: Option<Arc<Minimap>>,
}

impl FullMap {
    fn packet(&mut self, p: &[u8], cache: &mut FullMapCache) -> Result<Vec<u8>, String> {
        let key = (
            self.grd.to_bits(),
            self.radius.to_bits(),
            self.previous
                .as_ref()
                .map_or(0, |map| Arc::as_ptr(map) as usize),
        );
        if let Some(packet) = cache
            .entries
            .get(&key)
            .and_then(|entries| entries.iter().find(|entry| entry.input == p))
        {
            self.previous = Some(cache.snapshots[packet.snapshot].source.clone());
            return Ok(packet.output.as_ref().clone());
        }
        let decoded = Minimap::packet(self.previous.as_deref(), p)?;
        // Different old XOR bases can decode to the same current snapshot.
        // Intern it so reconnects do not permanently fragment later sharing.
        let snapshot = cache.snapshots.iter().position(|snapshot| {
            let old = &snapshot.source;
            old.side == decoded.side
                && old.age_seconds.to_bits() == decoded.age_seconds.to_bits()
                && old.cells == decoded.cells
        });
        let snapshot = snapshot.unwrap_or_else(|| {
            cache.snapshots.push(FullMapSnapshot {
                source: Arc::new(decoded),
                projected: BTreeMap::new(),
            });
            cache.snapshots.len() - 1
        });
        let FullMapSnapshot { source, projected } = &mut cache.snapshots[snapshot];
        let output = projected
            .entry((key.0, key.1))
            .or_insert_with(|| Arc::new(self.project(source)))
            .clone();
        let next = source.clone();
        // Cache only successful decoding. A bad delta must still fail without
        // advancing this recipient or poisoning another recipient's baseline.
        cache.entries.entry(key).or_default().push(FullMapPacket {
            _previous: self.previous.clone(),
            input: p.to_vec(),
            snapshot,
            output: output.clone(),
        });
        self.previous = Some(next);
        Ok(output.as_ref().clone())
    }

    fn project(&self, source: &Minimap) -> Vec<u8> {
        let side = source.side;
        let mut map = Minimap {
            side,
            cells: vec![0; side * side],
            age_seconds: 0.,
        };
        let scale = self.radius / self.grd;
        let edge = |cell: usize| {
            (side as f64 / 2. + (cell as f64 - side as f64 / 2.) * scale).clamp(0., side as f64)
        };
        for (i, &set) in source.cells.iter().enumerate() {
            if set == 0 {
                continue;
            }
            let (x, y) = (i % side, i / side);
            for row in edge(y).floor() as usize..edge(y + 1).ceil() as usize {
                for col in edge(x).floor() as usize..edge(x + 1).ceil() as usize {
                    map.cells[row * side + col] = 1;
                }
            }
        }
        minimap(&map)
    }
}

/// Full U in the grd projection, with plain zero runs and no delta base.
fn minimap(map: &Minimap) -> Vec<u8> {
    let mut q = vec![b'U'];
    q.extend((map.side as u16).to_be_bytes());
    let mut remaining = map.cells.len();
    while remaining > 0 {
        let zeros = map.cells[..remaining]
            .iter()
            .rev()
            .take_while(|&&v| v == 0)
            .count();
        if zeros == remaining {
            break;
        }
        if zeros >= 7 {
            let mut left = zeros;
            while left > 0 {
                let skip = left.min(127);
                q.push(128 + skip as u8);
                left -= skip;
            }
            remaining -= zeros;
        } else {
            let mut byte = 0;
            for bit in (0..7).rev() {
                if remaining == 0 {
                    break;
                }
                remaining -= 1;
                byte |= u8::from(map.cells[remaining] != 0) << bit;
            }
            q.push(byte);
        }
    }
    q
}

#[cfg(test)]
mod tests;
