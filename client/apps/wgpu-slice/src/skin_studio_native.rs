//! Native studio adapter: preview production and best-effort persistence, no transport.
use crate::{
    Result, bank::Bank, cosmetics::Cosmetics, frame::Frame, menu_storage::MenuStorage,
    render::PreparedFrame, scene::Resource,
};
use slither_client::{skin_studio::*, studio_preview::AppearancePreview};

pub struct SkinStudioNative {
    pub state: StudioState,
    pub storage: MenuStorage,
    pub status: String,
    pub now: f64,
    pub preview: AppearancePreview,
    pub wheel: f64,
    pub(crate) viewport: [f64; 2],
    pub thumbnails: std::collections::BTreeMap<u8, Resource>,
    bank: Bank,
    fallback: Cosmetics,
    staged: Option<AppearancePreview>,
}
impl SkinStudioNative {
    pub fn new(record: AppearanceRecord, storage: MenuStorage) -> Result<Self> {
        Ok(Self {
            state: StudioState::new(record).map_err(str::to_owned)?,
            storage,
            status: String::new(),
            now: 0.,
            preview: Default::default(),
            wheel: 0.,
            viewport: [960., 540.],
            thumbnails: Default::default(),
            bank: Bank::new()?,
            fallback: Cosmetics::from_images(Default::default(), None)?,
            staged: None,
        })
    }
    pub fn step(&mut self, now: f64) {
        self.now = now;
        self.state.step(now);
        if self.state.visible() {
            self.preview.tick(now);
        } else {
            self.preview.pause();
        }
    }
    pub fn open(&mut self, admitted: bool) -> bool {
        let opened = self.state.open(self.now, admitted);
        if opened {
            self.status.clear();
            self.preview = Default::default();
            self.wheel = 0.;
        }
        opened
    }
    pub fn activate(&mut self, control: slither_client::menu_controls::ControlId, nickname: &str) {
        self.activate_with_random(control, nickname, &mut rand::random);
    }
    /// Entropy is injected at the single Save boundary; fixtures need no global random state.
    pub fn activate_with_random(
        &mut self,
        control: slither_client::menu_controls::ControlId,
        nickname: &str,
        random_byte: &mut impl FnMut() -> u8,
    ) {
        use slither_client::skin_builder::PALETTES;
        if !self.state.interactive() {
            return;
        }
        if ![SAVE, CANCEL, BACKDROP].contains(&control) {
            self.status.clear();
        }
        match control {
            BUILT_IN_TAB => self.state.set_section(StudioSection::BuiltIn, self.now),
            BUILDER_TAB => self.state.set_section(StudioSection::Builder, self.now),
            COSMETICS_TAB => self.state.set_section(StudioSection::Cosmetics, self.now),
            COSMETIC_PREVIOUS => self.state.scroll_cosmetics(
                -(self.state.cosmetic_range(self.viewport).len().div_ceil(4) as isize),
                self.viewport,
            ),
            COSMETIC_NEXT => self.state.scroll_cosmetics(
                self.state.cosmetic_range(self.viewport).len().div_ceil(4) as isize,
                self.viewport,
            ),
            id if (COSMETIC_BASE..COSMETIC_BASE + 33).contains(&id.0) => {
                let index = id.0 - COSMETIC_BASE;
                self.state
                    .select_cosmetic((index > 0).then(|| (index - 1) as u8));
            }
            ADD => self.state.edit_builder(|b| {
                b.append(PALETTES[b.palette]);
            }),
            REMOVE => self.state.edit_builder(|b| {
                b.remove();
            }),
            RECOLOUR => {
                let mut result = Ok(());
                self.state.edit_builder(|b| {
                    result = b.recolour(PALETTES[b.palette]);
                });
                if let Err(e) = result {
                    self.status = e.into();
                }
            }
            RESET => self.state.edit_builder(|b| b.reset()),
            RUN_PREVIOUS => self
                .state
                .edit_builder(|b| b.selected = b.selected.saturating_sub(1)),
            RUN_NEXT => self.state.edit_builder(|b| {
                b.selected = (b.selected + 1).min(b.pattern().runs().len().saturating_sub(1))
            }),
            PREVIOUS => {
                self.state.cycle(false);
                self.status.clear();
            }
            NEXT => {
                self.state.cycle(true);
                self.status.clear();
            }
            CANCEL | BACKDROP => self.state.cancel(self.now),
            SAVE => match self.state.save(nickname, self.now, random_byte) {
                Ok(Some(record)) => {
                    self.status = if self.storage.save(&record).is_ok() {
                        "Skin saved"
                    } else {
                        "Saved for this run"
                    }
                    .into()
                }
                Err(e) => self.status = e.into(),
                _ => {}
            },
            id if id.0 >= RUN_BASE => self.state.edit_builder(|b| {
                let index = usize::from(id.0 - RUN_BASE);
                if index < b.pattern().runs().len() {
                    b.selected = index;
                }
            }),
            id => {
                if let Some(index) = PALETTES
                    .iter()
                    .position(|p| SWATCH_BASE + u16::from(*p) == id.0)
                {
                    self.state.edit_builder(|b| b.palette = index);
                }
            }
        }
    }
    /// Decode local art serially before copying the active panel; it never commits a choice.
    pub fn sync_art(
        &mut self,
        viewport: [f64; 2],
        cosmetics: Option<&mut Cosmetics>,
    ) -> Result<()> {
        self.viewport = viewport;
        self.thumbnails.clear();
        let Some(c) = cosmetics else {
            if self.state.interactive() && self.state.section == StudioSection::Cosmetics {
                self.status = self.state.preview_cosmetic().map_or_else(
                    || "No cosmetic".into(),
                    |id| format!("Cosmetic {id} - art unavailable"),
                );
            }
            return Ok(());
        };
        if !self.state.visible() {
            c.release_studio();
            return Ok(());
        }
        let visible: Vec<_> = if self.state.section == StudioSection::Cosmetics {
            self.state
                .cosmetic_range(viewport)
                .filter(|i| *i > 0)
                .map(|i| (i - 1) as u8)
                .collect()
        } else {
            vec![]
        };
        c.prepare_studio(&visible, self.state.preview_cosmetic())?;
        self.thumbnails = c.studio_thumbnails();
        if self.state.interactive() && self.state.section == StudioSection::Cosmetics {
            self.status = self.state.preview_cosmetic().map_or_else(
                || "No cosmetic".into(),
                |id| {
                    use crate::cosmetics::CosmeticArtStatus::*;
                    match c.studio_status(id) {
                        Ready => format!("Cosmetic {id}"),
                        Missing => format!("Cosmetic {id} - art unavailable"),
                        OverBudget => format!("Cosmetic {id} - art budget exceeded"),
                    }
                },
            );
        }
        Ok(())
    }
    /// Ready-only resolver and single-image suppression match own-head gameplay art.
    pub fn prepare_preview(
        &mut self,
        extent: [u32; 2],
        dpr: f64,
        cosmetics: Option<&Cosmetics>,
    ) -> Result<(Frame, Vec<Resource>)> {
        self.staged = None;
        self.bank.cosmetics = Some(match cosmetics {
            Some(c) => c.preview_sources_for(self.state.preview_cosmetic()),
            None => self.fallback.preview_sources(),
        });
        let selection = self.state.preview_selection().map_err(str::to_owned)?;
        let sources = self.bank.cosmetics.as_ref().expect("preview sources");
        let skin = sources
            .skin(selection.skin, &selection.custom_bytes)
            .map_err(|e| e.to_string())?;
        let requested = slither_client::presentation::resolve_skin(
            selection.skin,
            &selection.custom_bytes,
            false,
        )
        .map_err(|e| e.to_string())?;
        if self.status.is_empty()
            && (requested.face_sources != skin.face_sources || requested.antenna != skin.antenna)
        {
            self.status = "Some preview art is unavailable".into();
        }
        let area = extent.map(|v| f64::from(v) / dpr);
        let accessory = sources.preview_accessory(&skin).map(|(g, a)| {
            (
                g,
                slither_client::state::Point {
                    x: f64::from(a.width),
                    y: f64::from(a.height),
                },
            )
        });
        let pose = self.preview.prepare_with_accessory(
            &skin,
            StudioState::preview_bounds(area),
            &[[0.25, 0.75], [0.65, 0.35], [0.45, 0.55]],
            accessory,
        )?;
        let picture = crate::render::prepare_studio_preview(
            &mut self.bank,
            &pose,
            &skin,
            self.preview.time(),
            extent,
            dpr,
            self.state.alpha(),
        )?;
        self.staged = Some(pose.next);
        Ok(picture)
    }
    /// Admit preview with the immutable menu/world picture before advancing antenna state.
    pub fn append_preview(
        &mut self,
        picture: &mut PreparedFrame,
        cosmetics: Option<&Cosmetics>,
    ) -> Result<()> {
        let ui = picture.ui_frame();
        let (preview, resources) = self.prepare_preview(ui.extent, ui.viewport_scale, cosmetics)?;
        if preview.draws.len() > 1024 {
            return Err(crate::frame::DRAW_MEMORY_LIMIT.into());
        }
        let mut all: std::collections::BTreeMap<_, _> = picture
            .resources
            .iter()
            .map(|r| (r.asset.handle, r.clone()))
            .collect();
        for r in resources {
            if let Some(old) = all.insert(r.asset.handle, r.clone())
                && old.asset != r.asset
            {
                return Err("Preview resource collision".into());
            }
        }
        if all.len() > 2048
            || all.values().map(|r| r.image.pixels.len()).sum::<usize>() > crate::bank::PIXEL_BUDGET
        {
            return Err(crate::menu::MENU_PICTURE_QUOTA.into());
        }
        crate::frame::check_draw_count(picture.draw_count() + preview.draws.len())?;
        picture.ui_frame_mut().draws.extend(preview.draws);
        picture.resources = all.into_values().collect();
        Ok(())
    }
    pub fn presented(&mut self) {
        if let Some(next) = self.staged.take() {
            self.preview = next;
        }
    }
}
