//! Frozen pellet inputs, retained source grouping and the canonical image oracle.
//! Exact rectangles replace seven generic records without changing animation.
//! Study: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-02-retained-pellets/REPORT.md.
use crate::{
    Result,
    frame::{Draw, Frame, ResourceKey},
};
use slither_client::{presentation::*, state::Point};
use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    ops::Range,
    sync::Arc,
};
#[cfg(all(test, windows))]
#[allow(dead_code)]
#[path = "../examples/support/offscreen.rs"]
pub(crate) mod offscreen;

#[derive(Clone, Debug)]
pub(crate) struct Group {
    pub sources: FoodSources,
    pub range: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Instance(pub [[f32; 4]; 6]);
impl Instance {
    // Fold the fixed-layout byte stores into each rewritten mirror slot.
    #[inline(always)]
    pub fn bytes(self) -> [u8; 96] {
        let mut b = [0; 96];
        for (i, v) in self.0.into_iter().flatten().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Sources {
    pub source_identity: FoodSourceIdentity,
    pub groups: Vec<Group>,
    /// Dense world index -> grouped instance slot.
    pub slots: Vec<usize>,
    /// Group of each already grouped slot, independent of animation.
    pub slot_groups: Vec<usize>,
    pub assets: Vec<BodyAsset>,
}

#[derive(Default)]
pub(crate) struct Cache {
    sources: Option<Arc<Sources>>,
    // CPU-only lookup state stays here, not in queued pictures or GPU/worker
    // source objects. Weak-token reclamation therefore stays on the owner thread.
    identities: Vec<FoodStaticIdentity>,
}

impl Cache {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        world: &FoodWorld,
        view: BodyPaintView,
        bounds: Option<FoodCullBounds>,
        assets: &[BodyAsset],
        outlines: &[bool],
        index: &mut FoodAssetIndex,
        limit: usize,
    ) -> Result<Option<Batch>> {
        if world.foods().len() < 1024
            || bounds.is_none()
            || view.camera.x.abs() > 1e9
            || view.camera.y.abs() > 1e9
            || !(0.0001..=100.).contains(&view.zoom)
        {
            return Ok(None);
        }
        let limits = bounds.expect("checked bounds").limits();
        let current = self
            .sources
            .as_ref()
            .filter(|s| s.source_identity == world.source_identity() && s.assets == assets);
        if current.is_none() {
            // Rebuild dense membership but reuse classification at unchanged indices.
            // Weak creation tokens distinguish rollback forks without retaining the
            // mutable food array. Only new/swapped records read full source metadata.
            // https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-03-food-source-groups/REPORT.md.
            let previous = self.sources.as_ref();
            let mut families: Vec<_> = previous
                .into_iter()
                .flat_map(|s| &s.groups)
                .map(|g| Family {
                    sources: g.sources,
                    indices: Vec::new(),
                    same_previous: true,
                })
                .collect();
            let mut by_key: BTreeMap<_, _> = families
                .iter()
                .enumerate()
                .map(|(i, f)| (source_key(f.sources), i))
                .collect();
            for (i, food) in world.foods().iter().enumerate().rev() {
                let inherited = previous.and_then(|p| {
                    self.identities
                        .get(i)
                        .filter(|id| food.matches_static_identity(id))
                        .map(|_| p.slot_groups[p.slots[i]])
                });
                let group = if let Some(group) = inherited {
                    if !families[group].same_previous {
                        return Ok(None);
                    }
                    group
                } else {
                    let f = food.snapshot();
                    if f.position.x.abs() > 1e9 || f.position.y.abs() > 1e9 {
                        return Ok(None);
                    }
                    let s = f.sources;
                    match by_key.entry(source_key(s)) {
                        Entry::Occupied(entry) => {
                            let group = *entry.get();
                            let family = &mut families[group];
                            if family.indices.is_empty() {
                                family.same_previous = family.sources == s;
                                // The first current member owns the exact metadata,
                                // including signed zero, of the original tree path.
                                family.sources = s;
                            } else if family.sources != s {
                                return Ok(None);
                            }
                            group
                        }
                        Entry::Vacant(entry) => {
                            let group = families.len();
                            families.push(Family {
                                sources: s,
                                indices: Vec::new(),
                                same_previous: false,
                            });
                            entry.insert(group);
                            group
                        }
                    }
                };
                let family = &mut families[group];
                if family.indices.is_empty() && !source_admitted(family.sources, outlines) {
                    return Ok(None);
                }
                family.indices.push(i);
            }
            let order: Vec<_> = by_key
                .into_values()
                .filter(|i| !families[*i].indices.is_empty())
                .collect();
            // Compact projection substitutes family metadata. Source creation
            // guarantees finite values and positive full sizes; excluding zero
            // halves makes numeric family equality bit-exact for cached members.
            // Preserve the existing canonical fallback for signed-zero recipes.
            // https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-03-pellet-workers/REPORT.md.
            if order
                .iter()
                .any(|i| source_has_zero_half(&families[*i].sources))
            {
                return Ok(None);
            }
            // Source readiness still covers all retained sources, including culled food.
            let representatives: Vec<_> = order
                .iter()
                .map(|i| world.foods()[families[*i].indices[0]].clone())
                .collect();
            validate_food_with_index(
                FoodPaintInput {
                    foods: &representatives,
                    view,
                    quality: BodyPaintQuality {
                        high_quality: true,
                        blend: 1.,
                    },
                    bounds,
                    assets,
                    target_handle: None,
                    entry_alpha: 1.,
                    command_limit: limit,
                },
                index,
                1.,
            )
            .map_err(|e| e.to_string())?;
            let mut groups = Vec::new();
            let mut grouped = 0;
            let mut slots = vec![0; world.foods().len()];
            let mut slot_groups = Vec::new();
            slot_groups
                .try_reserve_exact(world.foods().len())
                .map_err(|_| crate::frame::DRAW_ALLOCATION_FAILED)?;
            for i in order {
                let family = &families[i];
                let start = grouped;
                for index in &family.indices {
                    slots[*index] = grouped;
                    grouped += 1;
                }
                slot_groups.resize(grouped, groups.len());
                groups.push(Group {
                    sources: family.sources,
                    range: start..grouped,
                });
            }
            let mut identities = Vec::new();
            identities
                .try_reserve_exact(world.foods().len())
                .map_err(|_| "food source identity cache allocation failed")?;
            identities.extend(world.foods().iter().map(FoodPresentation::static_identity));
            self.sources = Some(Arc::new(Sources {
                source_identity: world.source_identity(),
                groups,
                slots,
                slot_groups,
                assets: assets.to_vec(),
            }));
            self.identities = identities;
        }
        let sources = self.sources.as_ref().expect("retained sources").clone();
        let mut visible = vec![false; world.foods().len()];
        let mut count = 0;
        for (i, f) in world.foods().iter().enumerate() {
            let f = f.paint_state();
            let p = f.render_position;
            let inside =
                p.x >= limits[0] && p.x <= limits[1] && p.y >= limits[2] && p.y <= limits[3];
            visible[sources.slots[i]] = inside;
            count += usize::from(inside);
        }
        if count.checked_mul(7).is_none_or(|n| n > limit) {
            return Ok(None);
        }
        Ok(Some(Batch {
            world: world.clone(),
            sources,
            view,
            bounds: bounds.expect("checked bounds"),
            visible,
            visible_count: count,
            early_at: 0,
            late_at: 0,
        }))
    }
}

struct Family {
    sources: FoodSources,
    indices: Vec<usize>,
    same_previous: bool,
}

fn source_key(s: FoodSources) -> [ResourceKey; 4] {
    [
        s.outline.asset().into(),
        s.body.asset().into(),
        s.inner.asset().into(),
        s.outer.asset().into(),
    ]
}

fn source_admitted(s: FoodSources, outlines: &[bool]) -> bool {
    s.outline
        .asset()
        .handle
        .checked_sub(crate::bank::BODY_HANDLE_BASE + 1)
        .and_then(|i| outlines.get(i as usize))
        == Some(&true)
        && ![s.body, s.inner, s.outer].iter().any(|s| {
            s.width() > f64::from(s.asset().width)
                || s.height() > f64::from(s.asset().height)
                || s.half_width().abs() > 1e8
                || s.half_height().abs() > 1e8
        })
}

#[inline]
fn source_has_zero_half(s: &FoodSources) -> bool {
    [&s.body, &s.outline, &s.inner, &s.outer]
        .into_iter()
        .any(|source| source.half_width() == 0. || source.half_height() == 0.)
}

#[cfg(test)]
#[inline(always)]
fn same_source_bits(a: &FoodSources, b: &FoodSources) -> bool {
    a.indices == b.indices
        && [&a.body, &a.outline, &a.inner, &a.outer]
            .into_iter()
            .zip([&b.body, &b.outline, &b.inner, &b.outer])
            .all(|(a, b)| {
                a.asset() == b.asset()
                    && [a.width(), a.height(), a.half_width(), a.half_height()].map(f64::to_bits)
                        == [b.width(), b.height(), b.half_width(), b.half_height()]
                            .map(f64::to_bits)
            })
}

/// Opaque retained inputs. The ordinary image stream remains available as an oracle.
#[derive(Clone, Debug)]
pub struct Batch {
    pub(crate) world: FoodWorld,
    pub(crate) sources: Arc<Sources>,
    pub(crate) view: BodyPaintView,
    pub(crate) bounds: FoodCullBounds,
    pub(crate) visible: Vec<bool>,
    pub(crate) visible_count: usize,
    pub(crate) early_at: usize,
    pub(crate) late_at: usize,
}
impl PartialEq for Batch {
    fn eq(&self, other: &Self) -> bool {
        self.world == other.world
            && self.view == other.view
            && self.bounds == other.bounds
            && self.early_at == other.early_at
            && self.late_at == other.late_at
    }
}
impl Batch {
    pub fn draw_count(&self) -> usize {
        self.visible_count * 7
    }
    pub(crate) fn resource_assets(&self) -> impl Iterator<Item = BodyAsset> + '_ {
        self.sources
            .groups
            .iter()
            .filter(|g| self.visible[g.range.clone()].contains(&true))
            .flat_map(|g| {
                [
                    g.sources.outline.asset(),
                    g.sources.body.asset(),
                    g.sources.inner.asset(),
                    g.sources.outer.asset(),
                ]
            })
    }
    pub(crate) fn validate(&self, resident: &BTreeMap<ResourceKey, BodyAsset>) -> Result<()> {
        for a in self.resource_assets() {
            if resident.get(&a.into()) != Some(&a) {
                return Err("missing, stale or unready resource".into());
            }
        }
        Ok(())
    }
    pub(crate) fn canonical(&self, draws: &[Draw]) -> Result<Vec<Draw>> {
        if self.early_at > self.late_at || self.late_at > draws.len() {
            return Err("invalid retained food barriers".into());
        }
        let count = draws
            .len()
            .checked_add(self.draw_count())
            .ok_or(crate::frame::DRAW_MEMORY_LIMIT)?;
        crate::frame::check_draw_count(count)?;
        let mut index = FoodAssetIndex::default();
        let plan = plan_food_with_index(
            FoodPaintInput {
                foods: self.world.foods(),
                view: self.view,
                quality: BodyPaintQuality {
                    high_quality: true,
                    blend: 1.,
                },
                bounds: Some(self.bounds),
                assets: &self.sources.assets,
                target_handle: None,
                entry_alpha: 1.,
                command_limit: MAX_FOOD_PAINT_COMMANDS,
            },
            &mut index,
            Vec::new(),
            Vec::new(),
        )
        .map_err(|e| {
            if e == FoodPaintError::Allocation {
                crate::frame::DRAW_ALLOCATION_FAILED.to_owned()
            } else {
                e.to_string()
            }
        })?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| crate::frame::DRAW_ALLOCATION_FAILED)?;
        result.extend_from_slice(&draws[..self.early_at]);
        result.extend(plan.early.iter().map(Draw::from));
        result.extend_from_slice(&draws[self.early_at..self.late_at]);
        result.extend(plan.late.iter().map(Draw::from));
        result.extend_from_slice(&draws[self.late_at..]);
        Ok(result)
    }
    pub(crate) fn checked_sources(&self) -> BTreeSet<ResourceKey> {
        self.resource_assets().map(ResourceKey::from).collect()
    }
}

/// Numeric half of the unchanged food image stream. Compute a paired rectangle
/// once, retaining every f64 operation before the existing f32 boundary.
#[inline(always)]
fn project_food(
    frame: &Frame,
    f: FoodPaintSnapshot,
    source: FoodSource,
    center: Point,
    alpha: f64,
) -> Result<crate::frame::Quad> {
    let a = source.asset();
    let (source, destination) =
        match food_image_draw(source, center, f.animation.scale, 1.).map_err(|e| e.to_string())? {
            FoodImageDraw::Whole { x, y } => (
                BodyRect {
                    x: 0.,
                    y: 0.,
                    width: f64::from(a.width),
                    height: f64::from(a.height),
                },
                BodyRect {
                    x,
                    y,
                    width: f64::from(a.width),
                    height: f64::from(a.height),
                },
            ),
            FoodImageDraw::Scaled {
                source,
                destination,
            } => (source, destination),
        };
    frame.project(
        &Draw {
            asset: a,
            corners: None,
            source: Some(source),
            repeat_uv: None,
            black_effect: false,
            destination,
            translation: [0., 0.],
            rotation: 0.,
            effective_alpha: alpha,
            blend: BodyBlend::Lighter,
        },
        source,
    )
}
#[inline(always)]
pub(crate) fn glow_pair(
    frame: &Frame,
    f: impl Into<FoodPaintSnapshot>,
    view: BodyPaintView,
    outer: bool,
    pulse: f64,
) -> Result<(crate::frame::Quad, f32)> {
    let f = f.into();
    let glow = food_glow_paint(f, view, outer, 1., pulse).map_err(|e| e.to_string())?;
    Ok((
        project_food(frame, f, glow.source, glow.center, glow.alpha[0])?,
        glow.alpha[1] as f32,
    ))
}
#[inline(always)]
pub(crate) fn exact_instance(
    frame: &Frame,
    f: impl Into<FoodPaintSnapshot>,
    view: BodyPaintView,
    glows: &[(crate::frame::Quad, f32); 2],
    pulse: f64,
) -> Result<Instance> {
    let f = f.into();
    let center = food_body_center(f, view);
    let body = project_food(frame, f, f.sources.body, center, f.animation.fade)?;
    let pairs = [
        (&body, food_body_alpha(f, 1., pulse)[1] as f32),
        (&glows[0].0, glows[0].1),
        (&glows[1].0, glows[1].1),
    ];
    let mut result = Instance([[0.; 4]; 6]);
    for (i, (q, _)) in pairs.iter().enumerate() {
        result.0[i] = [
            q.vertices[0][0],
            q.vertices[0][1],
            q.vertices[2][0],
            q.vertices[2][1],
        ];
    }
    result.0[3] = [
        body.vertices[0][4],
        pairs[0].1,
        glows[0].0.vertices[0][4],
        glows[0].1,
    ];
    result.0[4] = [
        glows[1].0.vertices[0][4],
        glows[1].1,
        body.vertices[2][2],
        body.vertices[2][3],
    ];
    result.0[5] = [
        glows[0].0.vertices[2][2],
        glows[0].0.vertices[2][3],
        glows[1].0.vertices[2][2],
        glows[1].0.vertices[2][3],
    ];
    Ok(result)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::bank::Bank;

    pub(crate) fn rectangle(bank: &Bank) -> FoodWorld {
        rectangle_count(bank, 25_000)
    }
    pub(crate) fn rectangle_count(bank: &Bank, count: usize) -> FoodWorld {
        let mut world = FoodWorld::new();
        for i in 0..count {
            let factory = bank.food(
                (i % 10) as u8,
                [
                    ((i * 7919) % 65521) as f64 / 65521.,
                    ((i * 1543 + 77) % 65519) as f64 / 65519.,
                ],
            );
            world
                .spawn(
                    FoodSpawn {
                        key: i as u32,
                        sector: (0, 0),
                        position: Point {
                            x: 16_000. + (i % 200) as f64 * 2.,
                            y: 16_000. + (i / 200) as f64 * 2.,
                        },
                        base_size: 0.2,
                        raw_color: (i % 10) as u8,
                        rapid: true,
                    },
                    factory.input(1.),
                )
                .unwrap();
        }
        world
    }
    pub(crate) fn step(delta: f64) -> FrameStep {
        FrameStep {
            delta,
            unlagged_delta: delta,
            whole_steps: delta as u32,
            double_steps: (delta * 2.) as u32,
            lag_multiplier: 1.,
            accumulated: delta,
            doubled_accumulated: delta * 2.,
        }
    }
    pub(crate) fn view() -> BodyPaintView {
        BodyPaintView {
            camera: Point {
                x: 16_200.,
                y: 16_125.,
            },
            zoom: 0.7,
            half_width: 750.,
            half_height: 422.,
        }
    }
    pub(crate) fn batch(
        cache: &mut Cache,
        bank: &mut Bank,
        world: &FoodWorld,
        view: BodyPaintView,
    ) -> Batch {
        cache
            .prepare(
                world,
                view,
                Some(FoodCullBounds::from_view(view).unwrap()),
                &bank.assets,
                &bank.food_outline,
                &mut bank.food_asset_index,
                MAX_FOOD_PAINT_COMMANDS,
            )
            .unwrap()
            .unwrap()
    }

    #[test]
    fn exact_metadata_comparison_covers_every_source_and_axis() {
        let bank = Bank::new().unwrap();
        let world = rectangle_count(&bank, 1024);
        let original = world.foods()[0].snapshot().sources;
        assert!(same_source_bits(&original, &original));
        assert!(!source_has_zero_half(&original));
        for part in 0..4 {
            for axis in 0..2 {
                let mut positive = original;
                let mut negative = original;
                for (sources, zero) in [(&mut positive, 0.), (&mut negative, -0.)] {
                    let source = match part {
                        0 => &mut sources.body,
                        1 => &mut sources.outline,
                        2 => &mut sources.inner,
                        _ => &mut sources.outer,
                    };
                    *source = FoodSource::new(
                        source.asset(),
                        source.width(),
                        source.height(),
                        if axis == 0 { zero } else { source.half_width() },
                        if axis == 1 {
                            zero
                        } else {
                            source.half_height()
                        },
                    )
                    .unwrap();
                }
                assert!(source_has_zero_half(&positive));
                assert!(source_has_zero_half(&negative));
                assert_eq!(positive, negative);
                assert!(!same_source_bits(&positive, &negative));
            }
        }
    }

    #[test]
    fn signed_zero_source_metadata_uses_the_canonical_fallback() {
        let mut bank = Bank::new().unwrap();
        let mut positive = bank.food(0, [0., 0.]);
        for s in &mut positive.bodies {
            *s = FoodSource::new(s.asset(), s.width(), s.height(), 0., s.half_height()).unwrap();
        }
        let mut negative = positive.clone();
        for s in &mut negative.bodies {
            *s = FoodSource::new(s.asset(), s.width(), s.height(), -0., s.half_height()).unwrap();
        }
        let mut world = FoodWorld::new();
        for i in 0..1024 {
            let factory = if i % 2 == 0 { &positive } else { &negative };
            world
                .spawn(
                    FoodSpawn {
                        key: i,
                        sector: (0, 0),
                        position: view().camera,
                        base_size: 0.2,
                        raw_color: 0,
                        rapid: true,
                    },
                    factory.input(1.),
                )
                .unwrap();
        }
        let a = world.foods()[0].snapshot().sources;
        let b = world.foods()[1].snapshot().sources;
        assert_eq!(a, b);
        assert_ne!(a.body.half_width().to_bits(), b.body.half_width().to_bits());
        assert!(
            Cache::default()
                .prepare(
                    &world,
                    view(),
                    Some(FoodCullBounds::from_view(view()).unwrap()),
                    &bank.assets,
                    &bank.food_outline,
                    &mut bank.food_asset_index,
                    MAX_FOOD_PAINT_COMMANDS
                )
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn new_metadata_cannot_replace_a_family_while_cached_members_still_use_it() {
        let mut bank = Bank::new().unwrap();
        let good = bank.food(0, [0., 0.]);
        let mut changed = good.clone();
        for source in &mut changed.bodies {
            *source = FoodSource::new(
                source.asset(),
                source.width() * 0.5,
                source.height(),
                source.half_width(),
                source.half_height(),
            )
            .unwrap();
        }
        let mut world = FoodWorld::new();
        for i in 0..1024 {
            world
                .spawn(
                    FoodSpawn {
                        key: i,
                        sector: (0, 0),
                        position: view().camera,
                        base_size: 0.2,
                        raw_color: 0,
                        rapid: true,
                    },
                    good.input(1.),
                )
                .unwrap();
        }
        let mut cache = Cache::default();
        drop(batch(&mut cache, &mut bank, &world, view()));
        let previous = cache.sources.as_ref().unwrap().clone();
        world
            .spawn(
                FoodSpawn {
                    key: 1024,
                    sector: (0, 0),
                    position: view().camera,
                    base_size: 0.2,
                    raw_color: 0,
                    rapid: true,
                },
                changed.input(1.),
            )
            .unwrap();
        assert!(
            cache
                .prepare(
                    &world,
                    view(),
                    Some(FoodCullBounds::from_view(view()).unwrap()),
                    &bank.assets,
                    &bank.food_outline,
                    &mut bank.food_asset_index,
                    MAX_FOOD_PAINT_COMMANDS
                )
                .unwrap()
                .is_none()
        );
        assert!(Arc::ptr_eq(cache.sources.as_ref().unwrap(), &previous));
        world.consume(1024, FoodConsume::Immediate);
        let restored = batch(&mut cache, &mut bank, &world, view());
        assert_eq!(restored.sources.groups.len(), 1);
    }

    #[test]
    fn failed_source_validation_keeps_previous_lookup_coherent_for_retry() {
        let mut bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        let mut cache = Cache::default();
        let old = batch(&mut cache, &mut bank, &world, view());
        let old_groups = old.sources.slot_groups.clone();
        world.consume(17, FoodConsume::Immediate);
        let mut invalid = bank.assets.clone();
        let handle = world.foods()[0].snapshot().sources.body.asset().handle;
        invalid
            .iter_mut()
            .find(|a| a.handle == handle)
            .unwrap()
            .revision += 1;
        assert!(
            cache
                .prepare(
                    &world,
                    view(),
                    Some(FoodCullBounds::from_view(view()).unwrap()),
                    &invalid,
                    &bank.food_outline,
                    &mut bank.food_asset_index,
                    MAX_FOOD_PAINT_COMMANDS
                )
                .is_err()
        );
        assert!(Arc::ptr_eq(cache.sources.as_ref().unwrap(), &old.sources));
        assert_eq!(cache.sources.as_ref().unwrap().slot_groups, old_groups);
        assert_eq!(cache.identities.len(), old.world.foods().len());
        assert!(
            old.world
                .foods()
                .iter()
                .zip(&cache.identities)
                .all(|(food, id)| food.matches_static_identity(id))
        );
        let retry = batch(&mut cache, &mut bank, &world, view());
        let fresh = batch(&mut Cache::default(), &mut bank, &world, view());
        assert_eq!(retry.sources.slots, fresh.sources.slots);
        assert_eq!(retry.sources.groups.len(), fresh.sources.groups.len());
        for (a, b) in retry.sources.groups.iter().zip(&fresh.sources.groups) {
            assert_eq!(a.sources, b.sources);
            assert_eq!(a.range, b.range);
        }
    }

    #[test]
    fn sibling_creations_with_equal_serials_cannot_reuse_static_classification() {
        let mut bank = Bank::new().unwrap();
        let base = rectangle(&bank);
        let mut first = base.clone();
        let mut second = base;
        for (world, color) in [(&mut first, 0), (&mut second, 3)] {
            let factory = bank.food(color, [0., 0.]);
            world
                .spawn(
                    FoodSpawn {
                        key: 50_000,
                        sector: (0, 0),
                        position: view().camera,
                        base_size: 1.4,
                        raw_color: color,
                        rapid: true,
                    },
                    factory.input(1.),
                )
                .unwrap();
        }
        assert_eq!(
            first.foods().last().unwrap().snapshot().instance,
            second.foods().last().unwrap().snapshot().instance
        );
        let mut cache = Cache::default();
        drop(batch(&mut cache, &mut bank, &first, view()));
        drop(first);
        let changed = batch(&mut cache, &mut bank, &second, view());
        let fresh = batch(&mut Cache::default(), &mut bank, &second, view());
        assert_eq!(changed.sources.slots, fresh.sources.slots);
        assert_eq!(changed.sources.groups.len(), fresh.sources.groups.len());
        for (a, b) in changed.sources.groups.iter().zip(&fresh.sources.groups) {
            assert_eq!(a.sources, b.sources);
            assert_eq!(a.range, b.range);
        }
    }

    #[test]
    fn zero_half_fallback_does_not_publish_a_lookup() {
        let mut bank = Bank::new().unwrap();
        let mut factory = bank.food(0, [0., 0.]);
        for source in &mut factory.bodies {
            *source = FoodSource::new(
                source.asset(),
                source.width(),
                source.height(),
                -0.,
                source.half_height(),
            )
            .unwrap();
        }
        let mut world = FoodWorld::new();
        for i in 0..1025 {
            world
                .spawn(
                    FoodSpawn {
                        key: i,
                        sector: (0, 0),
                        position: view().camera,
                        base_size: 0.2,
                        raw_color: 0,
                        rapid: true,
                    },
                    factory.input(1.),
                )
                .unwrap();
        }
        let mut cache = Cache::default();
        for remove in [false, true] {
            if remove {
                world.consume(1024, FoodConsume::Immediate);
            }
            assert!(
                cache
                    .prepare(
                        &world,
                        view(),
                        Some(FoodCullBounds::from_view(view()).unwrap()),
                        &bank.assets,
                        &bank.food_outline,
                        &mut bank.food_asset_index,
                        MAX_FOOD_PAINT_COMMANDS
                    )
                    .unwrap()
                    .is_none()
            );
            assert!(cache.sources.is_none());
            assert!(cache.identities.is_empty());
        }
    }

    #[test]
    fn static_admission_rejects_bad_metadata_in_first_or_later_family_members() {
        let mut bank = Bank::new().unwrap();
        let good = bank.food(0, [0., 0.]);
        let mut bad = good.clone();
        for source in &mut bad.bodies {
            *source = FoodSource::new(
                source.asset(),
                f64::from(source.asset().width) + 1.,
                source.height(),
                source.half_width(),
                source.half_height(),
            )
            .unwrap();
        }
        for bad_index in [0, 1023] {
            let mut world = FoodWorld::new();
            for i in 0..1024 {
                world
                    .spawn(
                        FoodSpawn {
                            key: i,
                            sector: (0, 0),
                            position: view().camera,
                            base_size: 1.4,
                            raw_color: 0,
                            rapid: true,
                        },
                        if i == bad_index {
                            bad.input(1.)
                        } else {
                            good.input(1.)
                        },
                    )
                    .unwrap();
            }
            assert!(
                Cache::default()
                    .prepare(
                        &world,
                        view(),
                        Some(FoodCullBounds::from_view(view()).unwrap()),
                        &bank.assets,
                        &bank.food_outline,
                        &mut bank.food_asset_index,
                        MAX_FOOD_PAINT_COMMANDS
                    )
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn source_grouping_preserves_tree_key_and_reverse_world_order_after_removal() {
        let mut bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        let mut cache = Cache::default();
        for round in 0..4 {
            if round > 0 {
                for key in [17, 202, 1009] {
                    world.consume(key, FoodConsume::Immediate);
                }
                world.advance(step(5.), |_| unreachable!()).unwrap();
            }
            let mut ordered = BTreeMap::<[ResourceKey; 4], (FoodSources, Vec<usize>)>::new();
            for (i, food) in world.foods().iter().enumerate().rev() {
                let s = food.snapshot().sources;
                let key = [
                    s.outline.asset().into(),
                    s.body.asset().into(),
                    s.inner.asset().into(),
                    s.outer.asset().into(),
                ];
                ordered
                    .entry(key)
                    .or_insert_with(|| (s, Vec::new()))
                    .1
                    .push(i);
            }
            // Fresh rebuilds must preserve validation, group and dense-slot order.
            for _ in 0..3 {
                let b = batch(&mut cache, &mut bank, &world, view());
                assert_eq!(b.sources.groups.len(), ordered.len());
                let mut slot = 0;
                for (group, (_, (sources, indices))) in b.sources.groups.iter().zip(&ordered) {
                    assert_eq!(group.sources, *sources);
                    assert_eq!(group.range, slot..slot + indices.len());
                    for index in indices {
                        assert_eq!(b.sources.slots[*index], slot);
                        slot += 1;
                    }
                }
                assert_eq!(slot, world.foods().len());
            }
        }
    }

    #[test]
    fn rectangle_retains_sources_and_canonical_order_across_animation_and_camera_changes() {
        let mut bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        let mut cache = Cache::default();
        for _ in 0..12 {
            world.advance(step(5.), |_| unreachable!()).unwrap();
        }
        let first = batch(&mut cache, &mut bank, &world, view());
        let sources = first.sources.clone();
        for n in 0..8 {
            world
                .advance(step(2.0833333333333335), |_| unreachable!())
                .unwrap();
            let view = BodyPaintView {
                camera: Point {
                    x: 16_200. + n as f64 * 0.37,
                    y: 16_125. - n as f64 * 0.23,
                },
                ..view()
            };
            let b = batch(&mut cache, &mut bank, &world, view);
            for (i, food) in world.foods().iter().enumerate() {
                let slot = b.sources.slots[i];
                let group = &b.sources.groups[b.sources.slot_groups[slot]];
                assert!(group.range.contains(&slot));
                let expected = food.snapshot().sources;
                assert_eq!(group.sources, expected);
                assert!(same_source_bits(&group.sources, &expected));
            }
            assert!(Arc::ptr_eq(&sources, &b.sources));
            assert_eq!(sources.source_identity, world.source_identity());
            let plan = plan_food_with_index(
                FoodPaintInput {
                    foods: world.foods(),
                    view,
                    quality: BodyPaintQuality {
                        high_quality: true,
                        blend: 1.,
                    },
                    bounds: Some(b.bounds),
                    assets: &bank.assets,
                    target_handle: None,
                    entry_alpha: 1.,
                    command_limit: MAX_FOOD_PAINT_COMMANDS,
                },
                &mut FoodAssetIndex::default(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
            let expected: Vec<_> = plan
                .early
                .iter()
                .chain(&plan.late)
                .map(Draw::from)
                .collect();
            let actual = b.canonical(&[]).unwrap();
            assert_eq!(actual, expected);
            for (a, e) in actual.iter().zip(&expected) {
                assert_eq!(
                    [
                        a.destination.x,
                        a.destination.y,
                        a.destination.width,
                        a.destination.height,
                        a.effective_alpha
                    ]
                    .map(f64::to_bits),
                    [
                        e.destination.x,
                        e.destination.y,
                        e.destination.width,
                        e.destination.height,
                        e.effective_alpha
                    ]
                    .map(f64::to_bits)
                );
            }
            assert_eq!(b.draw_count(), 175_000);
        }
        assert_eq!(first.world.foods()[0].snapshot().fade, 1.);
    }

    #[test]
    fn menu_backdrop_expands_batches_in_order_and_failed_admission_publishes_nothing() {
        use crate::{menu::MenuFrame, render::PreparedFrame};
        let mut bank = Bank::new().unwrap();
        let world = rectangle(&bank);
        let b = batch(&mut Cache::default(), &mut bank, &world, view());
        let frame = Frame {
            identity: 7,
            extent: [1500, 844],
            viewport_scale: 1.,
            tick: 0,
            draws: Vec::new(),
            food_batch: Some(b),
            snakes: Vec::new(),
        };
        let original = PreparedFrame {
            resources: bank.resources(&frame).unwrap(),
            frame,
            overlay: None,
        };
        for overlay in [false, true] {
            let mut actual = original.clone();
            if overlay {
                actual.overlay = Some(Frame {
                    food_batch: None,
                    ..actual.frame.clone()
                });
            }
            let mut expected = actual.clone();
            let composed = MenuFrame::world_crossfade([1500, 844], 1., 0.4)
                .unwrap()
                .compose_over(&actual)
                .unwrap();
            expected.frame = expected.frame.canonical().unwrap();
            MenuFrame::world_crossfade([1500, 844], 1., 0.4)
                .unwrap()
                .append_to(&mut expected)
                .unwrap();
            assert!(
                MenuFrame::append_optional(
                    MenuFrame::world_crossfade([1500, 844], 1., 0.4),
                    &mut actual,
                )
                .unwrap()
            );
            assert_eq!(actual.frame, expected.frame);
            assert_eq!(actual.overlay, expected.overlay);
            assert_eq!(actual.frame, composed.frame);
            assert_eq!(actual.overlay, composed.overlay);
            assert!(actual.frame.food_batch.is_none());
        }

        let mut cover_only = original.clone();
        let mut page = MenuFrame::world_crossfade([1500, 844], 1., 0.4).unwrap();
        page.backdrop = None;
        let cover = page.world_cover.clone().unwrap();
        page.append_to(&mut cover_only).unwrap();
        assert!(cover_only.frame.food_batch.is_some());
        let mut expected = original.frame.canonical().unwrap().draws;
        expected.push(cover);
        assert_eq!(cover_only.frame.canonical().unwrap().draws, expected);

        for invalid_barrier in [false, true] {
            let mut rejected = original.clone();
            let b = rejected.frame.food_batch.as_mut().unwrap();
            if invalid_barrier {
                b.late_at = 1;
            } else {
                b.visible_count = crate::frame::MAX_DRAWS;
            }
            let before = rejected.frame.clone();
            let resources: Vec<_> = rejected.resources.iter().map(|r| r.asset).collect();
            assert!(
                MenuFrame::world_crossfade([1500, 844], 1., 0.4)
                    .unwrap()
                    .append_to(&mut rejected)
                    .is_err()
            );
            assert_eq!(rejected.frame, before);
            assert_eq!(
                rejected
                    .resources
                    .iter()
                    .map(|r| r.asset)
                    .collect::<Vec<_>>(),
                resources
            );
        }
    }

    #[test]
    fn static_cache_does_not_share_live_food_but_queued_pictures_stay_frozen() {
        let mut bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        let mut cache = Cache::default();
        let picture = batch(&mut cache, &mut bank, &world, view());
        let frozen = picture.world.foods()[0].snapshot();
        let shared = world.foods().as_ptr();
        assert_eq!(picture.world.foods().as_ptr(), shared);
        world.consume(frozen.key, FoodConsume::Eaten(None));
        assert_ne!(world.foods().as_ptr(), shared);
        assert_eq!(picture.world.foods()[0].snapshot(), frozen);
        drop(picture);

        // A rendered picture may go away while its static source cache remains.
        // That cache must not force a world-size COW on the next packet.
        // Rebuild against the current array so an accidental old cache snapshot
        // cannot pass merely because the queued-picture check already detached it.
        cache = Cache::default();
        let picture = batch(&mut cache, &mut bank, &world, view());
        drop(picture);
        let unique = world.foods().as_ptr();
        world.consume(frozen.key + 1, FoodConsume::Eaten(None));
        assert_eq!(world.foods().as_ptr(), unique);
        assert!(cache.sources.is_some());
    }

    #[test]
    fn source_epoch_distinguishes_removal_replacement_and_rollback_forks() {
        let mut bank = Bank::new().unwrap();
        let original = rectangle(&bank);
        let mut cache = Cache::default();
        let mut original = original;
        for _ in 0..12 {
            original.advance(step(5.), |_| unreachable!()).unwrap();
        }
        let old = batch(&mut cache, &mut bank, &original, view());
        let mut fork = original.clone();
        fork.consume(17, FoodConsume::Immediate);
        let f = bank.food(3, [0.3, 0.4]);
        fork.spawn(
            FoodSpawn {
                key: 17,
                sector: (0, 0),
                position: Point {
                    x: 16_111.,
                    y: 16_222.,
                },
                base_size: 4.,
                raw_color: 3,
                rapid: false,
            },
            f.input(1.),
        )
        .unwrap();
        assert_eq!(original.foods().len(), fork.foods().len());
        assert_ne!(original.source_identity(), fork.source_identity());
        let changed = batch(&mut cache, &mut bank, &fork, view());
        assert!(!Arc::ptr_eq(&old.sources, &changed.sources));
        let restored = batch(&mut cache, &mut bank, &original, view());
        assert_eq!(
            restored.canonical(&[]).unwrap(),
            old.canonical(&[]).unwrap()
        );
        assert_eq!(original.foods()[17].snapshot().base_size, 0.2);
    }

    #[test]
    fn compact_dynamic_rectangles_and_alphas_match_the_image_oracle_by_bits() {
        let bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        let frame = Frame {
            identity: 1,
            extent: [1500, 844],
            viewport_scale: 1.,
            tick: 0,
            draws: Vec::new(),
            food_batch: None,
            snakes: Vec::new(),
        };
        for _ in 0..12 {
            world.advance(step(2.31), |_| unreachable!()).unwrap();
            for i in (0..world.foods().len()).step_by(179) {
                let food = world.foods()[i].snapshot();
                let pulse = food_pulse(food);
                let glows = [
                    glow_pair(&frame, food, view(), false, pulse).unwrap(),
                    glow_pair(&frame, food, view(), true, pulse).unwrap(),
                ];
                let packed = exact_instance(&frame, food, view(), &glows, pulse).unwrap();
                let plan = plan_food(FoodPaintInput {
                    foods: &world.foods()[i..i + 1],
                    view: view(),
                    quality: BodyPaintQuality {
                        high_quality: true,
                        blend: 1.,
                    },
                    bounds: Some(FoodCullBounds::from_view(view()).unwrap()),
                    assets: &bank.assets,
                    target_handle: None,
                    entry_alpha: 1.,
                    command_limit: 7,
                })
                .unwrap();
                let resident = bank
                    .assets
                    .iter()
                    .map(|a| (ResourceKey::from(*a), *a))
                    .collect();
                let oracle = Frame {
                    draws: plan
                        .early
                        .iter()
                        .chain(&plan.late)
                        .map(Draw::from)
                        .collect(),
                    ..frame.clone()
                }
                .quads(&resident)
                .unwrap();
                for (part, first) in [1, 3, 5].into_iter().enumerate() {
                    let q = &oracle[first];
                    let second = &oracle[first + 1];
                    assert_eq!(
                        packed.0[part].map(f32::to_bits),
                        [
                            q.vertices[0][0],
                            q.vertices[0][1],
                            q.vertices[2][0],
                            q.vertices[2][1]
                        ]
                        .map(f32::to_bits)
                    );
                    let actual = match part {
                        0 => [
                            packed.0[3][0],
                            packed.0[3][1],
                            packed.0[4][2],
                            packed.0[4][3],
                        ],
                        1 => [
                            packed.0[3][2],
                            packed.0[3][3],
                            packed.0[5][0],
                            packed.0[5][1],
                        ],
                        _ => [
                            packed.0[4][0],
                            packed.0[4][1],
                            packed.0[5][2],
                            packed.0[5][3],
                        ],
                    };
                    assert_eq!(
                        actual.map(f32::to_bits),
                        [
                            q.vertices[0][4],
                            second.vertices[0][4],
                            q.vertices[2][2],
                            q.vertices[2][3]
                        ]
                        .map(f32::to_bits)
                    );
                }
            }
        }
    }

    #[test]
    fn consumed_motion_is_dynamic_and_expiry_rebuilds_the_dense_source_order() {
        let mut bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        let mut cache = Cache::default();
        for _ in 0..12 {
            world.advance(step(5.), |_| unreachable!()).unwrap();
        }
        let old = batch(&mut cache, &mut bank, &world, view());
        assert_eq!(old.visible_count, 25_000);
        let token = SnakeObjectToken::new(1).unwrap();
        world.consume(12, FoodConsume::Eaten(Some(token)));
        world
            .advance(step(2.), |_| {
                Ok(FoodEaterPose {
                    head: Point {
                        x: 16_150.,
                        y: 16_100.,
                    },
                    head_offset: Point::default(),
                    heading: 0.3,
                    angle_offset: 0.1,
                })
            })
            .unwrap();
        let flight = batch(&mut cache, &mut bank, &world, view());
        assert!(Arc::ptr_eq(&old.sources, &flight.sources));
        assert_eq!(flight.visible_count, 25_000);
        for _ in 0..21 {
            world
                .advance(step(2.), |_| {
                    Ok(FoodEaterPose {
                        head: Point {
                            x: 16_150.,
                            y: 16_100.,
                        },
                        head_offset: Point::default(),
                        heading: 0.3,
                        angle_offset: 0.1,
                    })
                })
                .unwrap();
        }
        let expired = batch(&mut cache, &mut bank, &world, view());
        assert!(!Arc::ptr_eq(&old.sources, &expired.sources));
        assert_eq!(expired.visible_count, 24_999);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "explicit authored offscreen instancing/pixel check"]
    fn authored_rectangle_instancing_stays_within_existing_rounding_budget() {
        use crate::gpu::{Renderer, instance};
        let adapter = pollster::block_on(instance().enumerate_adapters(wgpu::Backends::DX12))
            .into_iter()
            .find(|a| a.get_info().device_type == wgpu::DeviceType::IntegratedGpu)
            .expect("explicit iGPU probe");
        let mut renderer = pollster::block_on(Renderer::from_adapter_with_max_samples(
            adapter,
            wgpu::TextureFormat::Rgba8Unorm,
            4,
        ))
        .unwrap();
        let scope = renderer
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        let mut cache = Cache::default();
        let extent = [1500, 844];
        let target = offscreen::target(&renderer, extent);
        let reference = offscreen::target(&renderer, extent);
        let eater = SnakeObjectToken::new(1).unwrap();
        for tick in 0..30 {
            if tick == 26 {
                // Same-length replacement must invalidate serialized slots too.
                world = rectangle(&bank);
            }
            if tick == 18 {
                for key in (0..25_000).step_by(97) {
                    world.consume(key, FoodConsume::Eaten(Some(eater)));
                }
            }
            world
                .advance(step(5.), |_| {
                    Ok(FoodEaterPose {
                        head: Point {
                            x: 16_190.,
                            y: 16_130.,
                        },
                        head_offset: Point::default(),
                        heading: 0.3,
                        angle_offset: 0.1,
                    })
                })
                .unwrap();
            let view = if matches!(tick, 12 | 26) {
                // Hide part of the mirror, then bring those slots back next frame.
                BodyPaintView {
                    camera: Point {
                        x: 17_350.,
                        ..view().camera
                    },
                    ..view()
                }
            } else {
                view()
            };
            let batch = cache
                .prepare(
                    &world,
                    view,
                    Some(FoodCullBounds::from_view(view).unwrap()),
                    &bank.assets,
                    &bank.food_outline,
                    &mut bank.food_asset_index,
                    MAX_FOOD_PAINT_COMMANDS,
                )
                .unwrap();
            if batch.is_none() {
                continue;
            }
            let compact = Frame {
                identity: tick + 1,
                extent,
                viewport_scale: 1.,
                tick: tick as u32,
                draws: vec![],
                food_batch: batch,
                snakes: Vec::new(),
            };
            let canonical = compact.canonical().unwrap();
            renderer
                .sync_resources(&bank.resources(&compact).unwrap())
                .unwrap();
            renderer
                .render(&compact, &target.create_view(&Default::default()))
                .unwrap();
            renderer
                .render(&canonical, &reference.create_view(&Default::default()))
                .unwrap();
            let actual = offscreen::pixels(&renderer, &target).unwrap();
            let expected = offscreen::pixels(&renderer, &reference).unwrap();
            let mut active = Vec::new();
            for (a, b) in actual
                .as_chunks::<4>()
                .0
                .iter()
                .zip(expected.as_chunks::<4>().0)
            {
                if a[..3].iter().chain(&b[..3]).any(|v| *v != 0) {
                    active.extend((0..3).map(|i| a[i].abs_diff(b[i])));
                }
            }
            assert!(!active.is_empty());
            active.sort_unstable();
            let maximum = active.last().copied().unwrap();
            let p95 = active[(active.len() - 1) * 95 / 100];
            let state = if world.foods().iter().any(|f| f.snapshot().eaten) {
                "consumed"
            } else if world.foods().iter().all(|f| f.snapshot().fade == 1.) {
                "settled"
            } else {
                "fading"
            };
            println!(
                "authored rectangle tick={tick} state={state}: active RGB max={maximum}, p95={p95}"
            );
            assert!(
                maximum <= 2,
                "instanced pixels exceed existing paired-addition rounding"
            );
        }
        assert!(pollster::block_on(scope.pop()).is_none());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "explicit authored offscreen timestamp and prefix validation"]
    fn aggregate_queries_and_nonblack_prefix_use_complete_retained_picture() {
        use crate::gpu::{Renderer, instance};
        let adapter = pollster::block_on(instance().enumerate_adapters(wgpu::Backends::DX12))
            .into_iter()
            .find(|a| a.get_info().device_type == wgpu::DeviceType::IntegratedGpu)
            .expect("explicit iGPU probe");
        let mut renderer = pollster::block_on(Renderer::from_adapter_for_diagnostics(
            adapter,
            wgpu::TextureFormat::Rgba8Unorm,
            4,
            true,
        ))
        .unwrap();
        let scope = renderer
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        for _ in 0..12 {
            world.advance(step(5.), |_| unreachable!()).unwrap();
        }
        let b = batch(&mut Cache::default(), &mut bank, &world, view());
        let extent = [1500, 844];
        let mut frame = Frame {
            identity: 1,
            extent,
            viewport_scale: 1.,
            tick: 0,
            draws: Vec::new(),
            food_batch: Some(b),
            snakes: Vec::new(),
        };
        let target = offscreen::target(&renderer, extent);
        let mut resources = bank.resources(&frame).unwrap();
        renderer.sync_resources(&resources).unwrap();
        renderer
            .render(&frame, &target.create_view(&Default::default()))
            .unwrap();
        assert!(
            renderer.world_pass_count() > 1,
            "authored scene must exercise glow passes"
        );
        // Do not begin_diagnostic_frame: query 0 must survive the glow passes,
        // and query 1 is written only after the final world pass.
        let resolve = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        encoder.resolve_query_set(renderer.diagnostic_timestamps().unwrap(), 0..2, &resolve, 0);
        encoder.copy_buffer_to_buffer(&resolve, 0, &read, 0, 16);
        renderer.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            send.send(r).unwrap();
        });
        renderer
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        receive.recv().unwrap().unwrap();
        let bytes = read.slice(..).get_mapped_range().unwrap();
        let start = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        let end = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        assert!(end > start);
        drop(bytes);
        read.unmap();

        let page = crate::menu::MenuFrame::world_crossfade(extent, 1., 1.).unwrap();
        frame.draws.push(page.backdrop.clone().unwrap());
        let b = frame.food_batch.as_mut().unwrap();
        b.early_at += 1;
        b.late_at += 1;
        resources.extend(page.resources);
        renderer.sync_resources(&resources).unwrap();
        let canonical = frame.canonical().unwrap();
        let reference = offscreen::target(&renderer, extent);
        renderer
            .render(&frame, &target.create_view(&Default::default()))
            .unwrap();
        renderer
            .render(&canonical, &reference.create_view(&Default::default()))
            .unwrap();
        assert_eq!(
            offscreen::pixels(&renderer, &target).unwrap(),
            offscreen::pixels(&renderer, &reference).unwrap()
        );
        assert!(pollster::block_on(scope.pop()).is_none());
    }
}
