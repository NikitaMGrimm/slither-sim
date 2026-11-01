//! Fixed double-precision arena-edge geometry, independent of update clocks.

use super::{PI, PresentationError, finite};
use crate::state::Point;

/// Retained arena center and draw-prepared camera polar coordinates.
/// Angle and distance remain stale on draws without the original own object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArenaEdgeView {
    center: f64,
    angle: f64,
    distance: f64,
}

impl Default for ArenaEdgeView {
    fn default() -> Self {
        Self {
            center: 16384.,
            angle: 0.,
            distance: 0.,
        }
    }
}

impl ArenaEdgeView {
    /// Construct a frozen view; the center is the absolute coordinate on both axes.
    pub fn new(center: f64, angle: f64, distance: f64) -> Result<Self, PresentationError> {
        finite(&[center, angle, distance])?;
        if center <= 0. || distance < 0. {
            return Err(PresentationError);
        }
        Ok(Self {
            center,
            angle,
            distance,
        })
    }

    /// Absolute arena center coordinate, shared by x and y.
    pub fn center(self) -> f64 {
        self.center
    }

    /// Retained camera angle in radians, increasing toward positive screen y.
    pub fn angle(self) -> f64 {
        self.angle
    }

    /// Retained camera distance from the arena center, in world units.
    pub fn distance(self) -> f64 {
        self.distance
    }

    pub(super) fn set_center(&mut self, center: f64) -> Result<(), PresentationError> {
        *self = Self::new(center, self.angle, self.distance)?;
        Ok(())
    }

    pub(super) fn prepare_draw(&mut self, camera: Point) -> Result<(), PresentationError> {
        let dx = camera.x - self.center;
        let dy = camera.y - self.center;
        let angle = dy.atan2(dx);
        let distance = (dx * dx + dy * dy).sqrt();
        *self = Self::new(self.center, angle, distance)?;
        Ok(())
    }
}

/// Frozen draw inputs; radius is the displayed easing output and may be zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArenaEdgeInput {
    /// Event-time center and retained camera polar coordinates.
    pub view: ArenaEdgeView,
    /// Current absolute camera position, independently of retained polar coordinates.
    pub camera: Point,
    /// Positive backing-pixel scale per world unit.
    pub zoom: f64,
    /// Desired backing width in pixels.
    pub width: u32,
    /// Desired backing height in pixels.
    pub height: u32,
    /// Current displayed radius, without a received-target substitution.
    pub radius: f64,
}

/// Screen-coordinate paths: close and fill the band, then stroke the open inner path.
#[derive(Clone, Debug, PartialEq)]
pub struct ArenaEdgePlan {
    /// Initial move point, 41 inner line points, then 41 outer return points.
    pub fill: [Point; 83],
    /// Initial move point and 41 inner line points, including the duplicate start.
    pub stroke: [Point; 42],
    /// Backing-pixel stroke width.
    pub line_width: f64,
}

/// The unchanged stock arc, or a bounded, screen-error-controlled full circle.
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "One plan per frame: keep the stock fixed arrays allocation-free"
)]
pub enum ZoomArenaEdgePlan {
    /// Exact stock distance gate, sampling order, and arithmetic.
    Local(ArenaEdgePlan),
    /// Complete perimeter, with chord error bounded in projected pixels.
    Whole {
        /// Duplicate inner start, inner perimeter, and reversed outer perimeter.
        fill: Vec<Point>,
        /// Duplicate start followed by the complete inner perimeter.
        stroke: Vec<Point>,
        /// Projected stroke width in pixels.
        line_width: f64,
    },
}

impl ZoomArenaEdgePlan {
    /// Closed fill-band path, including the duplicate inner start.
    pub fn fill(&self) -> &[Point] {
        match self {
            Self::Local(plan) => &plan.fill,
            Self::Whole { fill, .. } => fill,
        }
    }

    /// Open stroke path, including the duplicate inner start.
    pub fn stroke(&self) -> &[Point] {
        match self {
            Self::Local(plan) => &plan.stroke,
            Self::Whole { stroke, .. } => stroke,
        }
    }

    /// Projected stroke width in pixels.
    pub fn line_width(&self) -> f64 {
        match self {
            Self::Local(plan) => plan.line_width,
            Self::Whole { line_width, .. } => *line_width,
        }
    }
}

/// At most 8,195 fill and 4,098 stroke points, before renderer draw budgeting.
pub const MAX_ARENA_EDGE_SEGMENTS: usize = 4096;

/// Keep the stock arc whenever it covers the projected visible boundary band.
/// `adjust_to_view` only opts out of exact stock behavior; it does not select a
/// full circle. In particular, a small wheel step must not change nearby edges.
pub fn plan_arena_edge_with_zoom(
    input: ArenaEdgeInput,
    adjust_to_view: bool,
) -> Result<Option<ZoomArenaEdgePlan>, PresentationError> {
    let local = plan_arena_edge(input)?;
    if !adjust_to_view || !visible_beyond_local(input, local.is_none())? {
        return Ok(local.map(ZoomArenaEdgePlan::Local));
    }
    let outer = (input.radius + 4000.) * input.zoom;
    finite(&[outer])?;
    // Sagitta = R * (1 - cos(pi/n)). The equivalent asin formula avoids
    // cancellation at large radii; use the outer band so both paths meet 0.5 px.
    let segments = ((PI / (2. * (0.25 / outer).min(1.).sqrt().asin())).ceil() as usize).max(40);
    if segments > MAX_ARENA_EDGE_SEGMENTS {
        return Err(PresentationError);
    }
    let project = |angle: f64, radius: f64| -> Result<Point, PresentationError> {
        let x = f64::from(input.width) / 2.
            + (input.view.center + angle.cos() * radius - input.camera.x) * input.zoom;
        let y = f64::from(input.height) / 2.
            + (input.view.center + angle.sin() * radius - input.camera.y) * input.zoom;
        finite(&[x, y])?;
        Ok(Point { x, y })
    };
    let mut stroke = Vec::with_capacity(segments + 2);
    stroke.push(project(-PI, input.radius)?);
    for i in 0..=segments {
        stroke.push(project(
            -PI + i as f64 / segments as f64 * 2. * PI,
            input.radius,
        )?);
    }
    let mut fill = Vec::with_capacity(2 * segments + 3);
    fill.extend_from_slice(&stroke);
    for i in (0..=segments).rev() {
        fill.push(project(
            -PI + i as f64 / segments as f64 * 2. * PI,
            input.radius + 4000.,
        )?);
    }
    Ok(Some(ZoomArenaEdgePlan::Whole {
        fill,
        stroke,
        line_width: 23. * input.zoom,
    }))
}

fn visible_beyond_local(input: ArenaEdgeInput, gated: bool) -> Result<bool, PresentationError> {
    let half_arc = (2000. / input.radius).min(PI);
    let inner = input.radius * input.zoom;
    let outer = (input.radius + 4000.) * input.zoom;
    let cx = f64::from(input.width) / 2. + (input.view.center - input.camera.x) * input.zoom;
    let cy = f64::from(input.height) / 2. + (input.view.center - input.camera.y) * input.zoom;
    let pad = 23. * input.zoom / 2. + 0.5;
    let xs = [-pad - cx, f64::from(input.width) + pad - cx];
    let ys = [-pad - cy, f64::from(input.height) + pad - cy];
    finite(&[inner, outer, cx, cy, pad, xs[0], xs[1], ys[0], ys[1]])?;
    let outside = |angle: f64| {
        let offset = (angle - input.view.angle + PI).rem_euclid(2. * PI) - PI;
        gated || offset.abs() > half_arc
    };
    let visible = |angle: f64, radius: f64| {
        let x = angle.cos() * radius;
        let y = angle.sin() * radius;
        x >= xs[0] && x <= xs[1] && y >= ys[0] && y <= ys[1] && outside(angle)
    };
    // An uncovered component either crosses a rectangle edge, contains a corner
    // in the annulus, or contains the antipode of the retained local arc. Check
    // both band radii, including the stroke fringe; no diagonal heuristic.
    for radius in [inner, outer] {
        for angle in [input.view.angle, input.view.angle + PI] {
            if visible(angle, radius) {
                return Ok(true);
            }
        }
        if radius == 0. {
            continue;
        }
        for x in xs {
            if x.abs() <= radius {
                let angle = (x / radius).acos();
                for angle in [angle, -angle] {
                    // The intersected coordinate is exact; avoid a trig roundoff
                    // moving a rectangle intersection just outside its own edge.
                    let y = angle.sin() * radius;
                    if y >= ys[0] && y <= ys[1] && outside(angle) {
                        return Ok(true);
                    }
                }
            }
        }
        for y in ys {
            if y.abs() <= radius {
                let angle = (y / radius).asin();
                for angle in [angle, PI - angle] {
                    let x = angle.cos() * radius;
                    if x >= xs[0] && x <= xs[1] && outside(angle) {
                        return Ok(true);
                    }
                }
            }
        }
    }
    for x in xs {
        for y in ys {
            let distance = x.hypot(y);
            if distance >= inner && distance <= outer && outside(y.atan2(x)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Plan both complete paths before submission, or omit them at the strict distance gate.
/// This has no quality, visibility, connection, lifecycle or update-clock input.
pub fn plan_arena_edge(input: ArenaEdgeInput) -> Result<Option<ArenaEdgePlan>, PresentationError> {
    finite(&[input.camera.x, input.camera.y, input.zoom, input.radius])?;
    if input.width == 0 || input.height == 0 || input.zoom <= 0. || input.radius < 0. {
        return Err(PresentationError);
    }
    let line_width = 23. * input.zoom;
    finite(&[line_width])?;
    if (input.radius - input.view.distance).abs() >= 4000. {
        return Ok(None);
    }
    let mut plan = ArenaEdgePlan {
        fill: [Point::default(); 83],
        stroke: [Point::default(); 42],
        line_width,
    };
    let project = |angle: f64, radius| -> Result<Point, PresentationError> {
        let ax = input.view.center + angle.cos() * radius;
        let ay = input.view.center + angle.sin() * radius;
        let x = f64::from(input.width) / 2. + (ax - input.camera.x) * input.zoom;
        let y = f64::from(input.height) / 2. + (ay - input.camera.y) * input.zoom;
        finite(&[ax, ay, x, y])?;
        Ok(Point { x, y })
    };
    for pass in 0..2 {
        let mut k = input.radius;
        // Division by computed zero intentionally selects this fallback.
        if 4000. / input.radius > 2. * PI {
            k = 4000. / (2. * PI);
        }
        let points = if pass == 0 {
            &mut plan.fill[..]
        } else {
            &mut plan.stroke[..]
        };
        points[0] = project(input.view.angle - 2000. / k, input.radius)?;
        for (index, j) in (-2000..=2000).step_by(100).enumerate() {
            points[index + 1] = project(input.view.angle + f64::from(j) / k, input.radius)?;
        }
        if pass == 0 {
            for index in 0..=40 {
                let j = 2000 - index as i32 * 100;
                points[index + 42] =
                    project(input.view.angle + f64::from(j) / k, input.radius + 4000.)?;
            }
        }
    }
    Ok(Some(plan))
}
