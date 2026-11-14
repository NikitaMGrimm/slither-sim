//! Host bot groups: which reserved bot slot plays for which group, and how
//! fast missing bots join. The simulation thread owns one roster and carries
//! out its changes; the roster never touches the world, so it is tested alone.
use super::BotBehavior;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Longest group name; names are typed in the host console.
const MAX_NAME: usize = 24;

/// A named set of host bots with one behavior.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BotGroup {
    pub name: String,
    pub behavior: BotBehavior,
    pub count: usize,
}

impl BotGroup {
    /// A group named after its behavior.
    pub fn new(behavior: BotBehavior, count: usize) -> Self {
        Self {
            name: behavior.name().into(),
            behavior,
            count,
        }
    }
}

/// A running host's bot groups; see `RunningHost::bots`.
#[derive(Clone, Debug, PartialEq)]
pub struct BotStatus {
    /// In configuration and creation order.
    pub groups: Vec<BotGroupStatus>,
    /// Reserved bot slots.
    pub capacity: usize,
    /// Slots not claimed by any group's target.
    pub free: usize,
    pub join_per_second: Option<f64>,
}

impl BotStatus {
    /// Bots in a group, alive or not.
    pub fn assigned(&self) -> usize {
        self.groups.iter().map(|group| group.assigned).sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BotGroupStatus {
    pub name: String,
    pub behavior: BotBehavior,
    /// Wanted bots.
    pub target: usize,
    /// Bots in the group: alive, dead and respawning, or waiting to join.
    pub assigned: usize,
    /// Bots with a living snake.
    pub alive: usize,
}

/// Group names are typed in the console, so they are short words.
pub(super) fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > MAX_NAME || !name.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(format!(
            "Bot group names need 1 to {MAX_NAME} printable ASCII characters without spaces"
        ));
    }
    Ok(())
}

pub(super) fn check_rate(rate: Option<f64>) -> Result<(), String> {
    match rate {
        Some(rate) if !rate.is_finite() || rate <= 0. => {
            Err("Bot join rate must be a positive, finite number of bots per second".into())
        }
        _ => Ok(()),
    }
}

/// Host facts that limit which behaviors may play.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Rules {
    pub owner_key: bool,
    /// Circular worlds queue every join for socket priority and growth holds;
    /// world-thread respawns would bypass that admission queue.
    pub circular: bool,
}

impl Rules {
    pub(super) fn check(
        &self,
        name: &str,
        behavior: BotBehavior,
        count: usize,
    ) -> Result<(), String> {
        if behavior == BotBehavior::Follow && !self.owner_key {
            return Err(format!("Bot group {name}: follow requires ownerKey"));
        }
        if behavior != BotBehavior::Forager && count > 0 && self.circular {
            return Err(format!(
                "Bot group {name}: center and follow need a world without circular spawning, \
                 such as official; Battledome hosts use forager bots"
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Use {
    Free,
    Bot(BotBehavior),
    /// A removed forager whose worker has not yet confirmed it stopped.
    Releasing,
}

struct Group {
    name: String,
    behavior: BotBehavior,
    target: usize,
    /// Bot indices, so removal takes the highest first.
    bots: BTreeSet<usize>,
}

/// One slot change for the simulation thread to carry out. `bot` counts from
/// the first bot slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
    Assign { bot: usize, behavior: BotBehavior },
    Unassign { bot: usize, behavior: BotBehavior },
}

/// Group targets and the assignment of reserved bot slots to groups.
pub(super) struct Roster {
    groups: Vec<Group>,
    uses: Vec<Use>,
    free: BTreeSet<usize>,
    rate: Option<f64>,
    /// Bots that may join now under `rate`.
    allowance: f64,
    /// World time of the last reconcile.
    clock: Option<f64>,
    /// The group that gets the next missing bot when several lack some.
    cursor: usize,
    rules: Rules,
}

impl Roster {
    /// Groups must already be validated; groups without bots are dropped.
    pub(super) fn new(
        groups: &[BotGroup],
        capacity: usize,
        rate: Option<f64>,
        rules: Rules,
    ) -> Self {
        Self {
            groups: groups
                .iter()
                .filter(|group| group.count > 0)
                .map(|group| Group {
                    name: group.name.clone(),
                    behavior: group.behavior,
                    target: group.count,
                    bots: BTreeSet::new(),
                })
                .collect(),
            uses: vec![Use::Free; capacity],
            free: (0..capacity).collect(),
            rate,
            allowance: 1.,
            clock: None,
            cursor: 0,
            rules,
        }
    }

    pub(super) fn capacity(&self) -> usize {
        self.uses.len()
    }

    /// Bots in a group, alive or not.
    pub(super) fn assigned(&self) -> usize {
        self.groups.iter().map(|group| group.bots.len()).sum()
    }

    /// The behavior a bot slot currently plays, if a group holds it.
    pub(super) fn behavior(&self, bot: usize) -> Option<BotBehavior> {
        match self.uses.get(bot) {
            Some(Use::Bot(behavior)) => Some(*behavior),
            _ => None,
        }
    }

    /// Create, resize or (with 0) remove a group. A new group needs a behavior
    /// unless its name is one.
    pub(super) fn set_group(
        &mut self,
        name: &str,
        behavior: Option<BotBehavior>,
        count: usize,
    ) -> Result<(), String> {
        check_name(name)?;
        let existing = self.groups.iter().position(|group| group.name == name);
        let behavior = match existing {
            Some(index) => {
                let current = self.groups[index].behavior;
                if behavior.is_some_and(|wanted| wanted != current) {
                    return Err(format!(
                        "Bot group {name} plays {current}; set it to 0 and add a new group for another behavior"
                    ));
                }
                current
            }
            None if count == 0 => return Err(format!("No bot group named {name}")),
            None => match behavior {
                Some(behavior) => behavior,
                None => name.parse().map_err(|_| {
                    format!("New bot group {name} needs a behavior: forager, center or follow")
                })?,
            },
        };
        self.rules.check(name, behavior, count)?;
        let others: usize = self
            .groups
            .iter()
            .filter(|group| group.name != name)
            .map(|group| group.target)
            .sum();
        // Other targets never exceed capacity, so this cannot overflow.
        if count > self.capacity() - others {
            return Err(format!(
                "Bot groups would want {} bots, above bots.capacity {}; raising it needs a restart",
                others.saturating_add(count),
                self.capacity()
            ));
        }
        match existing {
            Some(index) => self.groups[index].target = count,
            None => self.groups.push(Group {
                name: name.into(),
                behavior,
                target: count,
                bots: BTreeSet::new(),
            }),
        }
        Ok(())
    }

    pub(super) fn set_rate(&mut self, rate: Option<f64>) -> Result<(), String> {
        check_rate(rate)?;
        self.rate = rate;
        self.allowance = 1.;
        Ok(())
    }

    /// A worker confirmed a removed forager stopped; its slot is free again.
    pub(super) fn release(&mut self, bot: usize) -> bool {
        if self.uses.get(bot) != Some(&Use::Releasing) {
            return false;
        }
        self.uses[bot] = Use::Free;
        self.free.insert(bot);
        true
    }

    /// Move assignments toward the targets at world time `now`. Surplus bots
    /// leave at once, highest slots first; missing bots take the lowest free
    /// slots, round-robin over groups in order, as fast as the rate allows.
    pub(super) fn reconcile(&mut self, now: f64) -> Vec<Change> {
        let mut changes = Vec::new();
        for group in &mut self.groups {
            while group.bots.len() > group.target {
                let bot = group.bots.pop_last().unwrap();
                if group.behavior == BotBehavior::Forager {
                    self.uses[bot] = Use::Releasing;
                } else {
                    self.uses[bot] = Use::Free;
                    self.free.insert(bot);
                }
                changes.push(Change::Unassign {
                    bot,
                    behavior: group.behavior,
                });
            }
        }
        self.groups.retain(|group| group.target > 0);
        if let Some(rate) = self.rate {
            let elapsed = self.clock.map_or(0., |clock| (now - clock).max(0.));
            // At most one bot plus this step's share: a target raised after a
            // quiet hour still ramps instead of joining in a burst.
            self.allowance = (self.allowance + elapsed * rate).min(1. + elapsed * rate);
        }
        self.clock = Some(now);
        let groups = self.groups.len();
        while self.rate.is_none() || self.allowance >= 1. {
            let Some(&bot) = self.free.first() else {
                break;
            };
            let Some(index) = (0..groups)
                .map(|offset| (self.cursor + offset) % groups)
                .find(|&index| self.groups[index].bots.len() < self.groups[index].target)
            else {
                break;
            };
            self.free.remove(&bot);
            let group = &mut self.groups[index];
            group.bots.insert(bot);
            self.uses[bot] = Use::Bot(group.behavior);
            self.cursor = (index + 1) % groups;
            if self.rate.is_some() {
                self.allowance -= 1.;
            }
            changes.push(Change::Assign {
                bot,
                behavior: group.behavior,
            });
        }
        changes
    }

    /// `alive` tells whether a bot index has a living snake.
    pub(super) fn status(&self, alive: impl Fn(usize) -> bool) -> BotStatus {
        let wanted: usize = self.groups.iter().map(|group| group.target).sum();
        BotStatus {
            groups: self
                .groups
                .iter()
                .map(|group| BotGroupStatus {
                    name: group.name.clone(),
                    behavior: group.behavior,
                    target: group.target,
                    assigned: group.bots.len(),
                    alive: group.bots.iter().filter(|&&bot| alive(bot)).count(),
                })
                .collect(),
            capacity: self.capacity(),
            free: self.capacity() - wanted,
            join_per_second: self.rate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use BotBehavior::{Center, Follow, Forager};

    fn rules() -> Rules {
        Rules {
            owner_key: true,
            circular: false,
        }
    }

    fn roster(groups: &[BotGroup], capacity: usize, rate: Option<f64>) -> Roster {
        Roster::new(groups, capacity, rate, rules())
    }

    fn assigned(changes: &[Change]) -> Vec<(usize, BotBehavior)> {
        changes
            .iter()
            .filter_map(|change| match *change {
                Change::Assign { bot, behavior } => Some((bot, behavior)),
                Change::Unassign { .. } => None,
            })
            .collect()
    }

    fn counts(roster: &Roster) -> Vec<(String, usize, usize)> {
        roster
            .status(|_| false)
            .groups
            .into_iter()
            .map(|group| (group.name, group.target, group.assigned))
            .collect()
    }

    #[test]
    fn without_a_rate_every_missing_bot_joins_at_once() {
        let mut roster = roster(&[BotGroup::new(Forager, 5)], 8, None);
        let changes = roster.reconcile(0.);
        assert_eq!(
            assigned(&changes),
            (0..5).map(|bot| (bot, Forager)).collect::<Vec<_>>()
        );
        assert!(roster.reconcile(1.).is_empty());
        let status = roster.status(|bot| bot % 2 == 0);
        assert_eq!(status.capacity, 8);
        assert_eq!(status.free, 3);
        assert_eq!(status.groups[0].alive, 3);
        assert_eq!(status.assigned(), 5);
    }

    #[test]
    fn a_rate_ramps_bots_in_over_world_time() {
        let mut roster = roster(&[BotGroup::new(Center, 10)], 10, Some(4.));
        // The first bot joins at once, then one every quarter second.
        assert_eq!(roster.reconcile(0.).len(), 1);
        let mut joined = 1;
        let mut time = 0.;
        for _ in 0..110 {
            time += 0.01;
            joined += roster.reconcile(time).len();
        }
        assert_eq!(joined, 5, "1 + 4 per second over 1.1 seconds");
        for _ in 0..200 {
            time += 0.01;
            joined += roster.reconcile(time).len();
        }
        assert_eq!(joined, 10);
        assert_eq!(roster.assigned(), 10);
        // Allowance saved while full does not become a burst later.
        roster.set_group("center", None, 4).unwrap();
        assert_eq!(roster.reconcile(time).len(), 6);
        for _ in 0..1000 {
            time += 0.01;
            assert!(roster.reconcile(time).is_empty());
        }
        roster.set_group("center", None, 10).unwrap();
        assert_eq!(roster.reconcile(time).len(), 1);
        assert_eq!(roster.reconcile(time + 0.01).len(), 0);
    }

    #[test]
    fn several_groups_below_target_fill_round_robin_in_order() {
        let groups = [
            BotGroup::new(Forager, 3),
            BotGroup {
                name: "chasers".into(),
                behavior: Follow,
                count: 1,
            },
            BotGroup::new(Center, 2),
        ];
        let mut roster = roster(&groups, 6, None);
        assert_eq!(
            assigned(&roster.reconcile(0.)),
            [
                (0, Forager),
                (1, Follow),
                (2, Center),
                (3, Forager),
                (4, Center),
                (5, Forager)
            ]
        );
        assert_eq!(roster.behavior(1), Some(Follow));
        assert_eq!(roster.behavior(6), None);
    }

    #[test]
    fn removal_takes_the_highest_slots_at_once_and_foragers_wait_for_release() {
        let groups = [BotGroup::new(Forager, 3), BotGroup::new(Center, 3)];
        let mut roster = roster(&groups, 6, Some(1e-6));
        let mut time = 0.;
        while roster.assigned() < 6 {
            time += 1e6;
            roster.reconcile(time);
        }
        roster.set_group("forager", None, 1).unwrap();
        roster.set_group("center", None, 2).unwrap();
        // Removal ignores the slow rate.
        let changes = roster.reconcile(time);
        assert_eq!(
            changes,
            [
                Change::Unassign {
                    bot: 4,
                    behavior: Forager
                },
                Change::Unassign {
                    bot: 2,
                    behavior: Forager
                },
                Change::Unassign {
                    bot: 5,
                    behavior: Center
                },
            ]
        );
        assert_eq!(roster.behavior(4), None);
        // The center slot is free at once; forager slots wait for release.
        roster.set_group("center", None, 5).unwrap();
        roster.set_rate(None).unwrap();
        assert_eq!(assigned(&roster.reconcile(time)), [(5, Center)]);
        assert!(!roster.release(5));
        assert!(roster.release(4));
        assert!(!roster.release(4));
        assert_eq!(assigned(&roster.reconcile(time)), [(4, Center)]);
        roster.release(2);
        assert_eq!(assigned(&roster.reconcile(time)), [(2, Center)]);
        assert_eq!(
            counts(&roster),
            [("forager".into(), 1, 1), ("center".into(), 5, 5)]
        );
    }

    #[test]
    fn counts_include_dead_and_waiting_bots() {
        let mut roster = roster(&[BotGroup::new(Forager, 4)], 4, None);
        roster.reconcile(0.);
        let status = roster.status(|_| false);
        assert_eq!((status.groups[0].assigned, status.groups[0].alive), (4, 0));
        // Dead bots stay in their group; nothing is replaced.
        assert!(roster.reconcile(1.).is_empty());
    }

    #[test]
    fn group_changes_follow_the_rules() {
        let mut roster = roster(&[BotGroup::new(Forager, 2)], 4, None);
        let error = roster
            .set_group("extra", Some(Center), usize::MAX)
            .unwrap_err();
        assert!(error.contains("bots.capacity 4"), "{error}");
        let error = roster.set_group("forager", None, 5).unwrap_err();
        assert!(
            error.contains("bots.capacity 4") && error.contains("restart"),
            "{error}"
        );
        let error = roster.set_group("forager", Some(Center), 2).unwrap_err();
        assert!(error.contains("set it to 0"), "{error}");
        roster.set_group("forager", Some(Forager), 3).unwrap();
        let error = roster.set_group("chasers", None, 1).unwrap_err();
        assert!(error.contains("needs a behavior"), "{error}");
        roster.set_group("chasers", Some(Follow), 1).unwrap();
        assert!(roster.set_group("chasers", None, 1).is_ok());
        assert!(roster.set_group("more", Some(Center), 1).is_err());
        for name in ["", "two words", "tab\t", "é", &"x".repeat(25)] {
            assert!(roster.set_group(name, Some(Center), 0).is_err(), "{name:?}");
        }
        assert!(roster.set_group("missing", None, 0).is_err());
        roster.reconcile(0.);
        // Zero removes the group, so its name may take another behavior.
        roster.set_group("chasers", None, 0).unwrap();
        roster.reconcile(0.);
        assert_eq!(counts(&roster), [("forager".into(), 3, 3)]);
        roster.set_group("chasers", Some(Center), 1).unwrap();
        assert!(roster.set_rate(Some(0.)).is_err());
        assert!(roster.set_rate(Some(f64::NAN)).is_err());
        roster.set_rate(Some(2.)).unwrap();
        assert_eq!(roster.status(|_| false).join_per_second, Some(2.));
    }

    #[test]
    fn follow_needs_an_owner_key_and_circular_worlds_take_only_foragers() {
        let mut keyless = Roster::new(&[], 4, None, Rules::default());
        let error = keyless.set_group("follow", None, 1).unwrap_err();
        assert!(error.contains("requires ownerKey"), "{error}");
        keyless.set_group("center", None, 1).unwrap();
        let mut dome = Roster::new(
            &[],
            4,
            None,
            Rules {
                owner_key: true,
                circular: true,
            },
        );
        let error = dome.set_group("center", None, 1).unwrap_err();
        assert!(error.contains("circular spawning"), "{error}");
        dome.set_group("forager", None, 4).unwrap();
    }
}
