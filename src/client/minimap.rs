use crate::model::{Arena, Point, Snake};
use serde::{Deserialize, Serialize};

const MAX_SIDE: usize = 512;
/// The stock legacy `u` snapshot has a fixed 80-cell side.
const LEGACY_SIDE: usize = 80;
/// Bound on cells retained across one L packet's team layers: four 512-by-512
/// layers, enough for the client's two-team mode at the maximum side.
pub const MAX_TEAM_MINIMAP_CELLS: usize = 1 << 20;

/// Decode one stock run-length layer of `cells` cells starting at `cursor`, calling
/// `set` with each occupied index. Bytes of 128 or more skip `k - 128` cells; with
/// `extended`, 255 skips `126 * next byte`. Other bytes carry seven cells, high bit
/// first. Reverse order starts at the last index, as the stock M/V/U/L loops do.
/// A layer ends when its cells or the payload are exhausted.
fn layer(
    payload: &[u8],
    cursor: &mut usize,
    cells: usize,
    extended: bool,
    reverse: bool,
    mut set: impl FnMut(usize),
) -> Result<(), String> {
    let mut done = 0;
    while *cursor < payload.len() && done < cells {
        let k = payload[*cursor];
        *cursor += 1;
        if k >= 128 {
            let skip = if extended && k == 255 {
                let n = *payload.get(*cursor).ok_or("Truncated minimap run")?;
                *cursor += 1;
                126 * usize::from(n)
            } else {
                usize::from(k - 128)
            };
            done = (done + skip).min(cells);
        } else {
            for bit in (0..7).rev() {
                if done == cells {
                    break;
                }
                if k & (1 << bit) != 0 {
                    set(if reverse { cells - 1 - done } else { done });
                }
                done += 1;
            }
        }
    }
    Ok(())
}

fn side_at(payload: &[u8], at: usize) -> Result<usize, String> {
    let (Some(&a), Some(&b)) = (payload.get(at), payload.get(at + 1)) else {
        return Err("Truncated minimap size".into());
    };
    // The stock client clamps U/L sizes to 512; a zero side has no cells.
    match usize::from(u16::from_be_bytes([a, b])).min(MAX_SIDE) {
        0 => Err("Invalid minimap size".into()),
        side => Ok(side),
    }
}

/// Received minimap state across all stock command families.
///
/// `shown` is the latest picture. `modern` is the separate `asmc2` target used by
/// M/V/U/L; legacy `u` paints only `asmc`, so V must resume the modern target.
/// `base` is the stock client's `mmdata`, the
/// array V deltas toggle; only M and V write its cells. Before any full snapshot
/// it is unknown and V is rejected. A U, `u` or L snapshot whose side differs
/// from the current base (or with none yet) resets it to zeros of that side, as
/// the stock `setMinimapSize` does; a same-side snapshot leaves it unchanged.
#[derive(Clone, Debug, Default)]
pub(crate) struct MinimapDecoder {
    pub(crate) shown: Option<(Minimap, f64)>,
    modern: Option<Minimap>,
    base: Option<Vec<u8>>,
    pub(crate) command: Option<u8>,
    /// Layers of the last L snapshot, kept until another full snapshot replaces it.
    pub(crate) teams: Option<(usize, Vec<Vec<u8>>, f64)>,
}
impl MinimapDecoder {
    fn resize(&mut self, side: usize) {
        if self
            .base
            .as_ref()
            .is_none_or(|base| base.len() != side * side)
        {
            self.base = Some(vec![0; side * side]);
            self.modern = Some(Minimap {
                side,
                cells: vec![0; side * side],
                age_seconds: 0.,
            });
        }
    }

    fn show(&mut self, side: usize, cells: Vec<u8>, receipt: f64) {
        self.shown = Some((
            Minimap {
                side,
                cells,
                age_seconds: 0.,
            },
            receipt,
        ));
    }

    fn show_modern(&mut self, side: usize, cells: Vec<u8>, receipt: f64) {
        self.show(side, cells, receipt);
        self.modern = self.shown.as_ref().map(|(map, _)| map.clone());
    }

    /// Apply one M, V, U, u or L payload including its command byte. Callers
    /// clone the decoder per frame so an error leaves the committed state intact.
    pub(crate) fn apply(&mut self, payload: &[u8], receipt: f64) -> Result<(), String> {
        let command = *payload.first().ok_or("Empty minimap packet")?;
        match command {
            b'M' => {
                let map = Minimap::packet(None, payload)?;
                self.base = Some(map.cells.clone());
                self.show_modern(map.side, map.cells, receipt);
                self.teams = None;
            }
            b'V' => {
                let base = self
                    .base
                    .as_mut()
                    .ok_or("Minimap delta before full snapshot")?;
                let map = self
                    .modern
                    .as_mut()
                    .ok_or("Minimap delta before full snapshot")?;
                let mut toggled = Vec::new();
                layer(payload, &mut 1, base.len(), true, true, |i| toggled.push(i))?;
                // V toggles mmdata and paints the modern target. A legacy u
                // picture must not supply its untouched cells.
                for i in toggled {
                    base[i] ^= 1;
                    map.cells[i] = base[i];
                }
                map.validate()?;
                self.shown = Some((map.clone(), receipt));
            }
            b'U' => {
                let side = side_at(payload, 1)?;
                self.resize(side);
                let mut cells = vec![0; side * side];
                layer(payload, &mut 3, cells.len(), false, true, |i| cells[i] = 1)?;
                self.show_modern(side, cells, receipt);
                self.teams = None;
            }
            b'u' => {
                self.resize(LEGACY_SIDE);
                let mut cells = vec![0; LEGACY_SIDE * LEGACY_SIDE];
                layer(payload, &mut 1, cells.len(), false, false, |i| cells[i] = 1)?;
                self.show(LEGACY_SIDE, cells, receipt);
                self.teams = None;
            }
            b'L' => {
                let count = *payload.get(1).ok_or("Truncated team minimap")?;
                let side = side_at(payload, 2)?;
                if usize::from(count) * side * side > MAX_TEAM_MINIMAP_CELLS {
                    return Err(format!(
                        "Unsupported team minimap: {count} layers of {side}x{side} exceed the \
                         {MAX_TEAM_MINIMAP_CELLS}-cell limit"
                    ));
                }
                self.resize(side);
                let mut cursor = 4;
                let mut union = vec![0; side * side];
                let mut layers = Vec::with_capacity(usize::from(count));
                for _ in 0..count {
                    let mut cells = vec![0; side * side];
                    layer(payload, &mut cursor, cells.len(), true, true, |i| {
                        cells[i] = 1;
                        union[i] = 1;
                    })?;
                    layers.push(cells);
                }
                self.show_modern(side, union, receipt);
                self.teams = Some((side, layers, receipt));
            }
            _ => return Err("Expected minimap packet".into()),
        }
        self.command = Some(command);
        Ok(())
    }
}

/// A received binary occupancy snapshot, north-up in arena coordinates.
/// Age is time since receipt, not the unknown server sampling/transport delay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Minimap {
    pub side: usize,
    pub cells: Vec<u8>,
    pub age_seconds: f64,
}
impl Minimap {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=512).contains(&self.side)
            || self.cells.len() != self.side * self.side
            || self.cells.iter().any(|&v| v > 1)
            || !self.age_seconds.is_finite()
            || self.age_seconds < 0.
        {
            return Err("Invalid binary minimap".into());
        }
        Ok(())
    }

    /// Modern M/V command payload, including command byte but no transport header.
    /// Decode transactionally: a malformed delta cannot corrupt the last snapshot.
    pub fn packet(previous: Option<&Self>, payload: &[u8]) -> Result<Self, String> {
        let (mut map, mut cursor, delta) = match payload.first() {
            Some(b'M') if payload.len() >= 3 => {
                let side = usize::from(u16::from_be_bytes([payload[1], payload[2]]));
                if !(1..=512).contains(&side) {
                    return Err("Invalid minimap size".into());
                }
                (
                    Self {
                        side,
                        cells: vec![0; side * side],
                        age_seconds: 0.,
                    },
                    3,
                    false,
                )
            }
            Some(b'V') => {
                let mut map = previous
                    .ok_or("Minimap delta before full snapshot")?
                    .clone();
                map.validate()?;
                map.age_seconds = 0.;
                (map, 1, true)
            }
            _ => return Err("Expected modern M/V minimap packet".into()),
        };
        let cells = map.cells.len();
        layer(payload, &mut cursor, cells, true, true, |i| {
            if delta {
                map.cells[i] ^= 1;
            } else {
                map.cells[i] = 1;
            }
        })?;
        Ok(map)
    }

    /// Simulator approximation: rasterize live body centerlines to a binary grid.
    /// The original server's aggregation rule is unavailable; this is not a food map.
    pub fn from_snakes(snakes: &[Snake], arena: &Arena, side: usize) -> Self {
        assert!((1..=512).contains(&side));
        let mut cells = vec![0; side * side];
        let grid = |p: Point| Point {
            x: ((p.x - arena.center.x) / arena.radius + 1.) * side as f64 / 2.,
            y: ((p.y - arena.center.y) / arena.radius + 1.) * side as f64 / 2.,
        };
        for s in snakes.iter().filter(|s| s.alive) {
            let mut a = grid(s.head);
            for b in std::iter::once(s.head)
                .chain(s.body.iter().copied())
                .map(grid)
            {
                let steps = ((b.x - a.x).abs().max((b.y - a.y).abs()) * 2.)
                    .ceil()
                    .max(1.) as usize;
                for k in 0..=steps {
                    let t = k as f64 / steps as f64;
                    let x = (a.x + (b.x - a.x) * t).floor();
                    let y = (a.y + (b.y - a.y) * t).floor();
                    if x >= 0. && y >= 0. && x < side as f64 && y < side as f64 {
                        cells[y as usize * side + x as usize] = 1;
                    }
                }
                a = b;
            }
        }
        Self {
            side,
            cells,
            age_seconds: 0.,
        }
    }
}
