//! Local, serial decoding for a twelve-cell window and one pinned draft selection.
use super::*;
use std::{path::PathBuf, time::SystemTime};

pub const THUMBNAIL_PIXEL_BUDGET: usize = 4 * 1024 * 1024;
const THUMBNAIL_EXTENT: u32 = 128;
const ART_CAPACITY: &str = "Cosmetic pixel budget";
const THUMBNAIL_CAPACITY: &str = "Thumbnail pixel budget";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CosmeticArtStatus {
    Ready,
    #[default]
    Missing,
    OverBudget,
}
impl CosmeticArtStatus {
    fn failure(error: &str) -> Self {
        if error.contains("budget") {
            Self::OverBudget
        } else {
            Self::Missing
        }
    }
    fn error(self) -> String {
        if self == Self::OverBudget {
            "Cosmetic image budget"
        } else {
            "Cosmetic art unavailable"
        }
        .into()
    }
}
#[derive(Clone, PartialEq, Eq)]
enum Stamp {
    File(u64, SystemTime),
    Bound(crate::frame::ResourceKey),
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct RetainedCapacity {
    art: usize,
    thumbnails: usize,
}
struct FailedArt {
    stamp: Stamp,
    status: CosmeticArtStatus,
    capacity: Option<RetainedCapacity>,
}
impl FailedArt {
    fn new(stamp: Stamp, error: &str, capacity: RetainedCapacity) -> Self {
        // Source limits are fixed; only admission against retained pixels can improve after eviction.
        Self {
            stamp,
            status: CosmeticArtStatus::failure(error),
            capacity: matches!(error, ART_CAPACITY | THUMBNAIL_CAPACITY).then_some(capacity),
        }
    }
    fn matches(&self, stamp: &Stamp, capacity: RetainedCapacity) -> bool {
        self.stamp == *stamp && self.capacity.is_none_or(|previous| previous == capacity)
    }
}
#[derive(Default)]
pub(super) struct StudioArt {
    pub directory: Option<PathBuf>,
    thumbnails: BTreeMap<u8, (Resource, Stamp)>,
    selected: Option<(u8, Resource, Stamp)>,
    // Full art and thumbnails have different peak allocation requirements.
    selected_failures: BTreeMap<u8, FailedArt>,
    thumbnail_failures: BTreeMap<u8, FailedArt>,
    #[cfg(test)]
    source_reads: std::cell::Cell<usize>,
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
        self.studio.selected_failures.clear();
        self.studio.thumbnail_failures.clear();
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
    fn retained_capacity(&self) -> RetainedCapacity {
        RetainedCapacity {
            art: self.studio_pixel_bytes(),
            thumbnails: self
                .studio
                .thumbnails
                .values()
                .map(|(r, _)| r.image.pixels.len())
                .sum(),
        }
    }
    fn decoded_source(&self, id: u8, file: Option<PathBuf>, extra: usize) -> Result<RgbaImage> {
        if let Some(file) = file {
            use std::io::Read;
            if file.metadata().map_err(|e| e.to_string())?.len() > MAX_ENCODED_BYTES as u64 {
                return Err("Cosmetic encoded image budget".into());
            }
            #[cfg(test)]
            self.studio
                .source_reads
                .set(self.studio.source_reads.get() + 1);
            let mut bytes = Vec::new();
            std::fs::File::open(file)
                .map_err(|e| e.to_string())?
                .take(MAX_ENCODED_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            let (w, h) = bounded_reader(&bytes, 1024)?
                .into_dimensions()
                .map_err(image_error)?;
            if self.studio_pixel_bytes() + w as usize * h as usize * 4 + extra
                > COSMETIC_PIXEL_BUDGET
            {
                return Err(ART_CAPACITY.into());
            }
            decode(&bytes)
        } else {
            let r = self
                .accessories
                .get(&id)
                .ok_or("Cosmetic art unavailable")?;
            if self.studio_pixel_bytes() + r.image.pixels.len() + extra > COSMETIC_PIXEL_BUDGET {
                return Err(ART_CAPACITY.into());
            }
            Ok((*r.image).clone())
        }
    }
    fn resource(&mut self, id: u8, image: RgbaImage, thumbnail: bool) -> Result<Resource> {
        if self.studio_pixel_bytes() + image.pixels.len() > COSMETIC_PIXEL_BUDGET {
            return Err(ART_CAPACITY.into());
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
    fn prepare_selected(&mut self, selected: Option<u8>, capacity: RetainedCapacity) -> Result<()> {
        let Some(id) = selected else {
            self.studio.selected = None;
            return Ok(());
        };
        let (file, stamp) = self.source(id)?;
        if let Some(failure) = self.studio.selected_failures.get(&id)
            && failure.matches(&stamp, capacity)
        {
            return Err(failure.status.error());
        }
        if self
            .studio
            .selected
            .as_ref()
            .is_some_and(|(old, _, s)| *old == id && *s == stamp)
        {
            return Ok(());
        }
        // A bound in-memory source is already decoded and admitted.
        let loaded = if file.is_none() {
            Ok(self.accessories[&id].clone())
        } else {
            self.decoded_source(id, file, 0)
                .and_then(|image| self.resource(id, image, false))
        };
        let resource = match loaded {
            Ok(resource) => resource,
            Err(error) => {
                self.studio
                    .selected_failures
                    .insert(id, FailedArt::new(stamp, &error, capacity));
                return Err(error);
            }
        };
        self.studio.selected_failures.remove(&id);
        self.studio.selected = Some((id, resource, stamp));
        Ok(())
    }
    fn prepare_thumbnail(&mut self, id: u8, capacity: RetainedCapacity) -> Result<()> {
        let (file, stamp) = self.source(id)?;
        if let Some(failure) = self.studio.thumbnail_failures.get(&id)
            && failure.matches(&stamp, capacity)
        {
            return Err(failure.status.error());
        }
        let result = self.load_thumbnail(id, file, stamp.clone());
        match &result {
            Ok(()) => {
                self.studio.thumbnail_failures.remove(&id);
            }
            Err(error) => {
                self.studio
                    .thumbnail_failures
                    .insert(id, FailedArt::new(stamp, error, capacity));
            }
        }
        result
    }
    fn load_thumbnail(&mut self, id: u8, file: Option<PathBuf>, stamp: Stamp) -> Result<()> {
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
                return Err(THUMBNAIL_CAPACITY.into());
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
            return Err(THUMBNAIL_CAPACITY.into());
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
        self.studio
            .thumbnail_failures
            .retain(|id, _| visible.contains(id));
        // Recompute after actual resource changes, not after cached failures or status updates.
        let mut capacity = self.retained_capacity();
        let previous = self.studio.selected.as_ref().map(|(_, r, _)| r.asset);
        if let Some(id) = selected {
            match self.prepare_selected(Some(id), capacity) {
                Ok(()) => self.studio.status[usize::from(id)] = CosmeticArtStatus::Ready,
                Err(e) => {
                    self.studio.selected = None;
                    self.studio.status[usize::from(id)] = CosmeticArtStatus::failure(&e);
                }
            }
        } else {
            self.studio.selected = None;
        }
        if previous != self.studio.selected.as_ref().map(|(_, r, _)| r.asset) {
            capacity = self.retained_capacity();
        }
        for &id in visible {
            let previous = self.studio.thumbnails.get(&id).map(|(r, _)| r.asset);
            match self.prepare_thumbnail(id, capacity) {
                Ok(()) => {
                    if selected != Some(id) {
                        self.studio.status[usize::from(id)] = CosmeticArtStatus::Ready;
                    }
                }
                Err(e) => {
                    self.studio.thumbnails.remove(&id);
                    if selected != Some(id) || self.studio.selected.is_none() {
                        self.studio.status[usize::from(id)] = CosmeticArtStatus::failure(&e);
                    }
                }
            }
            if previous != self.studio.thumbnails.get(&id).map(|(r, _)| r.asset) {
                capacity = self.retained_capacity();
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
                let mut preview = r.clone();
                preview.asset.handle = crate::bank::STUDIO_COSMETIC_HANDLE_BASE
                    + r.asset.handle % crate::bank::HANDLE_NAMESPACE_SIZE;
                result.accessories.insert(id, preview);
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
        fn menu_art(&self, count: usize) {
            std::fs::create_dir_all(self.0.join("menu")).unwrap();
            let image = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([80, 120, 180, 255]));
            for name in ["skin", "builder", "cosmetic"].into_iter().take(count) {
                image.save(self.0.join(format!("menu/{name}.png"))).unwrap();
            }
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
    fn selection_switch_retries_capacity_failure_after_releasing_the_old_selection() {
        let directory = Directory::new();
        directory.menu_art(3);
        directory.accessory(0, [768, 1024]);
        directory.accessory(1, [768, 1024]);
        let mut art = Cosmetics::directory(&directory.0, None).unwrap();
        art.prepare_studio(&[], Some(0)).unwrap();
        assert_eq!(art.studio_status(0), CosmeticArtStatus::Ready);
        art.prepare_studio(&[], Some(1)).unwrap();
        assert_eq!(art.studio_status(1), CosmeticArtStatus::OverBudget);
        assert!(art.studio.selected.is_none());
        let reads = art.studio.source_reads.get();
        art.prepare_studio(&[], Some(1)).unwrap();
        assert_eq!(art.studio.source_reads.get(), reads + 1);
        assert_eq!(art.studio_status(1), CosmeticArtStatus::Ready);
        assert!(art.preview_sources_for(Some(1)).accessory(true).is_some());
    }
    #[test]
    fn unchanged_capacity_failures_do_not_reread_selected_or_thumbnail_sources() {
        let directory = Directory::new();
        directory.menu_art(3);
        directory.accessory(0, [1024, 1024]);
        let mut art = Cosmetics::directory(&directory.0, None).unwrap();
        art.prepare_studio(&[0], Some(0)).unwrap();
        assert_eq!(art.studio_status(0), CosmeticArtStatus::OverBudget);
        let reads = art.studio.source_reads.get();
        assert_eq!(reads, 2);
        for _ in 0..16 {
            art.prepare_studio(&[0], Some(0)).unwrap();
        }
        assert_eq!(art.studio.source_reads.get(), reads);
        // Both cached failures are invalidated by real retained capacity becoming available.
        art.menu.resident.clear();
        art.prepare_studio(&[0], Some(0)).unwrap();
        assert_eq!(art.studio.source_reads.get(), reads + 1);
        assert_eq!(art.studio_status(0), CosmeticArtStatus::Ready);
        assert!(art.studio.thumbnails.contains_key(&0));
    }
    #[test]
    fn evicting_thumbnails_retries_a_capacity_blocked_source_without_touching_the_file() {
        let directory = Directory::new();
        directory.menu_art(2);
        for id in 0..12 {
            directory.accessory(id, [128, 128]);
        }
        directory.accessory(30, [1000, 1000]);
        directory.accessory(31, [1024, 1024]);
        let mut art = Cosmetics::directory(&directory.0, None).unwrap();
        art.prepare_studio(&(0..12).collect::<Vec<_>>(), Some(31))
            .unwrap();
        let visible = (0..11).chain([30]).collect::<Vec<_>>();
        art.prepare_studio(&visible, Some(31)).unwrap();
        assert_eq!(art.studio_status(30), CosmeticArtStatus::OverBudget);
        let reads = art.studio.source_reads.get();
        for _ in 0..8 {
            art.prepare_studio(&visible, Some(31)).unwrap();
        }
        assert_eq!(art.studio.source_reads.get(), reads);
        art.prepare_studio(&[30], Some(31)).unwrap();
        assert_eq!(art.studio.source_reads.get(), reads + 1);
        assert_eq!(art.studio_status(30), CosmeticArtStatus::Ready);
        assert!(art.studio.thumbnails.contains_key(&30));
    }
    #[test]
    fn unchanged_failed_window_and_selection_do_not_reread_and_replacement_retries() {
        let directory = Directory::new();
        for id in (0..12).chain([31]) {
            std::fs::write(
                directory.0.join(format!("s/a_{}.png", ACCESSORIES[id].0)),
                b"broken",
            )
            .unwrap();
        }
        let mut art = Cosmetics::directory(&directory.0, None).unwrap();
        let visible = (0..12).collect::<Vec<_>>();
        art.prepare_studio(&visible, Some(31)).unwrap();
        let reads = art.studio.source_reads.get();
        assert_eq!(reads, 13);
        for _ in 0..32 {
            art.prepare_studio(&visible, Some(31)).unwrap();
        }
        assert_eq!(art.studio.source_reads.get(), reads);
        assert_eq!(art.studio_status(31), CosmeticArtStatus::Missing);
        assert!(art.studio.thumbnails.is_empty());
        // Different encoded sizes guarantee new stamps even on a coarse timestamp filesystem.
        directory.accessory(0, [32, 24]);
        directory.accessory(31, [48, 24]);
        art.prepare_studio(&visible, Some(31)).unwrap();
        assert_eq!(art.studio.source_reads.get(), reads + 2);
        assert!(art.studio.thumbnails.contains_key(&0));
        assert!(art.preview_sources_for(Some(31)).accessory(true).is_some());
        assert_eq!(art.studio_status(31), CosmeticArtStatus::Ready);
        art.prepare_studio(&visible, Some(31)).unwrap();
        assert_eq!(art.studio.source_reads.get(), reads + 2);
    }
    #[test]
    fn over_budget_failures_keep_their_status_until_replaced() {
        let directory = Directory::new();
        directory.accessory(0, [1025, 1]);
        std::fs::write(
            directory.0.join("s/a_graduation.png"),
            vec![0; MAX_ENCODED_BYTES + 1],
        )
        .unwrap();
        let mut art = Cosmetics::directory(&directory.0, None).unwrap();
        art.prepare_studio(&[0, 1], Some(0)).unwrap();
        let reads = art.studio.source_reads.get();
        for _ in 0..8 {
            art.prepare_studio(&[0, 1], Some(0)).unwrap();
        }
        assert_eq!(art.studio.source_reads.get(), reads);
        assert_eq!(art.studio_status(0), CosmeticArtStatus::OverBudget);
        assert_eq!(art.studio_status(1), CosmeticArtStatus::OverBudget);
        directory.accessory(0, [32, 24]);
        directory.accessory(1, [48, 24]);
        art.prepare_studio(&[0, 1], Some(0)).unwrap();
        assert_eq!(art.studio.source_reads.get(), reads + 2);
        assert_eq!(art.studio_status(0), CosmeticArtStatus::Ready);
        assert_eq!(art.studio_status(1), CosmeticArtStatus::Ready);
        assert_eq!(art.studio.thumbnails.len(), 2);
        assert!(art.studio.selected_failures.is_empty());
        assert!(art.studio.thumbnail_failures.is_empty());
    }
    #[test]
    fn compressed_metadata_is_bounded_during_probe_and_world_decode() {
        use std::io::Write;
        for expanded in [1024, COSMETIC_PIXEL_BUDGET + 1] {
            // Stream synthetic metadata; never allocate the expanded fixture.
            let mut compressed =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            let block = [b'x'; 4096];
            let mut remaining = expanded;
            while remaining > 0 {
                let count = remaining.min(block.len());
                compressed.write_all(&block[..count]).unwrap();
                remaining -= count;
            }
            let compressed = compressed.finish().unwrap();
            for kind in [png::chunk::iCCP, png::chunk::zTXt, png::chunk::iTXt] {
                let mut profile = if kind == png::chunk::iTXt {
                    b"Synthetic\0\x01\0\0\0".to_vec()
                } else {
                    b"Synthetic\0\0".to_vec()
                };
                profile.extend_from_slice(&compressed);
                let mut bytes = Vec::new();
                {
                    let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    let mut writer = encoder.write_header().unwrap();
                    writer.write_chunk(kind, &profile).unwrap();
                    writer.write_image_data(&[50, 100, 150, 255]).unwrap();
                }
                assert!(bytes.len() < MAX_ENCODED_BYTES);
                let probe = bounded_reader(&bytes, 1024)
                    .and_then(|reader| reader.into_dimensions().map_err(image_error));
                let world = decode(&bytes);
                let directory = Directory::new();
                std::fs::write(directory.0.join("s/a_oakley.png"), &bytes).unwrap();
                let mut art = Cosmetics::directory(&directory.0, None).unwrap();
                art.prepare_studio(&[0], Some(0)).unwrap();
                if expanded > COSMETIC_PIXEL_BUDGET {
                    assert!(probe.unwrap_err().contains("budget"));
                    assert!(world.unwrap_err().contains("budget"));
                    assert_eq!(art.studio_status(0), CosmeticArtStatus::OverBudget);
                    assert!(art.studio.thumbnails.is_empty());
                    assert!(art.preview_sources_for(Some(0)).accessory(true).is_none());
                } else {
                    assert_eq!(probe.unwrap(), (1, 1));
                    assert_eq!(world.unwrap().pixels, [50, 100, 150, 255]);
                    assert_eq!(art.studio_status(0), CosmeticArtStatus::Ready);
                    assert_eq!(art.studio.thumbnails.len(), 1);
                }
                assert!(art.studio_pixel_bytes() <= COSMETIC_PIXEL_BUDGET);
            }
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

    #[test]
    fn refreshed_preview_art_composes_with_an_older_world_revision() {
        use crate::{
            frame::Draw, menu::MenuFrame, menu_storage::MenuStorage, render::PreparedFrame,
            skin_studio_native::SkinStudioNative,
        };
        use slither_client::skin_studio::AppearanceRecord;
        let directory = Directory::new();
        directory.accessory(0, [32, 24]);
        let mut art = Cosmetics::directory(&directory.0, Some(0)).unwrap();
        let old = art.accessory(true).unwrap().clone();
        let mut studio = SkinStudioNative::new(
            AppearanceRecord {
                builtin: 7,
                cosmetic: Some(0),
                ..Default::default()
            },
            MenuStorage::at(None),
        )
        .unwrap();
        studio.open(true);
        studio.step(230.);
        directory.accessory(0, [48, 24]);
        studio.sync_art([960., 540.], Some(&mut art)).unwrap();
        let fresh = art
            .preview_sources_for(Some(0))
            .accessory(true)
            .unwrap()
            .asset;
        assert_ne!(fresh.handle, old.asset.handle);
        assert_ne!(fresh.revision, old.asset.revision);
        let mut frame = MenuFrame::world_crossfade([960, 540], 1., 0.)
            .unwrap()
            .frame;
        frame.draws.push(Draw {
            corners: None,
            asset: old.asset,
            source: None,
            repeat_uv: None,
            black_effect: false,
            destination: BodyRect {
                x: 10.,
                y: 10.,
                width: 32.,
                height: 24.,
            },
            translation: [0.; 2],
            rotation: 0.,
            effective_alpha: 1.,
            blend: BodyBlend::SourceOver,
        });
        let mut picture = PreparedFrame {
            frame,
            overlay: None,
            resources: vec![old.clone()],
        };
        studio.append_preview(&mut picture, Some(&art)).unwrap();
        assert!(picture.resources.iter().any(|r| r.asset == old.asset));
        assert!(picture.resources.iter().any(|r| r.asset == fresh));
        picture
            .frame
            .validate(
                &picture
                    .resources
                    .iter()
                    .map(|r| (r.asset.into(), r.asset))
                    .collect(),
            )
            .unwrap();
        assert_eq!(
            art.accessory(true).unwrap().asset,
            old.asset,
            "preview must not commit world art"
        );
        assert_eq!(art.accessory(true).unwrap().image.pixels, old.image.pixels);
    }
}
