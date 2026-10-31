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
        }
        opened
    }
    pub fn activate(&mut self, control: slither_client::menu_controls::ControlId, nickname: &str) {
        match control {
            PREVIOUS => {
                self.state.cycle(false, UnlockSnapshot::default());
                self.status.clear();
            }
            NEXT => {
                self.state.cycle(true, UnlockSnapshot::default());
                self.status.clear();
            }
            CANCEL | BACKDROP => self.state.cancel(self.now),
            SAVE => match self
                .state
                .save(nickname, self.now, UnlockSnapshot::default())
            {
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
            _ => {}
        }
    }
    /// Ready-only resolver is identical to gameplay's full-art fallback, without accessories.
    pub fn prepare_preview(
        &mut self,
        extent: [u32; 2],
        dpr: f64,
        cosmetics: Option<&Cosmetics>,
    ) -> Result<(Frame, Vec<Resource>)> {
        self.staged = None;
        self.bank.cosmetics = Some(match cosmetics {
            Some(c) => c.preview_sources(),
            None => self.fallback.preview_sources(),
        });
        let record = &self.state.draft().ok_or("Studio draft unavailable")?.record;
        let selection = record.selection();
        if selection != self.state.committed().selection()
            && !UnlockSnapshot::default().skin_available(selection.skin)
        {
            return Err("Preview selection unavailable".into());
        }
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
        let pose = self.preview.prepare(
            &skin,
            StudioState::preview_bounds(area),
            &[[0.25, 0.75], [0.65, 0.35], [0.45, 0.55]],
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
