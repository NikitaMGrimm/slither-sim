use crate::model::{Arena, Point, Snake};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod pit_projection_tests;

/// Stock-client compatibility: it clamps transmitted U/L sides to 512, although
/// the u16 size field could carry more.
pub(crate) const MAX_SIDE: usize = 512;
/// The stock legacy `u` snapshot has a fixed 80-cell side.
const LEGACY_SIDE: usize = 80;
/// Bound on cells retained across one L packet's team layers: four 512-by-512
/// layers, enough for the client's two-team mode at the maximum side.
pub const MAX_TEAM_MINIMAP_CELLS: usize = 1 << 20;
/// Aggregate logical grid cells decoded in one frame. Admit the largest
/// supported L snapshot (team layers plus union); repeated small packets must
/// not multiply full-grid work without bound. This is a local resource limit.
pub const MAX_FRAME_MINIMAP_CELLS: usize = MAX_TEAM_MINIMAP_CELLS + MAX_SIDE * MAX_SIDE;

/// Backend-local pit mask, independent of snakes and recipient wire histories.
#[derive(Default)]
pub(super) struct PitProjectionCache(Option<PitProjection>);

struct PitProjection {
    side: usize,
    outside: Vec<usize>,
    edge: Vec<usize>,
    min_near_outside: f64,
    max_near_edge: f64,
    min_far_edge: f64,
    max_far_inside: f64,
}

impl PitProjection {
    fn new(side: usize, radius: f64) -> Self {
        let mut mask = Self {
            side,
            outside: Vec::new(),
            edge: Vec::new(),
            min_near_outside: f64::INFINITY,
            max_near_edge: f64::NEG_INFINITY,
            min_far_edge: f64::INFINITY,
            max_far_inside: f64::NEG_INFINITY,
        };
        let center = side as f64 / 2.;
        for y in 0..side {
            for x in 0..side {
                let near = |v: usize| (v as f64 - center).max(0.).max(center - (v + 1) as f64);
                let far = |v: usize| {
                    (v as f64 - center)
                        .abs()
                        .max(((v + 1) as f64 - center).abs())
                };
                let nearest = near(x).hypot(near(y));
                if nearest > radius {
                    mask.outside.push(y * side + x);
                    mask.min_near_outside = mask.min_near_outside.min(nearest);
                } else {
                    let farthest = far(x).hypot(far(y));
                    if farthest >= radius {
                        mask.edge.push(y * side + x);
                        mask.max_near_edge = mask.max_near_edge.max(nearest);
                        mask.min_far_edge = mask.min_far_edge.min(farthest);
                    } else {
                        mask.max_far_inside = mask.max_far_inside.max(farthest);
                    }
                }
            }
        }
        mask
    }

    fn contains(&self, side: usize, radius: f64) -> bool {
        // Exact original comparisons define the interval, including equality.
        // Proportional growth can round the projected radius to adjacent floats;
        // neither an epsilon nor a rounded-radius key preserves boundary cells.
        self.side == side
            && radius < self.min_near_outside
            && radius >= self.max_near_edge
            && radius <= self.min_far_edge
            && radius > self.max_far_inside
    }
}

impl PitProjectionCache {
    pub(super) fn render_world(
        &mut self,
        world: &crate::Simulator,
        side: usize,
    ) -> Result<Minimap, String> {
        let mut map = Minimap::from_snakes(
            world.snakes(),
            &Arena {
                center: Point::default(),
                radius: world.radius(),
            },
            side,
        )?;
        if let Some(pit) = world.pit_radius() {
            self.apply(&mut map, pit / world.radius() * side as f64 / 2.);
        }
        Ok(map)
    }

    fn apply(&mut self, map: &mut Minimap, radius: f64) {
        if !self
            .0
            .as_ref()
            .is_some_and(|p| p.contains(map.side, radius))
        {
            self.0 = Some(PitProjection::new(map.side, radius));
        }
        let projection = self
            .0
            .as_ref()
            .expect("pit projection was just constructed");
        for &i in &projection.outside {
            map.cells[i] = 0;
        }
        for &i in &projection.edge {
            map.cells[i] = 1;
        }
    }
}

fn charge_cells(remaining: &mut usize, cells: usize) -> Result<(), String> {
    *remaining = remaining.checked_sub(cells).ok_or_else(|| {
        format!("Minimap frame exceeds the {MAX_FRAME_MINIMAP_CELLS}-cell work limit")
    })?;
    Ok(())
}

// Header sizing stays here solely to preserve consumer budget/error ordering.
// The shared parser validates dimensions and owns the complete run grammar.
fn selected(
    payload: &[u8],
    side: usize,
) -> Result<slither_protocol::v19::minimap::SelectedCells<'_>, String> {
    slither_protocol::v19::minimap::parse_minimap(payload, slither_protocol::DecodeAdmission::Sim)
        .and_then(|packet| packet.selected(side as u16))
        .map_err(|_| "Truncated minimap run".into())
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

fn snapshot_side(payload: &[u8]) -> Result<usize, String> {
    let bytes = payload
        .get(1..3)
        .ok_or("Expected modern M/V minimap packet")?;
    let side = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
    if !(1..=MAX_SIDE).contains(&side) {
        return Err("Invalid minimap size".into());
    }
    Ok(side)
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
    pub(crate) fn apply(
        &mut self,
        payload: &[u8],
        receipt: f64,
        remaining_cells: &mut usize,
    ) -> Result<(), String> {
        let command = *payload.first().ok_or("Empty minimap packet")?;
        match command {
            b'M' => {
                let side = snapshot_side(payload)?;
                charge_cells(remaining_cells, side * side)?;
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
                charge_cells(remaining_cells, base.len())?;
                let toggled = selected(payload, map.side)?;
                // V toggles mmdata and paints the modern target. A legacy u
                // picture must not supply its untouched cells.
                for cell in toggled {
                    let i = cell.index;
                    base[i] ^= 1;
                    map.cells[i] = base[i];
                }
                map.validate()?;
                self.shown = Some((map.clone(), receipt));
            }
            b'U' => {
                let side = side_at(payload, 1)?;
                charge_cells(remaining_cells, side * side)?;
                self.resize(side);
                let mut cells = vec![0; side * side];
                for cell in selected(payload, side)? {
                    cells[cell.index] = 1;
                }
                self.show_modern(side, cells, receipt);
                self.teams = None;
            }
            b'u' => {
                charge_cells(remaining_cells, LEGACY_SIDE * LEGACY_SIDE)?;
                self.resize(LEGACY_SIDE);
                let mut cells = vec![0; LEGACY_SIDE * LEGACY_SIDE];
                for cell in selected(payload, LEGACY_SIDE)? {
                    cells[cell.index] = 1;
                }
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
                charge_cells(remaining_cells, (usize::from(count) + 1) * side * side)?;
                self.resize(side);
                let mut union = vec![0; side * side];
                let mut layers = vec![vec![0; side * side]; usize::from(count)];
                for cell in selected(payload, side)? {
                    layers[usize::from(cell.layer)][cell.index] = 1;
                    union[cell.index] = 1;
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

// Finite nonzero values split into a signed mantissa in [1, 2) and a binary
// exponent. Normalize subnormals before extracting their exponent.
fn binary_parts(value: f64) -> (f64, i32) {
    let mut bits = value.to_bits();
    let mut adjustment = 0;
    if bits & (0x7ff << 52) == 0 {
        bits = (value * 4_503_599_627_370_496.).to_bits();
        adjustment = -52;
    }
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 + adjustment;
    let mantissa = f64::from_bits((bits & !(0x7ff << 52)) | (1023 << 52));
    (mantissa, exponent)
}

fn power_of_two(exponent: i32) -> f64 {
    f64::from_bits(((exponent + 1023) as u64) << 52)
}

// Evaluate n / d / (radius * 2^scale) without underflowing the scaled radius
// or overflowing an intermediate quotient. The final result may be infinite.
fn relative_intercept(n: f64, d: f64, radius: f64, scale: i32) -> f64 {
    if n == 0. {
        return 0.;
    }
    let (nm, ne) = binary_parts(n);
    let (dm, de) = binary_parts(d);
    let (rm, re) = binary_parts(radius);
    let (mantissa, correction) = binary_parts(nm / dm / rm);
    let exponent = ne - de - re - scale + correction;
    if exponent > 1023 {
        f64::INFINITY.copysign(mantissa)
    } else if exponent < -1075 {
        0_f64.copysign(mantissa)
    } else if exponent < -1022 {
        (mantissa * power_of_two(exponent + 1022)) * f64::MIN_POSITIVE
    } else {
        mantissa * power_of_two(exponent)
    }
}

fn difference_of_products(a: f64, b: f64, c: f64, d: f64) -> f64 {
    let cd = c * d;
    a.mul_add(b, -cd) + (-c).mul_add(d, cd)
}

// Only the extreme-edge fallback uses this approximate line clip. Keep world
// endpoints: projected infinities and rounded-equal entry/exit parameters can
// otherwise discard a real crossing. Work depends on grid size, not tail extent.
fn clipped_edge(a: Point, b: Point, arena: &Arena, side: usize) -> Option<(Point, Point)> {
    if !a.finite()
        || !b.finite()
        || !arena.center.finite()
        || !arena.radius.is_finite()
        || arena.radius <= 0.
    {
        return None;
    }
    let mut dx = b.x - a.x;
    let mut dy = b.y - a.y;
    if !dx.is_finite() || !dy.is_finite() {
        dx = b.x * 0.5 - a.x * 0.5;
        dy = b.y * 0.5 - a.y * 0.5;
    }
    let swap = dy.abs() > dx.abs();
    let (major, minor) = if swap { (dy, dx) } else { (dx, dy) };
    if major == 0. {
        return None;
    }
    let slope = minor / major;
    // Separate axis scales retain tiny offsets even when the other axis spans
    // enormous coordinates. Powers of two avoid additional division rounding.
    let scale_axis = |a: f64, b: f64, center: f64| {
        let largest = a.abs().max(b.abs()).max(center.abs());
        if largest == 0. {
            0
        } else {
            (-binary_parts(largest).1).clamp(-1022, 1023)
        }
    };
    let (scale_x, scale_y) = (
        scale_axis(a.x, b.x, arena.center.x),
        scale_axis(a.y, b.y, arena.center.y),
    );
    let scaled = |p: Point| Point {
        x: p.x * power_of_two(scale_x),
        y: p.y * power_of_two(scale_y),
    };
    let (sa, sb, center) = (scaled(a), scaled(b), scaled(arena.center));
    let axes = |p: Point| if swap { (p.y, p.x) } else { (p.x, p.y) };
    let ((au, av), (bu, bv), (cu, cv)) = (axes(sa), axes(sb), axes(center));
    let (du, dv) = (bu - au, bv - av);
    if du == 0. {
        return None;
    }
    // Compensated products preserve proportional crossings that slope times a
    // distant anchor would turn into a large spurious offset from the center.
    let n = difference_of_products(av, bu, au, bv) + difference_of_products(cu, dv, cv, du);
    let minor_scale = if swap { scale_x } else { scale_y };
    let intercept = relative_intercept(n, du, arena.radius, minor_scale);
    if !intercept.is_finite() {
        return None;
    }
    let (a_major, b_major, center_major) = (axes(a).0, axes(b).0, axes(arena.center).0);
    let project = |value: f64| (value - center_major) / arena.radius;
    let mut lo = project(a_major.min(b_major)).max(-1.);
    let mut hi = project(a_major.max(b_major)).min(1.);
    if slope == 0. {
        if !(-1. ..=1.).contains(&intercept) {
            return None;
        }
    } else {
        let first = (-1. - intercept) / slope;
        let last = (1. - intercept) / slope;
        lo = lo.max(first.min(last));
        hi = hi.min(first.max(last));
    }
    if lo > hi {
        return None;
    }
    let grid = |u: f64| {
        let v = slope.mul_add(u, intercept).clamp(-1., 1.);
        let (x, y) = if swap { (v, u) } else { (u, v) };
        Point {
            x: ((x + 1.) * side as f64 / 2.).clamp(0., side as f64),
            y: ((y + 1.) * side as f64 / 2.).clamp(0., side as f64),
        }
    };
    Some((grid(lo), grid(hi)))
}

impl Minimap {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_SIDE).contains(&self.side)
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
        let (mut map, delta) = match payload.first() {
            Some(b'M') => {
                let side = snapshot_side(payload)?;
                (
                    Self {
                        side,
                        cells: vec![0; side * side],
                        age_seconds: 0.,
                    },
                    false,
                )
            }
            Some(b'V') => {
                let mut map = previous
                    .ok_or("Minimap delta before full snapshot")?
                    .clone();
                map.validate()?;
                map.age_seconds = 0.;
                (map, true)
            }
            _ => return Err("Expected modern M/V minimap packet".into()),
        };
        for cell in selected(payload, map.side)? {
            if delta {
                map.cells[cell.index] ^= 1;
            } else {
                map.cells[cell.index] = 1;
            }
        }
        Ok(map)
    }

    /// Live-radius map for the stock client's live-radius own-player marker.
    /// Pit worlds hide snake cells outside the pit and retain its edge;
    /// proportional growth keeps the outline cells in the same projection.
    pub fn from_world(world: &crate::Simulator, side: usize) -> Result<Self, String> {
        let mut map = Self::from_snakes(
            world.snakes(),
            &Arena {
                center: Point::default(),
                radius: world.radius(),
            },
            side,
        )?;
        if let Some(pit) = world.pit_radius() {
            let radius = pit / world.radius() * side as f64 / 2.;
            let center = side as f64 / 2.;
            for y in 0..side {
                for x in 0..side {
                    let near = |v: usize| (v as f64 - center).max(0.).max(center - (v + 1) as f64);
                    let far = |v: usize| {
                        (v as f64 - center)
                            .abs()
                            .max(((v + 1) as f64 - center).abs())
                    };
                    let nearest = near(x).hypot(near(y));
                    if nearest > radius {
                        map.cells[y * side + x] = 0;
                    } else if far(x).hypot(far(y)) >= radius {
                        map.cells[y * side + x] = 1;
                    }
                }
            }
        }
        Ok(map)
    }

    /// Simulator approximation: rasterize live body centerlines to a binary grid.
    /// The original server's aggregation rule is unavailable; this is not a food map.
    /// Returns an error unless `side` is in 1..=512.
    pub fn from_snakes(snakes: &[Snake], arena: &Arena, side: usize) -> Result<Self, String> {
        if !(1..=MAX_SIDE).contains(&side) {
            return Err("Invalid minimap size".into());
        }
        let mut cells = vec![0; side * side];
        let grid = |p: Point| Point {
            x: ((p.x - arena.center.x) / arena.radius + 1.) * side as f64 / 2.,
            y: ((p.y - arena.center.y) / arena.radius + 1.) * side as f64 / 2.,
        };
        for s in snakes.iter().filter(|s| s.alive) {
            let mut previous = s.head;
            for next in std::iter::once(s.head).chain(s.body.iter().copied()) {
                let (mut a, mut b) = (grid(previous), grid(next));
                let samples = ((b.x - a.x).abs().max((b.y - a.y).abs()) * 2.)
                    .ceil()
                    .max(1.);
                if !a.finite() || !b.finite() || samples > (2 * side) as f64 {
                    let clipped = clipped_edge(previous, next, arena, side);
                    previous = next;
                    let Some(edge) = clipped else { continue };
                    (a, b) = edge;
                } else {
                    previous = next;
                }
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
            }
        }
        Ok(Self {
            side,
            cells,
            age_seconds: 0.,
        })
    }
}
