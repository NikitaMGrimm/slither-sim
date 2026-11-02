//! Quad-record attribution; GPU draw runs and preview commands are separate units.

/// Paint owner of a menu quad, including text after clipping.
#[derive(Clone, Copy, Debug, Default)]
#[repr(usize)]
pub enum MenuComponent {
    LoginWordmark,
    LoginControls,
    LoginText,
    #[default]
    Other,
}
impl MenuComponent {
    pub const ALL: [Self; 4] = [
        Self::LoginWordmark,
        Self::LoginControls,
        Self::LoginText,
        Self::Other,
    ];
}

/// Counts only successfully prepared records, never textures or batched GPU runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuAttribution(pub [usize; 4]);
impl MenuAttribution {
    pub fn total(self) -> usize {
        self.0.iter().sum()
    }
    pub fn get(self, component: MenuComponent) -> usize {
        self.0[component as usize]
    }
    pub(crate) fn add(&mut self, component: MenuComponent, count: usize) {
        self.0[component as usize] += count;
    }
    pub(crate) fn remove(&mut self, component: MenuComponent, count: usize) {
        self.0[component as usize] -= count;
    }
}

/// Reconciles the historical transition counter and the renderer's admission count.
#[derive(Clone, Copy, Debug)]
pub struct MenuDrawCounts {
    pub components: MenuAttribution,
    pub page: usize,
    pub cover: usize,
}
impl MenuDrawCounts {
    pub fn reported_menu(self) -> usize {
        self.components.total() + self.page
    }
    pub fn admitted_menu(self) -> usize {
        self.reported_menu() + self.cover
    }
}
