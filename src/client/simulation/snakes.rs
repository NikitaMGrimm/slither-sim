//! Snake report state, coverage candidates and ordered wire diffs.
use super::{DeliveryProfile, PlayerAppearance};
use crate::{
    client::encode,
    model::{Point, Snake},
    physics,
};
use std::{collections::HashMap, f64::consts::TAU};

/// Front anchors searched for a known snake's previous front. Contraction runs
/// before each insertion from index 3 on, so that front stays unchanged at
/// indices 1, 2 and 3 through three insertions and moves on the fourth: up to
/// `FRESH_ANCHORS - 1` new anchors encode incrementally, and more resend the
/// snake in full.
pub(super) const FRESH_ANCHORS: usize = 4;
/// Release rules from the [scheduler study] (2026-10-25; 1.93 million
/// retained frames): body anchors leave in the tick that makes them.
/// A snake's motion (heading, wanted heading,
/// speed) is reported when it changed, at most every six ~7 ms ticks: busy
/// gaps sit near 42 ms, a straight snake stays silent for seconds and one
/// snake's reports share a phase across recipients. Changed mass follows its
/// own ~105 ms opportunity. The exact comparators and rearm rules are not
/// identified; these minimum gaps are model choices. Clean [near-server data]
/// includes 24 ms motion, 84 ms mass and motion-before-mass counterexamples;
/// decoder regressions accept them without changing these release rules.
///
/// [scheduler study]: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-packet-scheduler/REPORT.md
/// [near-server data]: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-vps-timing/REPORT.md
pub(super) const MOTION_REPORT_TICKS: u64 = 6;
pub(super) const FAM_REPORT_TICKS: u64 = 15;

/// What one connection has been told about a snake, in wire terms.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct KnownSnake {
    pub(super) life: u64,
    pub(super) front: Point,
    pub(super) len: usize,
    pub(super) heading: u8,
    pub(super) wanted: u8,
    pub(super) speed: u8,
    pub(super) fam: u32,
    pub(super) kills: usize,
}

/// Reuse a delta only within one broadcast, for the same snake and identical
/// recipient history. Own-snake packets are never shared. Keeping just the
/// last history bounds storage even with divergent viewers.
#[derive(Clone, Default)]
pub(super) struct SnakeDeltaCache(Option<SnakeDelta>);

#[derive(Clone)]
struct SnakeDelta {
    before: KnownSnake,
    after: KnownSnake,
    packets: Vec<Vec<u8>>,
}

impl SnakeDeltaCache {
    #[expect(clippy::too_many_arguments, reason = "Same explicit inputs as changes")]
    pub(super) fn changes(
        &mut self,
        packets: &mut Vec<Vec<u8>>,
        own: bool,
        id: u16,
        known: &mut KnownSnake,
        snake: &Snake,
        wanted: f64,
        profile: &PlayerAppearance,
        reports: &SnakeReports,
    ) -> Result<(), String> {
        if own {
            return changes(packets, own, id, known, snake, wanted, profile, reports);
        }
        let entry = &mut self.0;
        if let Some(delta) = entry.as_ref().filter(|delta| delta.before == *known) {
            packets.extend_from_slice(&delta.packets);
            known.clone_from(&delta.after);
        } else {
            let before = known.clone();
            let start = packets.len();
            changes(packets, own, id, known, snake, wanted, profile, reports)?;
            *entry = Some(SnakeDelta {
                before,
                after: known.clone(),
                packets: packets[start..].to_vec(),
            });
        }
        Ok(())
    }
}

/// One snake's report opportunities, shared by every recipient. Recipients
/// differ only in which fields they still lack.
#[derive(Clone, Debug, Default)]
pub(super) struct SnakeReports {
    pub(super) life: u64,
    /// Heading, wanted heading and speed bytes last reported.
    pub(super) motion: [u8; 3],
    pub(super) motion_at: u64,
    pub(super) fam: u32,
    pub(super) fam_at: u64,
    /// Reported in the current tick.
    pub(super) motion_due: bool,
    pub(super) fam_due: bool,
    /// Some recipient holds a value other than the last report, from an
    /// appearance or a growth/tail mass; the next opportunity reports even
    /// if the snake returned to that value.
    pub(super) motion_stale: bool,
    pub(super) fam_stale: bool,
}

impl KnownSnake {
    /// Shown in full, including its complete motion.
    pub(super) fn new(life: u64, snake: &Snake, wanted: f64) -> Result<Self, String> {
        Ok(Self {
            life,
            front: snake.body[0],
            len: snake.body.len(),
            heading: encode::angle_byte(snake.heading),
            wanted: encode::angle_byte(wanted),
            speed: encode::speed_byte(snake.speed)?,
            fam: encode::fam_u24(snake.fam),
            kills: snake.kills,
        })
    }
}

impl SnakeReports {
    /// Open this tick's opportunities for a living snake. A new life starts
    /// from the state its appearance carries.
    pub(super) fn update(
        &mut self,
        life: u64,
        motion: [u8; 3],
        fam: u32,
        tick: u64,
        delivery: DeliveryProfile,
    ) {
        if self.life != life {
            *self = Self {
                life,
                motion,
                motion_at: tick,
                fam,
                fam_at: tick,
                ..Self::default()
            };
            return;
        }
        // Our responsive design reflects wanted-heading/speed changes promptly;
        // the stock client already turns toward a held target between reports.
        // Standalone mass drives its smoothed length and own score display.
        // See the delivery-profiles study (2026-10-25), not an official rule:
        // https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-delivery-profiles/REPORT.md
        let responsive = delivery == DeliveryProfile::Responsive;
        let urgent = responsive && motion[1..] != self.motion[1..];
        self.motion_due =
            (motion != self.motion || self.motion_stale) && (urgent || tick >= self.motion_at);
        if self.motion_due {
            self.motion = motion;
            self.motion_at = tick + MOTION_REPORT_TICKS;
            self.motion_stale = false;
        }
        self.fam_due = (fam != self.fam || self.fam_stale) && (responsive || tick >= self.fam_at);
        if self.fam_due {
            self.fam = fam;
            self.fam_at = tick + FAM_REPORT_TICKS;
            self.fam_stale = false;
        }
    }

    /// Note a recipient's view of this snake after its packets this tick.
    pub(super) fn observe(&mut self, known: &KnownSnake) {
        self.motion_stale |= [known.heading, known.wanted, known.speed] != self.motion;
        self.fam_stale |= known.fam != self.fam;
    }
}

/// What one connection has been told about each snake slot.
#[derive(Clone, Debug, Default)]
pub(super) struct KnownSnakes(Vec<Option<KnownSnake>>);
impl KnownSnakes {
    pub(super) fn contains_key(&self, slot: usize) -> bool {
        self.0.get(slot).is_some_and(Option::is_some)
    }
    pub(super) fn get_mut(&mut self, slot: usize) -> Option<&mut KnownSnake> {
        self.0.get_mut(slot)?.as_mut()
    }
    pub(super) fn remove(&mut self, slot: usize) -> Option<KnownSnake> {
        self.0.get_mut(slot)?.take()
    }
    pub(super) fn insert(&mut self, slot: usize, known: KnownSnake) {
        if self.0.len() <= slot {
            self.0.resize(slot + 1, None);
        }
        self.0[slot] = Some(known);
    }
    pub(super) fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(slot, known)| known.as_ref().map(|_| slot))
    }
}

/// Living snakes listed by every grid cell their widened bounds overlap.
/// `covered` rejects a head outside a snake's bounds widened by the vision
/// reach, so the cell holding a head lists every snake that can cover it.
/// Testing all snakes for each of ~500 connections every tick cost ~2.3 ms.
pub(super) struct CoverageGrid {
    cell: f64,
    cells: HashMap<(i64, i64), Vec<usize>>,
}
impl CoverageGrid {
    pub(super) fn new(snakes: &[Snake], bounds: &[[f64; 4]], vision: f64) -> Self {
        let reach = vision + 1.;
        let mut grid = Self {
            cell: reach.max(256.),
            cells: HashMap::new(),
        };
        for (slot, &[x0, y0, x1, y1]) in bounds.iter().enumerate() {
            if !snakes[slot].alive {
                continue;
            }
            let (left, top) = grid.key(x0 - reach, y0 - reach);
            let (right, bottom) = grid.key(x1 + reach, y1 + reach);
            for x in left..=right {
                for y in top..=bottom {
                    grid.cells.entry((x, y)).or_default().push(slot);
                }
            }
        }
        grid
    }

    fn key(&self, x: f64, y: f64) -> (i64, i64) {
        (
            (x / self.cell).floor() as i64,
            (y / self.cell).floor() as i64,
        )
    }

    /// Ascending slots of the snakes that may cover `point`.
    pub(super) fn near(&self, point: Point) -> &[usize] {
        self.cells
            .get(&self.key(point.x, point.y))
            .map_or(&[], Vec::as_slice)
    }
}

/// Coverage: any centerline part within `vision` of `head`.
pub(super) fn covered(head: Point, snake: &Snake, [x0, y0, x1, y1]: [f64; 4], vision: f64) -> bool {
    let reach = vision + 1.;
    if head.x < x0 - reach || head.x > x1 + reach || head.y < y0 - reach || head.y > y1 + reach {
        return false;
    }
    snake.head.distance(head) <= vision
        || snake
            .body
            .first()
            .is_some_and(|&point| physics::segment_distance(head, snake.head, point) <= vision)
        || snake
            .body
            .windows(2)
            .any(|pair| physics::segment_distance(head, pair[0], pair[1]) <= vision)
}

/// Full snake, then its complete motion with the turn direction: the stock
/// appearance handler leaves the heading at the last body anchor's direction.
/// A nonzero kill count follows, since appearances do not carry it.
pub(super) fn full(
    packets: &mut Vec<Vec<u8>>,
    own: bool,
    id: u16,
    snake: &Snake,
    wanted: f64,
    profile: &PlayerAppearance,
) -> Result<(), String> {
    packets.push(encode::appearance(id, snake, wanted, profile)?);
    packets.extend(encode::motion(
        own,
        id,
        Some(encode::angle_byte(snake.heading)),
        Some(encode::angle_byte(wanted)),
        Some(encode::speed_byte(snake.speed)?),
        encode::direction(snake.heading, wanted),
    ));
    if snake.kills > 0 {
        packets.push(encode::kills(id, snake.kills));
    }
    Ok(())
}

/// Incremental body, mass and motion reports for a known snake. Body changes
/// go out in the tick that makes them; motion and standalone mass wait for the
/// snake's shared opportunities in `reports`. We adopt the common observed
/// order: tail/standalone mass, motion, then anchors. The scheduler study linked
/// from `MOTION_REPORT_TICKS` retains 27 reverse mass/motion pairs; this is not a
/// universal official sort.
#[expect(
    clippy::too_many_arguments,
    reason = "Keep wire inputs explicit without a per-snake context"
)]
pub(super) fn changes(
    packets: &mut Vec<Vec<u8>>,
    own: bool,
    id: u16,
    known: &mut KnownSnake,
    snake: &Snake,
    wanted: f64,
    profile: &PlayerAppearance,
    reports: &SnakeReports,
) -> Result<(), String> {
    let body = &snake.body;
    let fam = encode::fam_u24(snake.fam);
    let mut fam_sent = false;
    let change = body.len() as isize - known.len as isize;
    match body
        .iter()
        .take(FRESH_ANCHORS)
        .position(|&point| point == known.front)
        .filter(|&fresh| change <= fresh as isize)
    {
        Some(fresh) => {
            // Growth and tail removal commute geometrically: contraction never
            // reads the tail, so report removals first, then the new anchors.
            let grown = change.clamp(0, fresh as isize);
            for _ in 0..grown - change {
                packets.push(encode::tail(id, fam));
                fam_sent = true;
            }
            // Growth anchors carry the mass themselves.
            if !fam_sent && grown == 0 && reports.fam_due && known.fam != fam {
                packets.push(encode::fam(id, fam));
                fam_sent = true;
            }
            if reports.motion_due {
                let [heading, target, speed] = reports.motion;
                packets.extend(encode::motion(
                    own,
                    id,
                    (heading != known.heading).then_some(heading),
                    (target != known.wanted).then_some(target),
                    (speed != known.speed).then_some(speed),
                    encode::direction(snake.heading, wanted),
                ));
                known.heading = heading;
                known.wanted = target;
                known.speed = speed;
            }
            for j in (0..fresh).rev() {
                let grows = (j as isize) < grown;
                let (point, previous) = (body[j], body[j + 1]);
                let angle = encode::angle_u16((point.y - previous.y).atan2(point.x - previous.x));
                let radians = f64::from(angle) * TAU / 65536.;
                let step = Point {
                    x: previous.x + radians.cos() * physics::ANCHOR_SPACING,
                    y: previous.y + radians.sin() * physics::ANCHOR_SPACING,
                };
                let carried = grows.then_some(fam);
                fam_sent |= grows;
                packets.push(if step.distance(point) < 1e-6 {
                    encode::anchor(own, id, angle, carried)
                } else {
                    encode::anchor_at(own, id, angle, point, carried)?
                });
            }
        }
        None => {
            // Unrecognizable continuity (e.g. an externally edited body): resend.
            full(packets, own, id, snake, wanted, profile)?;
            *known = KnownSnake::new(known.life, snake, wanted)?;
            return Ok(());
        }
    }
    known.front = body[0];
    known.len = body.len();
    if fam_sent {
        known.fam = fam;
    }
    if known.kills != snake.kills {
        known.kills = snake.kills;
        packets.push(encode::kills(id, snake.kills));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, Simulator};

    #[test]
    fn shared_deltas_preserve_distinct_histories_and_own_packet_types() {
        let world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["snake".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut detached_snake = world.snakes()[0].clone();
        let snake = &mut detached_snake;
        let before = KnownSnake::new(2, snake, 0.).unwrap();
        let next = Point {
            x: snake.body[0].x + 42.,
            y: snake.body[0].y,
        };
        physics::append_anchor(&mut snake.body, next, false);
        snake.head = next;
        snake.fam = 0.5;
        snake.kills = 3;
        let reports = SnakeReports {
            motion: [7, 9, 13],
            motion_due: true,
            fam_due: true,
            ..Default::default()
        };
        let mut cache = SnakeDeltaCache::default();
        let mut histories = vec![before.clone()];
        let mut stale = before.clone();
        stale.heading = 7;
        stale.fam = encode::fam_u24(0.5);
        histories.push(stale);
        let mut missing_front = before;
        missing_front.front = Point { x: 1e6, y: 1e6 };
        histories.push(missing_front);
        for history in histories {
            // Repeat peer encodings to exercise hits, with an own update between.
            for own in [false, false, true, false] {
                let (mut expected, mut actual) = (vec![vec![99]], vec![vec![99]]);
                let (mut reference, mut shared) = (history.clone(), history.clone());
                changes(
                    &mut expected,
                    own,
                    4,
                    &mut reference,
                    snake,
                    0.7,
                    &PlayerAppearance::default(),
                    &reports,
                )
                .unwrap();
                cache
                    .changes(
                        &mut actual,
                        own,
                        4,
                        &mut shared,
                        snake,
                        0.7,
                        &PlayerAppearance::default(),
                        &reports,
                    )
                    .unwrap();
                assert_eq!(actual, expected);
                assert_eq!(shared, reference);
            }
        }
    }
}
