//! Batch work on the current rayon thread pool. Results come back in range
//! order, so output never depends on the pool size or on scheduling.
use rayon::prelude::*;
use std::ops::Range;

/// Ranges to split work over when `useful` ranges would each pay for their
/// scheduling cost. A few per thread let idle threads take over uneven work.
pub(crate) fn parts(useful: usize) -> usize {
    useful.min(4 * rayon::current_num_threads()).max(1)
}

/// `work` over each range in parallel, results in range order.
pub(crate) fn map_ranges<R: Send>(
    ranges: &[Range<usize>],
    work: &(impl Fn(Range<usize>) -> R + Sync),
) -> Vec<R> {
    ranges.par_iter().map(|range| work(range.clone())).collect()
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
    fn mapped_ranges_return_in_order() {
        let ranges = balanced(&[true; 100], 7);
        let sums = map_ranges(&ranges, &|r: Range<usize>| r.sum::<usize>());
        let expected: Vec<usize> = ranges.iter().map(|r| r.clone().sum()).collect();
        assert_eq!(sums, expected);
    }
}
