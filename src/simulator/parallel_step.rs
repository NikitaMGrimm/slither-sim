//! The step's per-snake phases on the current rayon pool. Each function
//! returns exactly what its serial counterpart in `advance` would: tasks only
//! read shared state or write their own slots, and results merge in a fixed
//! order, so the world never depends on the pool size or on scheduling.
use super::*;
use crate::parallel::{even, map_ranges};
use rayon::prelude::*;
use std::ops::Range;

/// Mutable per-slot state that motion writes.
pub(super) struct MotionSlots<'a> {
    pub(super) snakes: &'a mut [Snake],
    pub(super) steering: &'a mut [SteeringCache],
    pub(super) heads: &'a mut [(Point, (f64, f64), f64)],
    pub(super) pit_entered: &'a mut [bool],
    pub(super) body_bounds: &'a mut [Option<Bounds>],
    pub(super) kiosks: &'a mut [KioskState],
}

/// Move every living snake. Boost drops draw from the world RNG and append
/// food, so they stay in slot order in a first serial pass. Motion then reads
/// and writes only its own slot, so splitting the passes keeps every value
/// identical. Wall and pit deaths come back in ascending slot order, as the
/// serial loop pushes them.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn movement(
    parts: usize,
    motion: &Motion<'_>,
    drops: &mut Drops<'_>,
    actions: &[Option<Action>],
    alive: &[usize],
    previous: &[Point],
    scales: &mut Vec<f64>,
    boosts: &mut [Boost],
    speeds: &mut [SpeedState],
    slots: MotionSlots<'_>,
    boundary_deaths: &mut Vec<(usize, DeathReason)>,
) {
    scales.resize(slots.snakes.len(), 0.0);
    for &i in alive {
        let a = actions[i].expect("validated action");
        let s = &mut slots.snakes[i];
        let body_len = s.body.len();
        scales[i] = drops.boost(
            motion.config,
            i,
            a.boost,
            s,
            &mut boosts[i],
            &mut speeds[i],
            motion.floor,
            motion.pit_radius,
            motion.radius,
        );
        // Like the combined serial pass in Simulator::advance, invalidate on
        // tail removal. Check boost separately because motion may append an
        // anchor; that append also invalidates, even if total length is equal.
        if s.body.len() != body_len {
            slots.body_bounds[i] = None;
        }
    }
    let MotionSlots {
        mut snakes,
        mut steering,
        mut heads,
        mut pit_entered,
        mut body_bounds,
        mut kiosks,
    } = slots;
    // Slot ranges of roughly equal living counts; dead slots cost nothing.
    let living: Vec<bool> = snakes.iter().map(|s| s.alive).collect();
    let ranges = crate::parallel::balanced(&living, parts);
    let mut pieces = Vec::with_capacity(ranges.len());
    for range in &ranges {
        let n = range.len();
        let (s, rest) = snakes.split_at_mut(n);
        snakes = rest;
        let (st, rest) = steering.split_at_mut(n);
        steering = rest;
        let (h, rest) = heads.split_at_mut(n);
        heads = rest;
        let (p, rest) = pit_entered.split_at_mut(n);
        pit_entered = rest;
        let (b, rest) = body_bounds.split_at_mut(n);
        body_bounds = rest;
        let (k, rest) = kiosks.split_at_mut(n);
        kiosks = rest;
        pieces.push((range.clone(), s, st, h, p, b, k));
    }
    let deaths: Vec<Vec<(usize, DeathReason)>> = pieces
        .into_par_iter()
        .map(|(range, s, st, h, p, b, k)| {
            let mut deaths = Vec::new();
            for (n, i) in range.enumerate() {
                let snake = &mut s[n];
                if !snake.alive {
                    continue;
                }
                let a = actions[i].expect("validated action");
                let body_len = snake.body.len();
                if let Some(reason) = motion.advance(
                    a.heading,
                    previous[i],
                    scales[i],
                    snake,
                    &mut st[n],
                    &mut h[n].1,
                    &mut p[n],
                    &mut b[n],
                    &mut k[n],
                ) {
                    deaths.push((i, reason));
                }
                // The second half of the serial pass's invalidation invariant:
                // kiosk tail removal must also discard the cached bounds.
                if snake.body.len() != body_len {
                    b[n] = None;
                }
            }
            deaths
        })
        .collect();
    boundary_deaths.extend(deaths.into_iter().flatten());
}

/// Resolve collisions with every head's search done in parallel first.
/// Searches only read this tick's geometry, so each head searches as if nothing
/// had fallen. The walk keeps the serial resolution: a first strike on a body
/// that already fell earlier in the walk repeats that head's search with fallen
/// bodies skipped, which is exactly the serial search, since every earlier
/// candidate failed regardless of which bodies fell.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn collisions<const PROFILE: bool>(
    parts: usize,
    scene: &Collisions<'_>,
    boundary_deaths: &[(usize, DeathReason)],
    protected: usize,
    dead: &mut Vec<(usize, Option<usize>, DeathReason)>,
    fallen: &mut [bool],
    strikes: &mut Vec<Option<usize>>,
    candidates: &mut Vec<u32>,
    profile: &mut StepProfile,
) {
    first_strikes::<PROFILE>(parts, scene, strikes, profile);
    resolve_collisions(
        scene.order,
        boundary_deaths,
        protected,
        dead,
        fallen,
        |k, i, fallen| match strikes[k] {
            Some(j) if fallen[j] => {
                scene.first_strike::<PROFILE>(i, |j| fallen[j], candidates, profile)
            }
            first => first,
        },
    );
}

/// Every head's first strike, by collision order position, ignoring which
/// bodies fall this tick. Profile counters sum the searches actually run.
fn first_strikes<const PROFILE: bool>(
    parts: usize,
    scene: &Collisions<'_>,
    strikes: &mut Vec<Option<usize>>,
    profile: &mut StepProfile,
) {
    strikes.clear();
    strikes.resize(scene.order.len(), None);
    let ranges = even(scene.order.len(), parts);
    let counts = map_ranges(
        &ranges,
        strikes,
        &|range: Range<usize>, out: &mut [Option<usize>]| {
            let mut candidates = Vec::new();
            let mut counts = StepProfile::default();
            for (k, strike) in range.zip(out) {
                *strike = scene.first_strike::<PROFILE>(
                    scene.order[k],
                    |_| false,
                    &mut candidates,
                    &mut counts,
                );
            }
            counts
        },
    );
    if PROFILE {
        for counts in counts {
            profile.body_pairs += counts.body_pairs;
            profile.segment_tests += counts.segment_tests;
        }
    }
}

/// `indexed_hits` over slices of the occupied cells; the caller sorts.
#[inline(never)]
pub(super) fn indexed_hits<const PROFILE: bool>(
    parts: usize,
    grid: &EaterGrid,
    pellets: &PelletIndex,
    snakes: &[Snake],
    eaters: &[(usize, Point, f64, Bounds)],
    hits: &mut Vec<(usize, usize)>,
    profile: &mut StepProfile,
) {
    let found: Vec<_> = even(grid.occupied_len(), parts)
        .into_par_iter()
        .map(|cells| {
            let mut hits = Vec::new();
            let mut counts = StepProfile::default();
            super::indexed_hits::<PROFILE>(
                grid,
                pellets,
                snakes,
                eaters,
                cells,
                &mut hits,
                &mut counts,
            );
            (hits, counts)
        })
        .collect();
    for (found, counts) in found {
        hits.extend(found);
        if PROFILE {
            profile.food_pairs += counts.food_pairs;
            profile.pickup_tests += counts.pickup_tests;
        }
    }
}
