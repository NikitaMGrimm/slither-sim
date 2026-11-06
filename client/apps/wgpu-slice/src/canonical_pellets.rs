//! Canonical vertex preparation; ordering and layer admission stay on the caller.
//! Exactness and timing: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-05-join-fade/REPORT.md.
use super::{
    DrawState, Pipeline, Texture, VertexChunk, additive_pair, covered_area, glow_bin, zero_pair,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::frame::{Frame, VERTEX_CHUNK_QUADS};
use crate::{
    Result,
    frame::{QUAD_VERTEX_BYTES, Quad, ResourceKey},
};
use slither_client::presentation::BodyBlend;
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct Packed {
    pub bytes: [u8; QUAD_VERTEX_BYTES],
    pub state: DrawState,
    pub outline: bool,
    pub glow: bool,
    pub area: f64,
    pub unsafe_prefix: bool,
    pub black_prefix: bool,
}
impl Packed {
    #[cfg(not(target_arch = "wasm32"))]
    fn store_body(bytes: &[u8; 96], t: &Texture, chunk: &mut VertexChunk, count: usize) -> Info {
        if chunk.states.is_empty() {
            if chunk.bytes.capacity() < count * QUAD_VERTEX_BYTES {
                chunk.bytes.reserve_exact(count * QUAD_VERTEX_BYTES);
            }
            if chunk.states.capacity() < count {
                chunk.states.reserve_exact(count);
            }
        }
        let start = chunk.bytes.len();
        chunk.bytes.resize(start + QUAD_VERTEX_BYTES, 0);
        let packed = &mut chunk.bytes[start..];
        for i in 0..4 {
            packed[i * 28 + 16..i * 28 + 20].copy_from_slice(&bytes[48..52]);
            packed[i * 28 + 24..i * 28 + 28].copy_from_slice(&t.layer.to_le_bytes());
        }
        packed[..8].copy_from_slice(&bytes[..8]);
        packed[28..32].copy_from_slice(&bytes[8..12]);
        packed[32..36].copy_from_slice(&bytes[4..8]);
        packed[36..40].copy_from_slice(&bytes[72..76]);
        packed[56..64].copy_from_slice(&bytes[8..16]);
        packed[64..72].copy_from_slice(&bytes[72..80]);
        packed[84..88].copy_from_slice(&bytes[..4]);
        packed[88..92].copy_from_slice(&bytes[12..16]);
        packed[96..100].copy_from_slice(&bytes[76..80]);
        let get = |i| f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        chunk.states.push(DrawState {
            pipeline: Pipeline::Lighter,
            key: t.array.key,
            repeat: false,
            glow_scale: 0,
            noop: false,
        });
        Info {
            outline: false,
            glow: false,
            area: 0.,
            scale: 0,
            unsafe_prefix: !(get(0) <= -1. && get(4) >= 1. && get(8) >= 1. && get(12) <= -1.),
            black_prefix: false,
        }
    }

    /// The quad slot a PairLighter consumed; never drawn.
    #[cfg(not(target_arch = "wasm32"))]
    fn store_noop(t: &Texture, chunk: &mut VertexChunk, count: usize) -> Info {
        store(
            Self {
                bytes: [0; QUAD_VERTEX_BYTES],
                state: DrawState {
                    pipeline: Pipeline::Lighter,
                    key: t.array.key,
                    repeat: false,
                    glow_scale: 0,
                    noop: true,
                },
                outline: false,
                glow: false,
                area: 0.,
                unsafe_prefix: false,
                black_prefix: false,
            },
            chunk,
            count,
        )
    }

    fn new(q: &Quad, second: Option<f32>, t: &Texture, soft: bool, attachment: [u32; 2]) -> Self {
        let mut state = DrawState::new(q, t.black);
        if let Some(alpha) = second {
            state.noop = zero_pair(q.vertices[0][4], alpha);
            state.pipeline = Pipeline::PairLighter;
        }
        state.key = t.array.key;
        let outline = soft && t.food_outline;
        let glow = soft && t.food_glow && q.blend == BodyBlend::Lighter;
        if glow {
            state.glow_scale = glow_bin(q, second.unwrap_or(0.), attachment);
        }
        let covers = q.vertices[0][0] <= -1.
            && q.vertices[0][1] >= 1.
            && q.vertices[1][0] >= 1.
            && q.vertices[1][1] >= 1.
            && q.vertices[2][0] >= 1.
            && q.vertices[2][1] <= -1.
            && q.vertices[5][0] <= -1.
            && q.vertices[5][1] <= -1.;
        let mut bytes = [0; QUAD_VERTEX_BYTES];
        for (i, v) in [q.vertices[0], q.vertices[1], q.vertices[2], q.vertices[5]]
            .into_iter()
            .enumerate()
        {
            for (j, value) in v.into_iter().chain([second.unwrap_or(0.)]).enumerate() {
                bytes[i * 28 + j * 4..i * 28 + j * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
            bytes[i * 28 + 24..i * 28 + 28].copy_from_slice(&t.layer.to_le_bytes());
        }
        Self {
            bytes,
            state,
            outline,
            glow,
            area: if outline || glow && state.glow_scale > 0 {
                covered_area(&q.vertices)
            } else {
                0.
            },
            unsafe_prefix: !outline && !covers,
            black_prefix: soft && t.black && q.blend == BodyBlend::SourceOver,
        }
    }
}

pub(super) fn pack<'a>(
    quads: impl Iterator<Item = Result<Quad>> + 'a,
    textures: &'a BTreeMap<ResourceKey, Texture>,
    encoded: bool,
    soft: bool,
    attachment: [u32; 2],
) -> impl Iterator<Item = Result<Packed>> + 'a {
    let mut quads = quads.peekable();
    std::iter::from_fn(move || {
        let q = match quads.next()? {
            Ok(q) => q,
            Err(e) => return Some(Err(e)),
        };
        let second = if encoded
            && quads
                .peek()
                .is_some_and(|next| next.as_ref().is_ok_and(|next| additive_pair(&q, next)))
        {
            Some(quads.next().unwrap().unwrap().vertices[0][4])
        } else {
            None
        };
        Some(Ok(Packed::new(
            &q,
            second,
            &textures[&q.key],
            soft,
            attachment,
        )))
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn prepare(
    frame: &Frame,
    textures: &BTreeMap<ResourceKey, Texture>,
    soft: bool,
    attachment: [u32; 2],
    vertices: &mut [VertexChunk],
) -> Result<Option<Vec<Info>>> {
    let Some(b) = frame
        .food_batch
        .as_ref()
        .filter(|b| b.canonical_submission && b.visible_count >= 2048)
    else {
        return Ok(None);
    };
    frame.check_limits()?;
    let resident = textures.iter().map(|(key, t)| (*key, t.asset)).collect();
    b.validate(&resident)?;
    let ordinary = Frame {
        food_batch: None,
        ..frame.clone()
    };
    let quads = ordinary.quads(&resident)?;
    // Preserve the generic lookahead even for coincident caller-authored draws
    // at an insertion barrier. Product snake/prey sources are disjoint.
    let food_keys = b.checked_sources();
    if quads
        .iter()
        .any(|q| q.blend == BodyBlend::Lighter && food_keys.contains(&q.key))
    {
        return Ok(None);
    }
    let mut surrounding = Vec::new();
    let mut ends = [0; 3];
    // Low inserts nothing at its late barrier, so the draws on both sides stay
    // adjacent and keep the generic pair lookahead across it.
    let ranges = if b.detailed {
        [0..b.early_at, b.early_at..b.late_at, b.late_at..quads.len()]
    } else {
        [
            0..b.early_at,
            b.early_at..quads.len(),
            quads.len()..quads.len(),
        ]
    };
    for (i, range) in ranges.into_iter().enumerate() {
        for p in pack(
            quads[range].iter().cloned().map(Ok),
            textures,
            true,
            soft,
            attachment,
        ) {
            surrounding.push(p?);
        }
        ends[i] = surrounding.len();
    }
    let visible: Vec<_> = b
        .world
        .foods()
        .iter()
        .enumerate()
        .rev()
        .filter(|(i, _)| b.visible[b.sources.slots[*i]])
        .map(|(i, _)| i)
        .collect();
    let source_textures: Vec<_> = b
        .sources
        .groups
        .iter()
        .map(|g| -> Result<_> {
            if !b.visible[g.range.clone()].contains(&true) {
                return Ok(None);
            }
            let get = |source: slither_client::presentation::FoodSource| {
                textures
                    .get(&source.asset().into())
                    .ok_or_else(|| "missing retained pellet source".to_owned())
            };
            Ok(Some([
                get(g.sources.outline)?,
                get(g.sources.body)?,
                get(if b.detailed {
                    g.sources.inner
                } else {
                    g.sources.body
                })?,
                get(if b.detailed {
                    g.sources.outer
                } else {
                    g.sources.body
                })?,
            ]))
        })
        .collect::<Result<_>>()?;
    let n = visible.len();
    // Low's black outlines precede every body on a black clear and cannot
    // change it. Omit their projection/stores but keep bodies in canonical
    // reverse-world order: source grouping changes AMD/DX12 blend rounding.
    // https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-05-low-preset/REPORT.md.
    let skip_outlines = !b.detailed && b.early_at == 0 && soft;
    let early_count = if skip_outlines { 1 } else { 2 } * n;
    let early_end = ends[0] + early_count;
    let late_start = ends[1] + early_count;
    let late_count = if b.detailed { 2 } else { 0 } * n;
    let late_end = late_start + late_count;
    let total = surrounding.len() + early_count + late_count;
    let packed_at = |index: usize, paired: Option<f32>| -> Result<Packed> {
        let (food, part) = if (ends[0]..early_end).contains(&index) {
            let i = index - ends[0];
            (i % n, if skip_outlines { 1 } else { i / n })
        } else if (late_start..late_end).contains(&index) {
            let i = index - late_start;
            (i / 2, 2 + i % 2)
        } else {
            let removed = if index >= late_end {
                early_count + late_count
            } else if index >= early_end {
                early_count
            } else {
                0
            };
            return Ok(surrounding[index - removed].clone());
        };
        let index = visible[food];
        let group = b.sources.slot_groups[b.sources.slots[index]];
        let texture = source_textures[group]
            .as_ref()
            .expect("visible source group")[part];
        let f = slither_client::presentation::FoodPaintSnapshot {
            animation: b.world.foods()[index].paint_state(),
            sources: b.sources.groups[group].sources,
        };
        let (q, second) = b.project_pair(frame, f, part)?;
        Ok(Packed::new(
            &q,
            paired.or(second),
            texture,
            soft,
            attachment,
        ))
    };
    // The generic packer greedily merges each adjacent coincident Lighter pair
    // into one quantized PairLighter draw. Distinct Low foods can coincide, so
    // a body equal to an unpaired predecessor turns that stored quad into the
    // same PairLighter and keeps its own slot as a noop draw: chunk boundaries
    // and claimants stay independent of where pairs fall.
    let bodies = if skip_outlines { ends[0] } else { ends[0] + n }..early_end;
    let unmatched = std::sync::atomic::AtomicBool::new(false);
    let body_texture = |group: usize| {
        source_textures[group]
            .as_ref()
            .expect("visible source group")[1]
    };
    let project_body = |index: usize| -> Result<([u8; 96], usize)> {
        let food = visible[index - bodies.start];
        let slot = b.sources.slots[food];
        let bytes = super::pellet_cpu::project(
            frame,
            super::pellet_cpu::Input {
                slot,
                animation: b.world.foods()[food].paint_state(),
                detailed: false,
            },
            &b.sources,
            b.view,
            b.user_zoom,
            b.physical_pixels_per_unit,
            attachment,
        )?
        .bytes;
        Ok((bytes, b.sources.slot_groups[slot]))
    };
    // Source key, both corners and the UV extent decide `additive_pair` for
    // these axis-aligned, non-repeating Lighter body rectangles.
    let shape = |bytes: &[u8; 96], group: usize| -> (ResourceKey, [f32; 6]) {
        let get = |i: usize| f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        (
            b.sources.groups[group].sources.body.asset().into(),
            [get(0), get(4), get(8), get(12), get(72), get(76)],
        )
    };
    let alpha = |bytes: &[u8; 96]| f32::from_le_bytes(bytes[48..52].try_into().unwrap());
    // Outline/glow-tagged body sources feed layer spans that a noop slot would
    // split; their pairs stay on the generic packer.
    let layered = |group: usize| {
        let t = body_texture(group);
        soft && (t.food_outline || t.food_glow)
    };
    // Packed::new with a second alpha differs from the single quad only in
    // these fields.
    let pair_last = |chunk: &mut VertexChunk, first: f32, second: f32| {
        let start = chunk.bytes.len() - QUAD_VERTEX_BYTES;
        for i in 0..4 {
            chunk.bytes[start + i * 28 + 20..start + i * 28 + 24]
                .copy_from_slice(&second.to_le_bytes());
        }
        let state = chunk.states.last_mut().expect("stored body");
        state.pipeline = Pipeline::PairLighter;
        state.noop = zero_pair(first, second);
    };
    let compute = |chunks: &mut [VertexChunk], first: usize| -> Result<Vec<Info>> {
        let count = total
            .saturating_sub(first * VERTEX_CHUNK_QUADS)
            .min(chunks.len() * VERTEX_CHUNK_QUADS);
        let mut info = Vec::new();
        info.try_reserve_exact(count)
            .map_err(|_| crate::frame::DRAW_ALLOCATION_FAILED)?;
        for (i, chunk) in chunks.iter_mut().enumerate() {
            let start = (first + i) * VERTEX_CHUNK_QUADS;
            let end = (start + VERTEX_CHUNK_QUADS).min(total);
            chunk.bytes.clear();
            chunk.states.clear();
            // A chunk's first body is the consumed half of a pair begun in the
            // previous chunk when an odd number of equal bodies precede it.
            let mut consumed = false;
            if !b.detailed && bodies.contains(&start) && start > bodies.start {
                let (bytes, group) = project_body(start)?;
                let first = shape(&bytes, group);
                let mut run = start;
                while run > bodies.start && {
                    let (bytes, group) = project_body(run - 1)?;
                    shape(&bytes, group) == first
                } {
                    run -= 1;
                }
                consumed = (start - run) % 2 == 1;
            }
            // The last stored body while it is unpaired: shape, alpha, group.
            let mut leader = None;
            for index in start..end {
                if unmatched.load(std::sync::atomic::Ordering::Relaxed) {
                    return Ok(info);
                }
                let count = (total - index).min(VERTEX_CHUNK_QUADS);
                if b.detailed || !bodies.contains(&index) {
                    leader = None;
                    info.push(store(packed_at(index, None)?, chunk, count));
                    continue;
                }
                let (bytes, group) = project_body(index)?;
                let texture = body_texture(group);
                if std::mem::take(&mut consumed) {
                    info.push(Packed::store_noop(texture, chunk, count));
                    continue;
                }
                let current = shape(&bytes, group);
                if let Some((previous, first, _)) = leader.take()
                    && previous == current
                {
                    if layered(group) {
                        unmatched.store(true, std::sync::atomic::Ordering::Relaxed);
                        return Ok(info);
                    }
                    pair_last(chunk, first, alpha(&bytes));
                    info.push(Packed::store_noop(texture, chunk, count));
                    continue;
                }
                leader = Some((current, alpha(&bytes), group));
                // Store ordinary gameplay bodies in their chunk claimant,
                // without an intermediate record pass.
                info.push(
                    if skip_outlines && !texture.food_outline && !texture.food_glow {
                        Packed::store_body(&bytes, texture, chunk, count)
                    } else {
                        store(packed_at(index, None)?, chunk, count)
                    },
                );
            }
            if let Some((previous, first, group)) = leader
                && bodies.contains(&end)
            {
                let (bytes, next_group) = project_body(end)?;
                if shape(&bytes, next_group) == previous {
                    if layered(group) {
                        unmatched.store(true, std::sync::atomic::Ordering::Relaxed);
                        return Ok(info);
                    }
                    pair_last(chunk, first, alpha(&bytes));
                }
            }
        }
        Ok(info)
    };
    let chunk_count = total.div_ceil(VERTEX_CHUNK_QUADS);
    // Low has no pulse/glow projection. Scoped startup costs more than a short
    // serial pass: menu backdrops and fewer than 8,192 bodies stay serial.
    // Measured 4K crossover (~8k–12k bodies), not a visual limit:
    // https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-05-low-preset/REPORT.md.
    let fallback = |vertices: &mut [VertexChunk]| {
        for chunk in vertices {
            chunk.bytes.clear();
            chunk.states.clear();
        }
        None
    };
    if !b.detailed && (!skip_outlines || n < 8192) {
        let info = compute(&mut vertices[..chunk_count], 0)?;
        return Ok(if unmatched.into_inner() {
            fallback(vertices)
        } else {
            Some(info)
        });
    }
    use std::sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    };
    let jobs: Vec<_> = vertices[..chunk_count]
        .iter_mut()
        .map(|chunk| Mutex::new(Some(chunk)))
        .collect();
    let results: Vec<_> = (0..chunk_count).map(|_| OnceLock::new()).collect();
    let next = AtomicUsize::new(0);
    // Outline, body and glow chunks have different projection costs. Claim
    // chunks independently, then merge their metadata in primitive order.
    let drain = || loop {
        let index = next.fetch_add(1, Ordering::Relaxed);
        if index >= chunk_count {
            break;
        }
        let chunk = jobs[index]
            .lock()
            .expect("unclaimed vertex chunk")
            .take()
            .expect("single chunk claimant");
        let value = compute(std::slice::from_mut(chunk), index);
        let _ = results[index].set(value);
    };
    std::thread::scope(|scope| {
        let participants = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
        let mut workers = Vec::new();
        for _ in 1..participants {
            let drain = &drain;
            if let Ok(worker) = std::thread::Builder::new().spawn_scoped(scope, drain) {
                workers.push(worker);
            }
        }
        drain();
        for worker in workers {
            let _ = worker.join();
        }
    });
    drop(jobs);
    if unmatched.into_inner() {
        return Ok(fallback(vertices));
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(total)
        .map_err(|_| crate::frame::DRAW_ALLOCATION_FAILED)?;
    for value in results {
        if let Some(Ok(info)) = value.into_inner() {
            result.extend(info);
        } else {
            return Ok(fallback(vertices));
        }
    }
    Ok(Some(result))
}

#[derive(Clone, Copy)]
pub(super) struct Info {
    pub outline: bool,
    pub glow: bool,
    pub area: f64,
    pub scale: u8,
    pub unsafe_prefix: bool,
    pub black_prefix: bool,
}

pub(super) fn store(p: Packed, chunk: &mut VertexChunk, count: usize) -> Info {
    if chunk.states.is_empty() {
        if chunk.bytes.capacity() < count * QUAD_VERTEX_BYTES {
            chunk.bytes.reserve_exact(count * QUAD_VERTEX_BYTES);
        }
        if chunk.states.capacity() < count {
            chunk.states.reserve_exact(count);
        }
    }
    chunk.bytes.extend_from_slice(&p.bytes);
    chunk.states.push(p.state);
    Info {
        outline: p.outline,
        glow: p.glow,
        area: p.area,
        scale: p.state.glow_scale,
        unsafe_prefix: p.unsafe_prefix,
        black_prefix: p.black_prefix,
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::{
        bank::Bank,
        gpu::{Renderer, instance},
        menu::MenuFrame,
        pellet_batch::{
            Cache,
            tests::{batch, rectangle, step, view},
        },
        render::PreparedFrame,
    };
    use slither_client::{
        presentation::{FoodSpawn, FoodWorld},
        state::Point,
    };

    #[test]
    #[ignore = "explicit authored offscreen catalog and canonical packing comparison"]
    fn prepared_bytes_states_and_coverage_match_the_canonical_stream() {
        let adapter = pollster::block_on(instance().enumerate_adapters(wgpu::Backends::VULKAN))
            .into_iter()
            .find(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu)
            .unwrap();
        let mut r = pollster::block_on(Renderer::from_adapter_with_max_samples(
            adapter,
            wgpu::TextureFormat::Rgba8Unorm,
            4,
        ))
        .unwrap();
        let mut bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        for _ in 0..12 {
            world.advance(step(5.), |_| unreachable!()).unwrap();
        }
        for (zoom, detailed) in [
            (1., true),
            (0.065, true),
            (0.0005, true),
            (1., false),
            (0.065, false),
            (0.0005, false),
        ] {
            let mut b = batch(&mut Cache::default(), &mut bank, &world, view());
            b.user_zoom = zoom;
            b.detailed = detailed;
            let frame = Frame {
                identity: 1,
                extent: [3840, 2160],
                viewport_scale: 1.,
                tick: 0,
                draws: Vec::new(),
                food_batch: Some(b),
                snakes: Vec::new(),
            };
            let mut picture = PreparedFrame {
                background_draws: 0,
                world_draws: 0,
                #[cfg(feature = "frame-bench")]
                food_submission: None,
                resources: bank.resources(&frame).unwrap(),
                frame,
                overlay: None,
            };
            MenuFrame::world_crossfade([3840, 2160], 1., 0.4)
                .unwrap()
                .append_to(&mut picture)
                .unwrap();
            r.sync_resources(&picture.resources).unwrap();
            if !detailed {
                let b = picture.frame.food_batch.as_ref().unwrap();
                let mut chunk = VertexChunk::default();
                for (i, f) in b.world.foods().iter().enumerate() {
                    let slot = b.sources.slots[i];
                    if !b.visible[slot] {
                        continue;
                    }
                    let (q, second) = b
                        .project_pair(&picture.frame, f.snapshot().into(), 1)
                        .unwrap();
                    assert!(second.is_none());
                    let t = &r.textures[&q.key];
                    assert!(!t.food_outline && !t.food_glow);
                    let expected = Packed::new(&q, None, t, true, [3840, 2160]);
                    chunk.bytes.clear();
                    chunk.states.clear();
                    let body = super::super::pellet_cpu::project(
                        &picture.frame,
                        super::super::pellet_cpu::Input {
                            slot,
                            animation: f.paint_state(),
                            detailed: false,
                        },
                        &b.sources,
                        b.view,
                        b.user_zoom,
                        b.physical_pixels_per_unit,
                        [3840, 2160],
                    )
                    .unwrap();
                    let info = Packed::store_body(&body.bytes, t, &mut chunk, 1);
                    assert_eq!(chunk.bytes, expected.bytes);
                    assert_eq!(chunk.states, [expected.state]);
                    assert_eq!(
                        (
                            info.outline,
                            info.glow,
                            info.area.to_bits(),
                            info.scale,
                            info.unsafe_prefix,
                            info.black_prefix
                        ),
                        (
                            expected.outline,
                            expected.glow,
                            expected.area.to_bits(),
                            expected.state.glow_scale,
                            expected.unsafe_prefix,
                            expected.black_prefix
                        )
                    );
                }
            }
            let resident = r.textures.iter().map(|(k, t)| (*k, t.asset)).collect();
            let canonical = picture.frame.canonical().unwrap();
            let expected: Vec<_> = pack(
                canonical.quad_iter(&resident).unwrap(),
                &r.textures,
                true,
                true,
                [3840, 2160],
            )
            .collect::<Result<_>>()
            .unwrap();
            let mut vertices: Vec<_> = (0..picture.frame.draw_count().div_ceil(VERTEX_CHUNK_QUADS))
                .map(|_| VertexChunk::default())
                .collect();
            let actual = prepare(
                &picture.frame,
                &r.textures,
                true,
                [3840, 2160],
                &mut vertices,
            )
            .unwrap()
            .unwrap();
            let bytes: Vec<_> = vertices
                .iter()
                .flat_map(|c| {
                    c.bytes
                        .as_chunks::<QUAD_VERTEX_BYTES>()
                        .0
                        .iter()
                        .map(|v| v.as_slice())
                })
                .collect();
            let states: Vec<_> = vertices.iter().flat_map(|c| &c.states).collect();
            for ((bytes, state), e) in bytes.iter().zip(states).zip(&expected) {
                assert_eq!(*bytes, e.bytes);
                assert_eq!(*state, e.state);
            }
            assert_eq!(actual.len(), expected.len());
            for (a, e) in actual.iter().zip(&expected) {
                assert_eq!(a.scale, e.state.glow_scale);
                assert_eq!(
                    (
                        a.outline,
                        a.glow,
                        a.area.to_bits(),
                        a.unsafe_prefix,
                        a.black_prefix
                    ),
                    (
                        e.outline,
                        e.glow,
                        e.area.to_bits(),
                        e.unsafe_prefix,
                        e.black_prefix
                    )
                );
            }
            let mut frame = picture.frame.clone();
            frame.extent = [1500, 844];
            let canonical = frame.canonical().unwrap();
            let target = crate::pellet_batch::offscreen::target(&r, frame.extent);
            let reference = crate::pellet_batch::offscreen::target(&r, frame.extent);
            r.render(&frame, &target.create_view(&Default::default()))
                .unwrap();
            r.render(&canonical, &reference.create_view(&Default::default()))
                .unwrap();
            assert_eq!(
                crate::pellet_batch::offscreen::pixels(&r, &target).unwrap(),
                crate::pellet_batch::offscreen::pixels(&r, &reference).unwrap()
            );
        }
    }

    /// Authored foods sharing `id` coincide exactly: same position, color,
    /// factory units and spawn time.
    fn coincident(bank: &Bank, count: usize, id: impl Fn(usize) -> usize) -> FoodWorld {
        let mut world = FoodWorld::new();
        for i in 0..count {
            let j = id(i);
            world
                .spawn(
                    FoodSpawn {
                        key: i as u32,
                        sector: (0, 0),
                        position: Point {
                            x: 16_000. + (j % 200) as f64 * 2.,
                            y: 16_000. + (j / 200) as f64 * 2.,
                        },
                        base_size: 0.2,
                        raw_color: (j % 10) as u8,
                        rapid: true,
                    },
                    bank.food((j % 10) as u8, [0.25, 0.5]).input(1.),
                )
                .unwrap();
        }
        for _ in 0..12 {
            world.advance(step(5.), |_| unreachable!()).unwrap();
        }
        world
    }

    fn vulkan() -> Renderer {
        let adapter = pollster::block_on(instance().enumerate_adapters(wgpu::Backends::VULKAN))
            .into_iter()
            .find(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu)
            .unwrap();
        pollster::block_on(Renderer::from_adapter_with_max_samples(
            adapter,
            wgpu::TextureFormat::Rgba8Unorm,
            4,
        ))
        .unwrap()
    }

    fn low_picture(bank: &mut Bank, world: &FoodWorld) -> PreparedFrame {
        let mut b = batch(&mut Cache::default(), bank, world, view());
        assert_eq!(b.visible_count, world.foods().len());
        b.detailed = false;
        b.canonical_submission = true;
        let frame = Frame {
            identity: 1,
            extent: [3840, 2160],
            viewport_scale: 1.,
            tick: 0,
            draws: Vec::new(),
            food_batch: Some(b),
            snakes: Vec::new(),
        };
        PreparedFrame {
            background_draws: 0,
            world_draws: 0,
            #[cfg(feature = "frame-bench")]
            food_submission: None,
            resources: bank.resources(&frame).unwrap(),
            frame,
            overlay: None,
        }
    }

    /// Prepared Low bytes, states and coverage equal the generic packer's
    /// stream, apart from omitted black outlines and the noop slots of
    /// consumed pair quads, and render the canonical pixels. Returns the
    /// numbers of PairLighter draws and of noop slots.
    fn assert_generic(r: &mut Renderer, picture: &PreparedFrame, label: &str) -> (usize, usize) {
        r.sync_resources(&picture.resources).unwrap();
        let resident = r.textures.iter().map(|(k, t)| (*k, t.asset)).collect();
        let canonical = picture.frame.canonical().unwrap();
        let expected: Vec<_> = pack(
            canonical.quad_iter(&resident).unwrap(),
            &r.textures,
            true,
            true,
            [3840, 2160],
        )
        .collect::<Result<_>>()
        .unwrap();
        let mut vertices: Vec<_> = (0..picture.frame.draw_count().div_ceil(VERTEX_CHUNK_QUADS))
            .map(|_| VertexChunk::default())
            .collect();
        let info = prepare(
            &picture.frame,
            &r.textures,
            true,
            [3840, 2160],
            &mut vertices,
        )
        .unwrap()
        .unwrap_or_else(|| panic!("{label}: generic fallback"));
        let mut actual = Vec::new();
        let mut consumed = 0;
        for (i, info) in info.iter().enumerate() {
            let chunk = &vertices[i / VERTEX_CHUNK_QUADS];
            let j = i % VERTEX_CHUNK_QUADS;
            let state = chunk.states[j];
            if state.noop && state.pipeline == Pipeline::Lighter {
                assert!(!info.outline && !info.glow && !info.unsafe_prefix && !info.black_prefix);
                consumed += 1;
                continue;
            }
            actual.push((
                &chunk.bytes[j * QUAD_VERTEX_BYTES..(j + 1) * QUAD_VERTEX_BYTES],
                state,
                *info,
            ));
        }
        let pairs = expected
            .iter()
            .filter(|e| e.state.pipeline == Pipeline::PairLighter)
            .count();
        let omitted = expected.len() - actual.len();
        assert!(
            expected[..omitted].iter().all(|e| e.black_prefix),
            "{label}"
        );
        for (k, ((bytes, state, info), e)) in actual.iter().zip(&expected[omitted..]).enumerate() {
            assert_eq!(*bytes, e.bytes, "{label}: quad {k}");
            assert_eq!(*state, e.state, "{label}: quad {k}");
            assert_eq!(
                (
                    info.outline,
                    info.glow,
                    info.area.to_bits(),
                    info.scale,
                    info.unsafe_prefix,
                    info.black_prefix
                ),
                (
                    e.outline,
                    e.glow,
                    e.area.to_bits(),
                    e.state.glow_scale,
                    e.unsafe_prefix,
                    e.black_prefix
                ),
                "{label}: quad {k}"
            );
        }
        let mut frame = picture.frame.clone();
        frame.extent = [1500, 844];
        let canonical = frame.canonical().unwrap();
        let target = crate::pellet_batch::offscreen::target(r, frame.extent);
        let reference = crate::pellet_batch::offscreen::target(r, frame.extent);
        r.render(&frame, &target.create_view(&Default::default()))
            .unwrap();
        r.render(&canonical, &reference.create_view(&Default::default()))
            .unwrap();
        assert!(
            crate::pellet_batch::offscreen::pixels(r, &target).unwrap()
                == crate::pellet_batch::offscreen::pixels(r, &reference).unwrap(),
            "{label}: pixels"
        );
        (pairs, consumed)
    }

    #[test]
    #[ignore = "explicit authored offscreen catalog and canonical packing comparison"]
    fn coincident_low_bodies_pair_like_the_generic_packer() {
        let mut r = vulkan();
        let mut bank = Bank::new().unwrap();
        // Body order is reverse world order: with 8,200 foods world food i is
        // stream body 8,199 - i, so world foods 8 and 7 straddle the first chunk.
        let run = |first: usize, last: usize| {
            move |i: usize| {
                if (first..=last).contains(&i) {
                    first
                } else {
                    i
                }
            }
        };
        let cases: [(&str, FoodWorld, usize); 8] = [
            (
                "4,096 serial pairs",
                coincident(&bank, 4_096, |i| i / 2),
                2_048,
            ),
            (
                "8,194 scoped pairs",
                coincident(&bank, 8_194, |i| i / 2),
                4_097,
            ),
            (
                "pair across a chunk",
                coincident(&bank, 8_200, run(7, 8)),
                1,
            ),
            (
                "triple from a chunk end",
                coincident(&bank, 8_200, run(6, 8)),
                1,
            ),
            (
                "triple to a chunk start",
                coincident(&bank, 8_200, run(7, 9)),
                1,
            ),
            (
                "four across a chunk",
                coincident(&bank, 8_200, run(5, 8)),
                2,
            ),
            (
                "one pair in 25,000",
                coincident(&bank, 25_000, run(0, 1)),
                1,
            ),
            ("distinct", coincident(&bank, 8_200, |i| i), 0),
        ];
        for (label, world, pairs) in cases {
            for menu in [false, true] {
                let mut picture = low_picture(&mut bank, &world);
                if menu {
                    MenuFrame::world_crossfade([3840, 2160], 1., 0.4)
                        .unwrap()
                        .append_to(&mut picture)
                        .unwrap();
                }
                let label = format!("{label}, menu {menu}");
                assert_eq!(
                    assert_generic(&mut r, &picture, &label),
                    (pairs, pairs),
                    "{label}"
                );
            }
        }
    }

    #[test]
    #[ignore = "explicit authored offscreen catalog and canonical packing comparison"]
    fn caller_pairs_cross_the_empty_low_late_barrier() {
        use slither_client::presentation::{BodyBlend, BodyRect};
        let mut r = vulkan();
        let mut bank = Bank::new().unwrap();
        let world = coincident(&bank, 4_096, |i| i);
        let body = world.foods()[0].snapshot().sources.body.asset();
        for early_at in [0, 1] {
            let mut picture = low_picture(&mut bank, &world);
            // A valid caller resource that is not a food source, so neither the
            // food-key check nor the body pair rule covers it.
            let mut resource = picture
                .resources
                .iter()
                .find(|r| r.asset == body)
                .unwrap()
                .clone();
            resource.asset.handle = 9_000_000_001;
            resource.food_outline = false;
            resource.food_glow = false;
            let mut draw = crate::frame::Draw {
                corners: None,
                asset: resource.asset,
                source: None,
                repeat_uv: None,
                black_effect: false,
                destination: BodyRect {
                    x: 620.3,
                    y: 380.4,
                    width: 74.,
                    height: 74.,
                },
                translation: [0., 0.],
                rotation: 0.,
                effective_alpha: 0.,
                blend: BodyBlend::SourceOver,
            };
            let mut draws = vec![draw.clone(); 16];
            draw.blend = BodyBlend::Lighter;
            draw.effective_alpha = 0.423;
            draws[15] = draw.clone();
            draw.effective_alpha = 0.611;
            draws.push(draw);
            picture.frame.draws = draws;
            let b = picture.frame.food_batch.as_mut().unwrap();
            b.early_at = early_at;
            b.late_at = 16;
            picture.resources.push(resource);
            let label = format!("late barrier pair, early_at {early_at}");
            // The caller pair has no noop slot: the surrounding packer merges it.
            assert_eq!(assert_generic(&mut r, &picture, &label), (1, 0), "{label}");
        }
    }
}
