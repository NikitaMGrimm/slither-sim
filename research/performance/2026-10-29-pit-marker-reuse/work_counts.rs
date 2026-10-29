//! Untimed operation census only. Never overlay this in a performance build.
use std::cell::Cell;
thread_local! {
    static COUNTS: Cell<[u64; 7]> = const { Cell::new([0; 7]) };
}
pub fn begin() { COUNTS.with(|v| v.set([0; 7])); }
pub fn add(index: usize, count: u64) {
    COUNTS.with(|v| { let mut counts = v.get(); counts[index] += count; v.set(counts); });
}
pub fn finish() -> [u64; 7] { COUNTS.with(Cell::get) }
