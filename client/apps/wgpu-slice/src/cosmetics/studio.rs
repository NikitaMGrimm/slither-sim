//! Local, serial decoding for a twelve-cell window and one pinned draft selection.
use super::*;
use std::{path::PathBuf, time::SystemTime};

pub const THUMBNAIL_PIXEL_BUDGET: usize = 4 * 1024 * 1024;
const THUMBNAIL_EXTENT: u32 = 128;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CosmeticArtStatus {
    Ready,
    #[default]
    Missing,
    OverBudget,
}
#[derive(Clone, PartialEq, Eq)]
enum Stamp {
    File(u64, SystemTime),
    Bound(crate::frame::ResourceKey),
}
#[derive(Default)]
pub(super) struct StudioArt {
    pub directory: Option<PathBuf>,
    thumbnails: BTreeMap<u8, (Resource, Stamp)>,
    selected: Option<(u8, Resource, Stamp)>,
    revisions: [u64; 32],
    status: [CosmeticArtStatus; 32],
}
impl Cosmetics {
    /// Only visible thumbnails are retained; selected full art is independent of the window.
    pub fn studio_thumbnails(&self) -> BTreeMap<u8, Resource> {
        self.studio
            .thumbnails
            .iter()
            .map(|(id, (r, _))| (*id, r.clone()))
            .collect()
    }
    pub fn studio_status(&self, id: u8) -> CosmeticArtStatus {
        self.studio
            .status
            .get(usize::from(id))
            .copied()
            .unwrap_or_default()
    }
    pub fn release_studio(&mut self) {
        self.studio.thumbnails.clear();
        self.studio.selected = None;
    }
    fn source(&self, id: u8) -> Result<(Option<PathBuf>, Stamp)> {
        if id > 31 {
            return Err("Cosmetic ID".into());
        }
        if let Some(root) = &self.studio.directory {
            let file = root
                .join(format!("s/a_{}.png", ACCESSORIES[usize::from(id)].0))
                .canonicalize()
                .map_err(|e| e.to_string())?;
            if !file.starts_with(root) {
                return Err("Cosmetic path escaped directory".into());
            }
            let metadata = file.metadata().map_err(|e| e.to_string())?;
            if metadata.len() > MAX_ENCODED_BYTES as u64 {
                return Err("Cosmetic encoded image budget".into());
            }
            let stamp = Stamp::File(
                metadata.len(),
                metadata.modified().map_err(|e| e.to_string())?,
            );
            Ok((Some(file), stamp))
        } else {
            let resource = self
                .accessories
                .get(&id)
                .ok_or("Cosmetic art unavailable")?;
            Ok((None, Stamp::Bound(resource.asset.into())))
        }
    }
    /// Charge shared sources once, including retained candidates during replacement.
    pub fn studio_pixel_bytes(&self) -> usize {
        let mut sources = BTreeMap::new();
        for r in self
            .resources()
            .chain(self.studio.thumbnails.values().map(|(r, _)| r))
            .chain(self.studio.selected.iter().map(|(_, r, _)| r))
        {
            sources.insert(
                crate::frame::ResourceKey::from(r.asset),
                r.image.pixels.len(),
            );
        }
        sources.values().sum()
    }
    fn decoded_source(&self, id: u8, file: Option<PathBuf>, extra: usize) -> Result<RgbaImage> {
        if let Some(file) = file {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(file)
                .map_err(|e| e.to_string())?
                .take(MAX_ENCODED_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.is_empty() || bytes.len() > MAX_ENCODED_BYTES {
                return Err("Cosmetic encoded image budget".into());
            }
            let reader = image::ImageReader::new(Cursor::new(&bytes))
                .with_guessed_format()
                .map_err(|e| e.to_string())?;
            let (w, h) = reader.into_dimensions().map_err(|e| e.to_string())?;
            if w > 1024
                || h > 1024
                || self.studio_pixel_bytes() + w as usize * h as usize * 4 + extra
                    > COSMETIC_PIXEL_BUDGET
            {
                return Err("Cosmetic pixel budget".into());
            }
            decode(&bytes)
        } else {
            let r = self
                .accessories
                .get(&id)
                .ok_or("Cosmetic art unavailable")?;
            if self.studio_pixel_bytes() + r.image.pixels.len() + extra > COSMETIC_PIXEL_BUDGET {
                return Err("Cosmetic pixel budget".into());
            }
            Ok((*r.image).clone())
        }
    }
    fn resource(&mut self, id: u8, image: RgbaImage, thumbnail: bool) -> Result<Resource> {
        if self.studio_pixel_bytes() + image.pixels.len() > COSMETIC_PIXEL_BUDGET {
            return Err("Cosmetic pixel budget".into());
        }
        let revision = self.studio.revisions[usize::from(id)]
            .checked_add(1)
            .ok_or("Cosmetic revision overflow")?;
        self.studio.revisions[usize::from(id)] = revision;
        Ok(Resource {
            asset: BodyAsset {
                handle: crate::bank::COSMETIC_HANDLE_BASE
                    + if thumbnail { 4096 } else { 2048 }
                    + u64::from(id),
                revision,
                width: image.width,
                height: image.height,
                status: BodyAssetStatus::Ready,
            },
            image: image.into(),
        })
    }
    fn prepare_selected(&mut self, selected: Option<u8>) -> Result<()> {
        let Some(id) = selected else {
            self.studio.selected = None;
            return Ok(());
        };
        let (file, stamp) = self.source(id)?;
        if self
            .studio
            .selected
            .as_ref()
            .is_some_and(|(old, _, s)| *old == id && *s == stamp)
        {
            return Ok(());
        }
        // A bound in-memory source is already decoded and admitted.
        let resource = if file.is_none() {
            self.accessories[&id].clone()
        } else {
            let image = self.decoded_source(id, file, 0)?;
            self.resource(id, image, false)?
        };
        self.studio.selected = Some((id, resource, stamp));
        Ok(())
    }
    fn prepare_thumbnail(&mut self, id: u8) -> Result<()> {
        let (file, stamp) = self.source(id)?;
        if self
            .studio
            .thumbnails
            .get(&id)
            .is_some_and(|(_, old)| *old == stamp)
        {
            return Ok(());
        }
        let source = if let Some((selected, r, old)) = &self.studio.selected
            && *selected == id
            && *old == stamp
        {
            if self.studio_pixel_bytes() + r.image.pixels.len() + 128 * 128 * 4
                > COSMETIC_PIXEL_BUDGET
            {
                return Err("Thumbnail pixel budget".into());
            }
            (*r.image).clone()
        } else {
            self.decoded_source(id, file, 128 * 128 * 4)?
        };
        let scale =
            (f64::from(THUMBNAIL_EXTENT) / f64::from(source.width.max(source.height))).min(1.);
        let w = (f64::from(source.width) * scale).round().max(1.) as u32;
        let h = (f64::from(source.height) * scale).round().max(1.) as u32;
        let source = image::RgbaImage::from_raw(source.width, source.height, source.pixels)
            .ok_or("Cosmetic source pixels")?;
        let mut pixels =
            image::imageops::resize(&source, w, h, image::imageops::FilterType::CatmullRom)
                .into_raw();
        for p in pixels.as_chunks_mut::<4>().0 {
            for i in 0..3 {
                p[i] = p[i].min(p[3]);
            }
        }
        let kept: usize = self
            .studio
            .thumbnails
            .values()
            .map(|(r, _)| r.image.pixels.len())
            .sum();
        if kept + pixels.len() > THUMBNAIL_PIXEL_BUDGET {
            return Err("Thumbnail pixel budget".into());
        }
        let resource = self.resource(
            id,
            RgbaImage {
                width: w,
                height: h,
                pixels,
            },
            true,
        )?;
        self.studio.thumbnails.insert(id, (resource, stamp));
        Ok(())
    }
    /// Synchronous requests cannot publish stale work: each decode finishes before the next begins.
    pub fn prepare_studio(&mut self, visible: &[u8], selected: Option<u8>) -> Result<()> {
        if visible.len() > 12
            || visible.iter().any(|id| *id > 31)
            || selected.is_some_and(|id| id > 31)
        {
            return Err("Cosmetic window budget".into());
        }
        self.studio.thumbnails.retain(|id, _| visible.contains(id));
        if let Some(id) = selected {
            match self.prepare_selected(Some(id)) {
                Ok(()) => self.studio.status[usize::from(id)] = CosmeticArtStatus::Ready,
                Err(e) => {
                    self.studio.selected = None;
                    self.studio.status[usize::from(id)] = if e.contains("budget") {
                        CosmeticArtStatus::OverBudget
                    } else {
                        CosmeticArtStatus::Missing
                    };
                }
            }
        } else {
            self.studio.selected = None;
        }
        for &id in visible {
            if let Err(e) = self.prepare_thumbnail(id) {
                self.studio.thumbnails.remove(&id);
                if selected != Some(id) || self.studio.selected.is_none() {
                    self.studio.status[usize::from(id)] = if e.contains("budget") {
                        CosmeticArtStatus::OverBudget
                    } else {
                        CosmeticArtStatus::Missing
                    };
                }
            }
        }
        Ok(())
    }
    /// Committed local choice only; never reconfigure sessions, banks or wire appearances.
    pub fn commit_accessory(&mut self, choice: Option<u8>) -> Result<()> {
        if choice.is_some_and(|id| id > 31) {
            return Err("Accessory must be 0..31".into());
        }
        let visible: Vec<_> = self.studio.thumbnails.keys().copied().collect();
        self.prepare_studio(&visible, choice)?;
        if self.studio.directory.is_some() {
            self.accessories.clear();
        }
        if let Some((id, r, _)) = &self.studio.selected {
            self.accessories.insert(*id, r.clone());
        }
        self.own_accessory = choice;
        Ok(())
    }
    /// Only the requested ready full-size source enters the isolated preview bank.
    pub fn preview_sources_for(&self, choice: Option<u8>) -> Self {
        let mut result = self.preview_sources();
        if let Some(id) = choice {
            let ready = self
                .studio
                .selected
                .as_ref()
                .filter(|(selected, _, _)| *selected == id)
                .map(|(_, r, _)| r)
                .or_else(|| {
                    self.studio
                        .directory
                        .is_none()
                        .then(|| self.accessories.get(&id))
                        .flatten()
                });
            if let Some(r) = ready {
                result.accessories.insert(id, r.clone());
                result.own_accessory = Some(id);
            }
        }
        result
    }
    pub fn preview_accessory(
        &self,
        skin: &SkinDescriptor,
    ) -> Option<(StaticHeadAccessoryGeometry, BodyAsset)> {
        if matches!(skin.face, FaceGeometry::SingleImage(_)) {
            return None;
        }
        let (id, r) = self.selected_accessory(true)?;
        let v = ACCESSORIES[usize::from(id)];
        Some((
            StaticHeadAccessoryGeometry {
                forward: match skin.face {
                    FaceGeometry::Paired(face) => face.eyes.forward,
                    _ => StandardEyeGeometry::default().forward,
                },
                image_scale: v.1,
                pivot_x: v.2,
                pivot_y: v.3,
            },
            r.asset,
        ))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "studio-art-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(path.join("s")).unwrap();
            Self(path)
        }
        fn accessory(&self, id: usize, size: [u32; 2]) {
            let path = self.0.join(format!("s/a_{}.png", ACCESSORIES[id].0));
            image::RgbaImage::from_pixel(size[0], size[1], image::Rgba([100, 120, 200, 180]))
                .save(path)
                .unwrap();
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let path = self.0.canonicalize().unwrap();
            assert!(path.starts_with(std::env::temp_dir().canonicalize().unwrap()));
            assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("studio-art-test-")
            );
            std::fs::remove_dir_all(path).unwrap();
        }
    }
    #[test]
    fn directory_is_lazy_windowed_pins_selected_and_retains_old_picture_pixels() {
        let directory = Directory::new();
        for id in 0..32 {
            directory.accessory(id, [32, 24]);
        }
        let mut art = Cosmetics::directory(&directory.0, None).unwrap();
        assert!(art.accessories.is_empty());
        assert!(art.studio.thumbnails.is_empty());
        art.prepare_studio(&(0..12).collect::<Vec<_>>(), Some(31))
            .unwrap();
        assert_eq!(art.studio.thumbnails.len(), 12);
        assert_eq!(art.studio.selected.as_ref().unwrap().0, 31);
        assert_eq!(art.own_accessory, None, "browsing must not commit");
        let old = art.studio_thumbnails()[&0].clone();
        let pixels = old.image.pixels.clone();
        art.prepare_studio(&(12..24).collect::<Vec<_>>(), Some(31))
            .unwrap();
        assert!(!art.studio.thumbnails.contains_key(&0));
        assert_eq!(old.image.pixels, pixels);
        assert!(art.preview_sources_for(Some(31)).accessory(true).is_some());
        directory.accessory(0, [48, 24]);
        art.prepare_studio(&[0], Some(31)).unwrap();
        assert!(art.studio_thumbnails()[&0].asset.revision > old.asset.revision);
        assert_eq!(old.image.pixels, pixels);
        std::fs::remove_file(directory.0.join("s/a_hardhat.png")).unwrap();
        art.prepare_studio(&[31], Some(31)).unwrap();
        assert_eq!(art.studio_status(31), CosmeticArtStatus::Missing);
        assert!(art.preview_sources_for(Some(31)).accessory(true).is_none());
        assert!(art.studio.thumbnails.is_empty());
        assert!(art.studio_pixel_bytes() <= COSMETIC_PIXEL_BUDGET);
        assert!(
            art.prepare_studio(&(0..13).collect::<Vec<_>>(), None)
                .is_err()
        );
        assert!(art.prepare_studio(&[32], None).is_err());
    }
    #[test]
    fn invalid_oversized_and_full_cache_keep_choices_but_publish_placeholders() {
        let directory = Directory::new();
        directory.accessory(0, [32, 24]);
        directory.accessory(1, [1024, 1024]);
        directory.accessory(2, [1025, 1]);
        std::fs::write(directory.0.join("s/a_headphones.png"), b"invalid image").unwrap();
        std::fs::write(
            directory.0.join("s/a_eyebrows.png"),
            vec![0; MAX_ENCODED_BYTES + 1],
        )
        .unwrap();
        std::fs::create_dir(directory.0.join("menu")).unwrap();
        let png = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([120, 130, 140, 255]));
        for name in ["skin", "builder", "cosmetic"] {
            png.save(directory.0.join(format!("menu/{name}.png")))
                .unwrap();
        }
        let mut art = Cosmetics::directory(&directory.0, Some(0)).unwrap();
        let held = art.accessory(true).unwrap().clone();
        for id in [1, 2, 3, 4] {
            art.prepare_studio(&[id], Some(id)).unwrap();
            assert!(art.preview_sources_for(Some(id)).accessory(true).is_none());
            assert!(art.studio.thumbnails.is_empty());
            assert_eq!(
                art.studio_status(id),
                if id == 3 {
                    CosmeticArtStatus::Missing
                } else {
                    CosmeticArtStatus::OverBudget
                }
            );
            assert!(art.studio_pixel_bytes() <= COSMETIC_PIXEL_BUDGET);
        }
        assert_eq!(art.own_accessory, Some(0));
        assert_eq!(art.accessory(true).unwrap().asset, held.asset);
        assert_eq!(art.accessory(true).unwrap().image.pixels, held.image.pixels);
    }

    #[test]
    fn lazy_accessory_filter_preserves_the_manifest_path_rules() {
        let directory = Directory::new();
        std::fs::create_dir(directory.0.join("menu")).unwrap();
        std::fs::write(
            directory.0.join("menu/manifest.txt"),
            "menu-art-v1\ncosmetic 1 s/a_local-menu.png\n",
        )
        .unwrap();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([80, 120, 180, 255]))
            .save(directory.0.join("s/a_local-menu.png"))
            .unwrap();
        let art = Cosmetics::directory(&directory.0, None).unwrap();
        assert!(art.menu_art(MenuArt::Cosmetic).is_some());
        assert!(art.accessories.is_empty());
    }
}
