// Copied ONLY into a prepared snapshot, never compiled by the shipped crate.
// Counters and clocks perturb execution. These are diagnostic inclusive wall
// intervals, not comparative timings, allocation counts, or a cost model.
use std::{cell::RefCell, time::Instant};

#[derive(Clone, Copy)]
pub(super) enum Phase {
    Batch,
    Index,
    FoodFilter,
    FoodOrder,
    FoodCopy,
    FoodLinearFilterCopy,
    OwnCopy,
    PreyFilterCopy,
    OpponentFilterCopy,
}

#[derive(Clone, Copy)]
pub(super) enum Count {
    IndexSnakes,
    IndexAnchors,
    IndexFood,
    ObservedSlots,
    IndexedObservations,
    LinearObservations,
    FoodCandidates,
    FoodAccepted,
    FoodCopies,
    OwnCopiedAnchors,
    PreyCandidates,
    PreyAccepted,
    PreyCopies,
    OpponentCandidates,
    OpponentHeadTests,
    OpponentAnchorTests,
    OpponentAccepted,
    OpponentCopiedAnchors,
    EmptyBatches,
}

#[derive(Default)]
struct Profile {
    seconds: [f64; 9],
    calls: [u64; 9],
    counts: [u64; 19],
}

// No worker threads, persistent caches, atomics, or changes to Simulator state.
// The generated single-thread harness activates this only around its timed loop.
thread_local! {
    static PROFILE: RefCell<Option<Profile>> = const { RefCell::new(None) };
}

pub(super) struct Clock(Phase, Option<Instant>);

impl Clock {
    pub(super) fn new(phase: Phase) -> Self {
        Self(
            phase,
            PROFILE.with(|p| p.borrow().as_ref().map(|_| Instant::now())),
        )
    }
}

impl Drop for Clock {
    fn drop(&mut self) {
        if let Some(start) = self.1 {
            let seconds = start.elapsed().as_secs_f64();
            PROFILE.with(|p| {
                if let Some(p) = p.borrow_mut().as_mut() {
                    p.seconds[self.0 as usize] += seconds;
                    p.calls[self.0 as usize] += 1;
                }
            });
        }
    }
}

pub(super) fn add(counts: &[(Count, usize)]) {
    PROFILE.with(|p| {
        if let Some(p) = p.borrow_mut().as_mut() {
            for &(key, value) in counts {
                p.counts[key as usize] += value as u64;
            }
        }
    });
}

/// Snapshot-only diagnostic API. Trace mode never creates a session.
pub struct Session;

impl Session {
    pub fn start() -> Self {
        PROFILE.with(|p| {
            assert!(
                p.borrow().is_none(),
                "nested phase sessions are unsupported"
            );
            *p.borrow_mut() = Some(Profile::default());
        });
        Self
    }

    pub fn finish(self) -> serde_json::Value {
        let p = PROFILE.with(|p| p.borrow_mut().take().expect("active phase session"));
        let phase_names = [
            "batch_inclusive",
            "index_setup",
            "indexed_food_candidate_filter",
            "accepted_food_index_order",
            "indexed_food_copies",
            "linear_food_filter_and_copies",
            "own_copy",
            "prey_filter_and_copies",
            "opponent_filter_and_copies",
        ];
        let count_names = [
            "index_snakes",
            "index_body_anchor_visits",
            "index_food_visits",
            "observed_slots",
            "indexed_observations",
            "linear_observations",
            "food_candidate_visits",
            "food_accepted",
            "food_copies",
            "own_copied_body_anchors",
            "prey_candidate_visits",
            "prey_accepted",
            "prey_copies",
            "opponent_slot_candidate_visits",
            "opponent_head_tests",
            "opponent_body_anchor_tests",
            "opponents_accepted",
            "opponent_copied_body_anchors",
            "early_empty_batches",
        ];
        let phases: std::collections::BTreeMap<_, _> = phase_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                (
                    *name,
                    serde_json::json!({
                        "wallSeconds": p.seconds[i], "calls": p.calls[i],
                    }),
                )
            })
            .collect();
        let counts: std::collections::BTreeMap<_, _> = count_names
            .iter()
            .enumerate()
            .map(|(i, name)| (*name, p.counts[i]))
            .collect();
        serde_json::json!({
            "format": "slither-observation-phases-v1",
            "phases": phases, "counts": counts,
            "scope": "actual observation code; inclusive batch wall intervals contain nested phases, counter/clock overhead, stable-slot allocation, internal temporary/index destruction; detached output destruction excluded",
            "limits": "diagnostic only, separate from comparative timings; do not sum batch with nested phases; gaps include bookkeeping; prey/opponent and linear food predicates/copies remain interleaved; counters count visits/accepted records/copied body anchors, not allocations or unique entities; no internal clocks in trace mode",
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        PROFILE.with(|p| *p.borrow_mut() = None);
    }
}
