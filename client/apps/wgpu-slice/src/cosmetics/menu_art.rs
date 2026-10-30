//! Bounded semantic art manifest shared by local file and served-byte resolvers.
use super::*;

pub const MANIFEST_PATH: &str = "menu/manifest.txt";
pub const MANIFEST_LIMIT: usize = 4096;
pub const MAX_LOGO_FRAMES: u8 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MenuArt {
    Logo(u8),
    Skin,
    Builder,
    Cosmetic,
}
impl MenuArt {
    fn ordinal(self) -> u64 {
        match self {
            Self::Logo(n) => u64::from(n),
            Self::Skin => 32,
            Self::Builder => 33,
            Self::Cosmetic => 34,
        }
    }
}
#[derive(Clone, Debug)]
pub struct MenuArtEntry {
    pub art: MenuArt,
    pub revision: u32,
    pub path: String,
}
#[derive(Clone, Debug)]
pub struct MenuManifest(pub Vec<MenuArtEntry>);
impl Default for MenuManifest {
    fn default() -> Self {
        Self(vec![
            MenuArtEntry {
                art: MenuArt::Logo(0),
                revision: 1,
                path: "menu/logo.png".into(),
            },
            MenuArtEntry {
                art: MenuArt::Skin,
                revision: 1,
                path: "menu/skin.png".into(),
            },
            MenuArtEntry {
                art: MenuArt::Builder,
                revision: 1,
                path: "menu/builder.png".into(),
            },
            MenuArtEntry {
                art: MenuArt::Cosmetic,
                revision: 1,
                path: "menu/cosmetic.png".into(),
            },
        ])
    }
}
impl MenuManifest {
    /// UTF-8 lines: `menu-art-v1`, then `role revision relative-path`.
    /// Roles are logo:0..31, skin, builder, cosmetic; paths are local PNG/JPEG.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MANIFEST_LIMIT {
            return Err("Menu manifest byte budget".into());
        }
        let mut lines = std::str::from_utf8(bytes)
            .map_err(|_| "Menu manifest UTF-8")?
            .lines();
        if lines.next() != Some("menu-art-v1") {
            return Err("Menu manifest version".into());
        }
        let mut entries: Vec<MenuArtEntry> = Vec::new();
        let cosmetic_paths = paths();
        for line in lines.filter(|s| !s.is_empty()) {
            if entries.len() == 35 {
                return Err("Menu manifest entry budget".into());
            }
            let fields: Vec<_> = line.split_ascii_whitespace().collect();
            if fields.len() != 3 {
                return Err("Menu manifest fields".into());
            }
            let art = match fields[0] {
                "skin" => MenuArt::Skin,
                "builder" => MenuArt::Builder,
                "cosmetic" => MenuArt::Cosmetic,
                s => MenuArt::Logo(
                    s.strip_prefix("logo:")
                        .and_then(|n| n.parse::<u8>().ok())
                        .filter(|n| *n < MAX_LOGO_FRAMES && format!("logo:{n}") == s)
                        .ok_or("Menu logo frame")?,
                ),
            };
            let revision = fields[1]
                .parse::<u32>()
                .ok()
                .filter(|r| *r > 0 && fields[1].bytes().all(|b| b.is_ascii_digit()))
                .ok_or("Menu art revision")?;
            let path = fields[2];
            // Existing image bindings keep their meaning in both loaders.
            if cosmetic_paths.iter().any(|p| p == path) {
                return Err("Menu manifest path collides with cosmetic".into());
            }
            if !safe_path(path) || entries.iter().any(|e| e.art == art || e.path == path) {
                return Err("Menu manifest duplicate or unsafe path".into());
            }
            entries.push(MenuArtEntry {
                art,
                revision,
                path: path.into(),
            });
        }
        Ok(Self(entries))
    }
}
pub fn safe_path(path: &str) -> bool {
    path.len() <= 128
        && (path.ends_with(".png") || path.ends_with(".jpg"))
        && path
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_.-".contains(&b))
}

#[derive(Default)]
pub(super) struct MenuArtCache {
    pub manifest: MenuManifest,
    pub resident: BTreeMap<MenuArt, Resource>,
    versions: BTreeMap<MenuArt, u64>,
    order: Vec<MenuArt>,
}
impl MenuArtCache {
    pub fn bytes(&self) -> usize {
        self.resident.values().map(|r| r.image.pixels.len()).sum()
    }
    pub fn insert(&mut self, art: MenuArt, image: RgbaImage, other_bytes: usize) -> Result<()> {
        let entry = self
            .manifest
            .0
            .iter()
            .find(|e| e.art == art)
            .ok_or("Unknown menu art")?;
        // Menu slots display at most 450px wide. Keep the existing GPU texture
        // bound without an atlas or eager multi-frame residency. Filtering is
        // on premultiplied pixels, so transparent edges retain their colour.
        let image = if image.width > 512 || image.height > 512 {
            let scale = 512. / f64::from(image.width.max(image.height));
            let width = (f64::from(image.width) * scale).round().max(1.) as u32;
            let height = (f64::from(image.height) * scale).round().max(1.) as u32;
            let source = image::RgbaImage::from_raw(image.width, image.height, image.pixels)
                .ok_or("Menu art pixels")?;
            let resized = image::imageops::resize(
                &source,
                width,
                height,
                image::imageops::FilterType::Triangle,
            );
            RgbaImage {
                width,
                height,
                pixels: resized.into_raw(),
            }
        } else {
            image
        };
        let mut retire = vec![art];
        if matches!(art, MenuArt::Logo(_))
            && !self.resident.contains_key(&art)
            && self
                .order
                .iter()
                .filter(|a| matches!(a, MenuArt::Logo(_)))
                .count()
                == 2
        {
            retire.push(
                *self
                    .order
                    .iter()
                    .find(|a| matches!(a, MenuArt::Logo(_)))
                    .unwrap(),
            );
        }
        let kept: usize = self
            .resident
            .iter()
            .filter(|(a, _)| !retire.contains(a))
            .map(|(_, r)| r.image.pixels.len())
            .sum();
        if other_bytes + kept + image.pixels.len() > COSMETIC_PIXEL_BUDGET {
            return Err("Cosmetic pixel budget".into());
        }
        // Monotonic GPU revisions survive eviction and manifest replacement.
        let revision = self
            .versions
            .get(&art)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Menu revision overflow")?
            .max(u64::from(entry.revision));
        for key in retire {
            self.resident.remove(&key);
            self.order.retain(|a| *a != key);
        }
        self.versions.insert(art, revision);
        self.order.push(art);
        self.resident.insert(
            art,
            Resource {
                asset: BodyAsset {
                    handle: crate::bank::COSMETIC_HANDLE_BASE + 1024 + art.ordinal(),
                    revision,
                    width: image.width,
                    height: image.height,
                    status: BodyAssetStatus::Ready,
                },
                image,
            },
        );
        Ok(())
    }
}
