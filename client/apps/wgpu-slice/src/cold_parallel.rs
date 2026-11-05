//! Bounded scoped parallelism for independent cold rasterization.
//!
//! Every item still runs the same scalar code on one thread; only the thread
//! changes, and callers observe results in input order. Wasm stays serial.
//! The cold join bursts this spreads are recorded in
//! https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-05-cold-resources/REPORT.md.
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

// Resource cap including the caller, as for the pellet pool: leave CPU capacity
// for other applications. available_parallelism limits this further.
const MAX_PARTICIPANTS: usize = 4;

fn participants(items: usize) -> usize {
    // Warm frames call this with no work; skip the parallelism probe, which
    // re-reads affinity and cgroup limits on Linux each time.
    if items <= 1 || cfg!(target_arch = "wasm32") {
        return 1;
    }
    std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(MAX_PARTICIPANTS)
        .min(items)
}

/// `items.iter().map(f).collect()`, with independent items claimed by up to
/// four threads. A thread that cannot start leaves its share to the caller.
pub(crate) fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let participants = participants(items.len());
    if participants <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let work = || {
        let mut done = Vec::new();
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(item) = items.get(i) else {
                return done;
            };
            done.push((i, f(item)));
        }
    };
    let mut slots: Vec<Option<R>> = items.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let helpers: Vec<_> = (1..participants)
            .filter_map(|_| std::thread::Builder::new().spawn_scoped(scope, work).ok())
            .collect();
        for (i, value) in work() {
            slots[i] = Some(value);
        }
        for helper in helpers {
            let done = helper
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            for (i, value) in done {
                slots[i] = Some(value);
            }
        }
    });
    slots
        .into_iter()
        .map(|value| value.expect("every claimed item completes"))
        .collect()
}

/// Run `f(row_index, row)` for each `width`-element row of `data`, with rows
/// claimed by up to four threads. Rows must be independent of each other.
pub(crate) fn rows<T: Send>(data: &mut [T], width: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    assert!(width > 0 && data.len().is_multiple_of(width));
    let participants = participants(data.len() / width);
    if participants <= 1 {
        for (y, row) in data.chunks_mut(width).enumerate() {
            f(y, row);
        }
        return;
    }
    let pending = Mutex::new(data.chunks_mut(width).enumerate());
    let work = || {
        loop {
            let claimed = pending.lock().unwrap_or_else(|e| e.into_inner()).next();
            let Some((y, row)) = claimed else {
                return;
            };
            f(y, row);
        }
    };
    std::thread::scope(|scope| {
        for _ in 1..participants {
            // A helper that cannot start leaves its rows to the caller.
            let _ = std::thread::Builder::new().spawn_scoped(scope, work);
        }
        work();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_keeps_input_order_for_any_claim_order() {
        let items: Vec<u64> = (0..997).collect();
        let mapped = map(&items, |x| {
            if x % 7 == 0 {
                std::thread::yield_now();
            }
            x * x + 1
        });
        assert_eq!(mapped, items.iter().map(|x| x * x + 1).collect::<Vec<_>>());
        assert!(map(&[] as &[u8], |x| *x).is_empty());
        assert_eq!(map(&[5u8], |x| x + 1), [6]);
    }

    #[test]
    fn rows_visit_every_row_once_with_its_index() {
        let mut data = vec![0usize; 13 * 101];
        rows(&mut data, 13, |y, row| {
            for (x, v) in row.iter_mut().enumerate() {
                *v += y * 1000 + x + 1;
            }
        });
        for (i, v) in data.iter().enumerate() {
            assert_eq!(*v, (i / 13) * 1000 + i % 13 + 1);
        }
    }

    #[test]
    #[should_panic(expected = "item 3")]
    fn a_worker_panic_reaches_the_caller() {
        let items: Vec<usize> = (0..64).collect();
        map(&items, |x| {
            assert_ne!(*x, 3, "item 3");
        });
    }
}
