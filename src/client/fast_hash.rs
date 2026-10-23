//! Fast deterministic hashing for exact keys. Use it only for maps whose
//! iteration order is never observed, so the hasher affects speed alone.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FastHasher(u64);

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }
    fn write_u16(&mut self, value: u16) {
        self.write_u64(u64::from(value));
    }
    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value));
    }
    fn write_u64(&mut self, value: u64) {
        let product = u128::from(self.0 ^ value) * 0x9e37_79b9_7f4a_7c15;
        self.0 = product as u64 ^ (product >> 64) as u64;
    }
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

pub(super) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;
pub(super) type FastSet<K> = HashSet<K, BuildHasherDefault<FastHasher>>;
