//! Bounds tree for conservative opponent candidates in one immutable batch.
use super::{Bounds, Snake, tame};

struct Entry {
    bounds: Bounds,
    slot: usize,
}

struct Node {
    bounds: Bounds,
    start: usize,
    end: usize,
    right: usize,
}

pub(super) struct SnakeIndex {
    entries: Vec<Entry>,
    nodes: Vec<Node>,
    outliers: Vec<usize>,
}

impl SnakeIndex {
    pub(super) fn new(snakes: &[Snake], bounds: &[Bounds]) -> Self {
        let mut tree = Self {
            entries: Vec::new(),
            nodes: Vec::new(),
            outliers: Vec::new(),
        };
        for (slot, (snake, &bounds)) in snakes.iter().zip(bounds).enumerate() {
            if !snake.alive {
                continue;
            }
            if tame(bounds.min) && tame(bounds.max) {
                tree.entries.push(Entry { bounds, slot });
            } else {
                // NaN and extreme bounds retain the original per-snake predicate.
                tree.outliers.push(slot);
            }
        }
        if !tree.entries.is_empty() {
            tree.build(0, tree.entries.len());
        }
        tree
    }

    fn build(&mut self, start: usize, end: usize) -> usize {
        let mut bounds = self.entries[start].bounds;
        for entry in &self.entries[start + 1..end] {
            bounds.include(entry.bounds.min);
            bounds.include(entry.bounds.max);
        }
        let node = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            start,
            end,
            right: 0,
        });
        // Short leaf scans avoid traversing a separate node for each snake.
        if end - start > 8 {
            let x = bounds.max.x - bounds.min.x > bounds.max.y - bounds.min.y;
            let center = |b: Bounds| {
                if x {
                    b.min.x + b.max.x
                } else {
                    b.min.y + b.max.y
                }
            };
            let mid = start + (end - start) / 2;
            self.entries[start..end].select_nth_unstable_by(mid - start, |a, b| {
                center(a.bounds).total_cmp(&center(b.bounds))
            });
            self.build(start, mid);
            self.nodes[node].right = self.build(mid, end);
        }
        node
    }

    pub(super) fn visit(&self, query: Bounds, mut candidate: impl FnMut(usize)) {
        if !self.nodes.is_empty() {
            self.visit_node(0, query, &mut candidate);
        }
        for &slot in &self.outliers {
            candidate(slot);
        }
    }

    fn visit_node(&self, node: usize, query: Bounds, candidate: &mut impl FnMut(usize)) {
        let n = &self.nodes[node];
        if !overlaps(n.bounds, query) {
            return;
        }
        if n.right == 0 {
            for entry in &self.entries[n.start..n.end] {
                if overlaps(entry.bounds, query) {
                    candidate(entry.slot);
                }
            }
        } else {
            self.visit_node(node + 1, query, candidate);
            self.visit_node(n.right, query, candidate);
        }
    }
}

fn overlaps(a: Bounds, b: Bounds) -> bool {
    !(a.max.x < b.min.x || a.min.x > b.max.x || a.max.y < b.min.y || a.min.y > b.max.y)
}
