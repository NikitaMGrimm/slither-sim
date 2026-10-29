//! Container oracle against the pre-compaction implementation.
use super::{KnownSnake, KnownSnakes};
use crate::model::Point;

#[derive(Default)]
struct Previous {
    by_slot: Vec<Option<KnownSnake>>,
    slots: Vec<usize>,
}

impl Previous {
    fn insert(&mut self, slot: usize, known: KnownSnake) {
        if self.by_slot.len() <= slot {
            self.by_slot.resize(slot + 1, None);
        }
        if self.by_slot[slot].is_none() {
            let index = self.slots.binary_search(&slot).unwrap_err();
            self.slots.insert(index, slot);
        }
        self.by_slot[slot] = Some(known);
    }

    fn remove(&mut self, slot: usize) -> Option<KnownSnake> {
        let known = self.by_slot.get_mut(slot)?.take()?;
        let index = self.slots.binary_search(&slot).unwrap();
        self.slots.remove(index);
        Some(known)
    }

    fn get_mut(&mut self, slot: usize) -> Option<&mut KnownSnake> {
        self.by_slot.get_mut(slot)?.as_mut()
    }

    fn state(&self) -> Vec<(usize, [u64; 9])> {
        self.slots
            .iter()
            .map(|&slot| (slot, bits(self.by_slot[slot].as_ref().unwrap())))
            .collect()
    }
}

fn bits(known: &KnownSnake) -> [u64; 9] {
    [
        known.life,
        known.front.x.to_bits(),
        known.front.y.to_bits(),
        known.len as u64,
        known.heading.into(),
        known.wanted.into(),
        known.speed.into(),
        known.fam.into(),
        known.kills as u64,
    ]
}

fn state(known: &mut KnownSnakes) -> Vec<(usize, [u64; 9])> {
    let slots: Vec<_> = known.slots().collect();
    slots
        .into_iter()
        .map(|slot| (slot, bits(known.get_mut(slot).unwrap())))
        .collect()
}

struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn slot(&mut self) -> usize {
        let value = self.next();
        if value.is_multiple_of(257) {
            65_535
        } else {
            (value % 512) as usize
        }
    }

    fn record(&mut self) -> KnownSnake {
        let front = match self.next() % 4 {
            0 => Point { x: -0., y: 0. },
            1 => Point {
                x: f64::from_bits(0x7ff8_0000_0000_1234),
                y: f64::from_bits(0x7ff0_0000_0000_0042),
            },
            2 => Point {
                x: f64::INFINITY,
                y: f64::NEG_INFINITY,
            },
            _ => Point {
                x: f64::from_bits(self.next()),
                y: f64::from_bits(self.next()),
            },
        };
        KnownSnake {
            life: self.next(),
            front,
            len: self.next() as usize,
            heading: self.next() as u8,
            wanted: self.next() as u8,
            speed: self.next() as u8,
            fam: self.next() as u32,
            kills: self.next() as usize,
        }
    }
}

#[test]
fn compact_histories_match_previous_slots_under_random_operations() {
    let mut random = Random(0x4c32_0f9a_77b1_861d);
    let mut recipients: Vec<_> = (0..8)
        .map(|_| (KnownSnakes::default(), Previous::default()))
        .collect();
    let mut output = Vec::new();
    for step in 0..20_000 {
        let recipient = random.next() as usize % recipients.len();
        let (actual, previous) = &mut recipients[recipient];
        let slot = random.slot();
        if step % 997 == 0 {
            // A reconnect resets only its own history, including a high slot.
            *actual = KnownSnakes::default();
            *previous = Previous::default();
        }
        match random.next() % 8 {
            0 | 1 => {
                let value = random.record();
                actual.insert(slot, value.clone());
                previous.insert(slot, value);
            }
            2 => assert_eq!(
                actual.remove(slot).as_ref().map(bits),
                previous.remove(slot).as_ref().map(bits)
            ),
            3 => {
                let value = random.record();
                let left = actual.get_mut(slot);
                let right = previous.get_mut(slot);
                assert_eq!(left.as_deref().map(bits), right.as_deref().map(bits));
                if let (Some(left), Some(right)) = (left, right) {
                    *left = value.clone();
                    *right = value;
                }
            }
            4 => {
                let before = state(actual);
                let mut cloned = actual.clone();
                let value = random.record();
                cloned.insert(slot, value.clone());
                assert_eq!(cloned.remove(slot).as_ref().map(bits), Some(bits(&value)));
                assert_eq!(state(actual), before);
            }
            _ => {
                let mut nearby: Vec<_> = (0..8).map(|_| random.slot()).collect();
                nearby.sort_unstable();
                nearby.dedup();
                let mut expected = nearby.clone();
                expected.extend_from_slice(&previous.slots);
                expected.sort_unstable();
                expected.dedup();
                actual.candidates(&nearby, &mut output);
                assert_eq!(output, expected);
            }
        }
        for probe in [slot, slot ^ 1, usize::MAX] {
            assert_eq!(
                actual.contains_key(probe),
                previous.by_slot.get(probe).is_some_and(Option::is_some)
            );
        }
        assert_eq!(state(actual), previous.state());
        if step % 17 == 0 {
            for (actual, previous) in &mut recipients {
                assert_eq!(state(actual), previous.state());
            }
        }
    }
}
