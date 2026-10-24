//! Batch work on the current rayon thread pool. Results come back in range
//! order, so output never depends on the pool size or on scheduling.
use rayon::prelude::*;
use std::ops::Range;

/// Ranges to split work over when `useful` ranges would each pay for their
/// scheduling cost. A few per thread let idle threads take over uneven work.
pub(crate) fn parts(useful: usize) -> usize {
    match rayon::current_num_threads() {
        // One thread would run the ranges in turn, each with its own buffers.
        1 => 1,
        threads => useful.min(4 * threads).max(1),
    }
}

/// `work` over each range and its slice of `items` in parallel, results in
/// range order. The ranges must cover `items` consecutively from the start.
pub(crate) fn map_ranges<T: Send, R: Send>(
    ranges: &[Range<usize>],
    items: &mut [T],
    work: &(impl Fn(Range<usize>, &mut [T]) -> R + Sync),
) -> Vec<R> {
    let mut parts = Vec::with_capacity(ranges.len());
    let mut rest = items;
    for range in ranges {
        let (part, tail) = rest.split_at_mut(range.len());
        parts.push((range.clone(), part));
        rest = tail;
    }
    parts
        .into_par_iter()
        .map(|(range, part)| work(range, part))
        .collect()
}

/// Split `0..flags.len()` into at most `parts` consecutive ranges holding
/// roughly equal numbers of set flags.
pub(crate) fn balanced(flags: &[bool], parts: usize) -> Vec<Range<usize>> {
    let total = flags.iter().filter(|&&f| f).count();
    let parts = parts.clamp(1, total.max(1));
    let mut ranges = Vec::with_capacity(parts);
    let (mut start, mut seen) = (0, 0);
    for (i, &flag) in flags.iter().enumerate() {
        seen += usize::from(flag);
        // Cut after the item that completes this part's share.
        if ranges.len() + 1 < parts && seen * parts >= total * (ranges.len() + 1) {
            ranges.push(start..i + 1);
            start = i + 1;
        }
    }
    ranges.push(start..flags.len());
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_cover_every_item_in_order_and_balance_flags() {
        for len in 0..40 {
            for parts in 1..6 {
                let flags: Vec<bool> = (0..len).map(|i| i % 3 != 1).collect();
                let ranges = balanced(&flags, parts);
                assert_eq!(ranges[0].start, 0);
                assert_eq!(ranges.last().unwrap().end, len);
                assert!(ranges.windows(2).all(|w| w[0].end == w[1].start));
                assert!(ranges.len() <= parts);
                let total = flags.iter().filter(|&&f| f).count();
                for r in &ranges {
                    let set = flags[r.clone()].iter().filter(|&&f| f).count();
                    assert!(
                        set <= total.div_ceil(ranges.len()) + 1,
                        "{len} {parts} {ranges:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn only_pools_with_several_threads_split() {
        let parts_on = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| (parts(0), parts(3), parts(1000)))
        };
        assert_eq!(parts_on(1), (1, 1, 1));
        assert_eq!(parts_on(2), (1, 3, 8));
    }

    #[test]
    fn mapped_ranges_return_in_order() {
        let ranges = balanced(&[true; 100], 7);
        let mut items: Vec<usize> = (0..100).collect();
        let sums = map_ranges(
            &ranges,
            &mut items,
            &|r: Range<usize>, part: &mut [usize]| {
                assert!(part.iter().copied().eq(r.clone()));
                part.iter_mut().for_each(|item| *item *= 2);
                r.sum::<usize>()
            },
        );
        let expected: Vec<usize> = ranges.iter().map(|r| r.clone().sum()).collect();
        assert_eq!(sums, expected);
        assert!(items.iter().copied().eq((0..100).map(|i| i * 2)));
    }
}
