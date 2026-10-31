//! Bounded custom-pattern editing. Retained wire bytes are owned by the studio.

/// Selectable stock palette IDs, in chooser order; 40 is preview-only in the reference.
pub const PALETTES: [u8; 39] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 37, 39, 41,
];
/// Expanded-colour limit for appending, independent of retained legacy run lengths.
pub const MAX_COLOURS: usize = 47;

/// One normalized run, potentially longer than one wire count after merging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PatternRun {
    /// Expanded colour count, bounded by the 123 retained wire pairs.
    pub count: usize,
    /// A selectable normal palette.
    pub palette: u8,
}
/// At most 123 wire pairs; never expands a retained pattern into individual colours.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PatternRuns(Vec<PatternRun>);
impl PatternRuns {
    /// Interpret only complete, positive, selectable runs. Prefix bytes remain opaque.
    /// Other retained payloads remain usable unchanged but start an empty editor.
    pub fn from_wire(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 10 || bytes.len() > 255 || !bytes.len().is_multiple_of(2) {
            return None;
        }
        let mut result = Self::default();
        for pair in bytes[8..].as_chunks::<2>().0 {
            if pair[0] == 0 || !PALETTES.contains(&pair[1]) {
                return None;
            }
            result.push(usize::from(pair[0]), pair[1]);
        }
        Some(result)
    }
    /// Normalized bounded runs for windowed controls.
    pub fn runs(&self) -> &[PatternRun] {
        &self.0
    }
    /// Expanded count without allocating an expanded vector.
    pub fn colours(&self) -> usize {
        self.0.iter().map(|r| r.count).sum()
    }
    fn push(&mut self, count: usize, palette: u8) {
        if let Some(last) = self.0.last_mut().filter(|r| r.palette == palette) {
            last.count += count;
        } else {
            self.0.push(PatternRun { count, palette });
        }
    }
    /// Encode normal pairs, splitting merged counts at 255. No preview padding.
    pub fn encode(&self, header: [u8; 8]) -> Result<Vec<u8>, &'static str> {
        let pairs: usize = self.0.iter().map(|r| r.count.div_ceil(255)).sum();
        if pairs > 123 {
            return Err("Pattern exceeds wire limit");
        }
        if self.0.is_empty() {
            return Err("Pattern is empty");
        }
        let mut bytes = header.to_vec();
        for run in &self.0 {
            let mut remaining = run.count;
            while remaining > 0 {
                let count = remaining.min(255);
                bytes.extend([count as u8, run.palette]);
                remaining -= count;
            }
        }
        Ok(bytes)
    }
}

/// A transaction-local editor. Equality with original runs preserves every retained byte.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuilderDraft {
    pattern: PatternRuns,
    original: PatternRuns,
    /// Selected run, not an index into an expanded legacy vector.
    pub selected: usize,
    /// Roving palette focus and recolour target.
    pub palette: usize,
}
impl BuilderDraft {
    /// Read bounded editable runs, without changing the retained record.
    pub fn new(bytes: &[u8]) -> Self {
        let pattern = PatternRuns::from_wire(bytes).unwrap_or_default();
        Self {
            original: pattern.clone(),
            pattern,
            selected: 0,
            palette: 0,
        }
    }
    /// Current bounded runs.
    pub fn pattern(&self) -> &PatternRuns {
        &self.pattern
    }
    /// No entropy or normalization is needed for an unchanged pattern.
    pub fn changed(&self) -> bool {
        self.pattern != self.original
    }
    /// Append one colour, merging its adjacent equal predecessor.
    pub fn append(&mut self, palette: u8) -> bool {
        if self.pattern.colours() >= MAX_COLOURS || !PALETTES.contains(&palette) {
            return false;
        }
        self.pattern.push(1, palette);
        self.selected = self.pattern.0.len() - 1;
        true
    }
    /// Remove the selected whole run, allowing long legacy patterns to shrink.
    pub fn remove(&mut self) -> bool {
        if self.selected >= self.pattern.0.len() {
            return false;
        }
        let mut pattern = PatternRuns::default();
        for (index, run) in self.pattern.0.iter().enumerate() {
            if index != self.selected {
                pattern.push(run.count, run.palette);
            }
        }
        self.pattern = pattern;
        self.selected = self.selected.min(self.pattern.0.len().saturating_sub(1));
        true
    }
    /// Recolour a whole run without expanding it; preserve the wire-size bound.
    pub fn recolour(&mut self, palette: u8) -> Result<(), &'static str> {
        if self.selected >= self.pattern.0.len() || !PALETTES.contains(&palette) {
            return Err("Select a pattern run");
        }
        let mut pattern = PatternRuns::default();
        let mut selected = 0;
        for (index, run) in self.pattern.0.iter().enumerate() {
            pattern.push(
                run.count,
                if index == self.selected {
                    palette
                } else {
                    run.palette
                },
            );
            if index == self.selected {
                selected = pattern.0.len() - 1;
            }
        }
        pattern.encode([0; 8])?;
        self.pattern = pattern;
        self.selected = selected;
        Ok(())
    }
    /// Reset only the builder colours; the studio retains its saved custom bytes.
    pub fn reset(&mut self) {
        self.pattern = PatternRuns::default();
        self.selected = 0;
    }
}
