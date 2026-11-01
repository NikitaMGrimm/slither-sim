//! Same shared renderer inputs from live AnimatedClient. No simulation or transport.
use crate::{
    Result,
    bank::Bank,
    frame::{
        DRAW_MEMORY_LIMIT, Draw, Frame, MAX_DRAWS, MAX_EXTENT, SnakeDrawObservation,
        check_draw_count, push_draw, reserve_draws,
    },
    scene::Resource,
};
use slither_client::{presentation::*, state::Point};

/// Passive diagnostic callback; it must not mutate presentation or submit work.
#[cfg(feature = "frame-profile")]
pub type PreparationObserver = fn(&'static str, u64);
#[cfg(feature = "frame-profile")]
thread_local! {
    static PREPARATION_OBSERVER: std::cell::Cell<Option<PreparationObserver>> = const { std::cell::Cell::new(None) };
}
/// Install a calling-thread observer for finite profiling, returning the previous
/// observer for restoration. This does not add records to latency-trace-v1.
#[cfg(feature = "frame-profile")]
pub fn set_preparation_observer(
    observer: Option<PreparationObserver>,
) -> Option<PreparationObserver> {
    PREPARATION_OBSERVER.with(|slot| slot.replace(observer))
}
macro_rules! preparation_phase {
    ($stage:literal, $identity:expr) => {{
        #[cfg(feature = "frame-profile")]
        if let Some(observer) = PREPARATION_OBSERVER.with(|slot| slot.get()) {
            observer($stage, $identity);
        }
    }};
}

struct PreparationError(String);
impl From<ClientPresentationError> for PreparationError {
    fn from(e: ClientPresentationError) -> Self {
        Self(e.to_string())
    }
}

#[derive(Clone)]
pub struct PreparedFrame {
    pub frame: Frame,
    pub overlay: Option<Frame>,
    pub resources: Vec<Resource>,
}
/// Producer limits bound text resources; they no longer reserve world capacity.
pub const PANEL_PRODUCER_DRAWS: usize = 96;
pub const PANEL_CONTROL_DRAWS: usize = 8;
impl PreparedFrame {
    pub fn pixel_bytes(&self) -> usize {
        self.resources.iter().map(|r| r.image.pixels.len()).sum()
    }
    pub fn draw_count(&self) -> usize {
        self.frame.draws.len() + self.overlay.as_ref().map_or(0, |f| f.draws.len())
    }
    pub fn ui_frame(&self) -> &Frame {
        self.overlay.as_ref().unwrap_or(&self.frame)
    }
    pub fn ui_frame_mut(&mut self) -> &mut Frame {
        self.overlay.as_mut().unwrap_or(&mut self.frame)
    }
}
/// Desktop content size in CSS/logical pixels. DPR affects presentation only.
pub fn backing_size(area: [f64; 2]) -> Result<[u32; 2]> {
    if area.into_iter().any(|n| !n.is_finite() || n < 0.) {
        return Err("Invalid desktop content size".into());
    }
    if area.contains(&0.) {
        return Ok([0, 0]);
    }
    let size = slither_client::desktop_layout::desktop_backing_size(area[0], area[1])?;
    Ok([size.width, size.height])
}
pub fn dimensions(extent: [u32; 2], scale: f64) -> Result<[u32; 2]> {
    if extent.contains(&0)
        || extent.iter().any(|n| *n > MAX_EXTENT)
        || !scale.is_finite()
        || scale <= 0.
    {
        return Err("suspended/invalid backing extent".into());
    }
    let logical = extent.map(|v| (f64::from(v) / scale).ceil());
    if logical
        .iter()
        .any(|n| !n.is_finite() || *n < 1. || *n > f64::from(MAX_EXTENT))
    {
        return Err("logical extent budget".into());
    }
    Ok(logical.map(|n| n as u32))
}
/// Convert body geometry coordinates to displayed logical pixels. Attachment
/// resolution/MSAA do not affect this mapping. Use the larger axis so backing
/// rounding cannot omit a shadow whose displayed diameter reaches four pixels.
pub fn body_logical_scale(
    extent: [u32; 2],
    scale: f64,
    window: Option<([u32; 2], f64)>,
) -> Result<f64> {
    dimensions(extent, scale)?;
    let Some((physical, dpr)) = window else {
        return Ok(1.);
    };
    dimensions(physical, dpr)?;
    Ok((0..2)
        .map(|axis| scale * f64::from(physical[axis]) / dpr / f64::from(extent[axis]))
        .fold(0., f64::max))
}
fn image(asset: BodyAsset, x: f64, y: f64, w: f64, h: f64, alpha: f64) -> Draw {
    Draw {
        corners: None,
        asset,
        source: None,
        repeat_uv: None,
        black_effect: false,
        destination: BodyRect {
            x,
            y,
            width: w,
            height: h,
        },
        translation: [0., 0.],
        rotation: 0.,
        effective_alpha: alpha,
        blend: BodyBlend::SourceOver,
    }
}
fn prey(c: &PreyImageCommand) -> Draw {
    let (source, destination) = match c.draw {
        FoodImageDraw::Whole { x, y } => (
            None,
            BodyRect {
                x,
                y,
                width: f64::from(c.source.asset().width),
                height: f64::from(c.source.asset().height),
            },
        ),
        FoodImageDraw::Scaled {
            source,
            destination,
        } => (Some(source), destination),
    };
    Draw {
        corners: None,
        asset: c.source.asset(),
        source,
        repeat_uv: None,
        black_effect: false,
        destination,
        translation: [0., 0.],
        rotation: 0.,
        effective_alpha: c.alpha.effective,
        blend: c.blend,
    }
}
fn polygon(asset: BodyAsset, points: [Point; 4], alpha: f64) -> Draw {
    let mut d = image(asset, 0., 0., 1., 1., alpha);
    d.corners = Some(points.map(|p| [p.x, p.y]));
    d
}

fn placed(
    asset: BodyAsset,
    source: Option<BodyRect>,
    destination: BodyRect,
    translation: Point,
    rotation: f64,
    alpha: f64,
) -> Draw {
    let mut draw = image(
        asset,
        destination.x,
        destination.y,
        destination.width,
        destination.height,
        alpha,
    );
    draw.source = source;
    draw.translation = [translation.x, translation.y];
    draw.rotation = rotation;
    draw
}

fn submit_face(
    bank: &mut Bank,
    skin: &SkinDescriptor,
    snake: &PreparedSnake,
    view: EyeLayoutInput,
    accessory: Option<(StaticHeadAccessoryGeometry, BodyAsset)>,
    draws: &mut Vec<Draw>,
) -> Result<()> {
    submit_face_geometry(
        bank,
        skin,
        snake.face_geometry,
        snake.face_heading,
        snake.pupil,
        snake.metrics.spawn_progress,
        view,
        accessory,
        draws,
    )
}
#[allow(clippy::too_many_arguments)]
fn submit_face_geometry(
    bank: &mut Bank,
    skin: &SkinDescriptor,
    face_geometry: FaceGeometry,
    heading: f64,
    pupil: Point,
    spawn: f64,
    view: EyeLayoutInput,
    accessory: Option<(StaticHeadAccessoryGeometry, BodyAsset)>,
    draws: &mut Vec<Draw>,
) -> Result<()> {
    if let FaceGeometry::Paired(face) = face_geometry {
        let e = face
            .eyes
            .layout(heading, pupil, view)
            .map_err(|e| e.to_string())?;
        for pair in e.pairs {
            if e.outline_width > 0. {
                let outer = e.eye_radius + e.outline_width / 2.;
                if e.eye_alpha >= 1. {
                    // The opaque fill covers the disk's interior, saving draws.
                    push_draw(
                        draws,
                        image(
                            bank.disk([0; 3])?,
                            pair.eye.x - outer,
                            pair.eye.y - outer,
                            outer * 2.,
                            outer * 2.,
                            e.eye_alpha,
                        ),
                    )?;
                } else {
                    let inner = (e.eye_radius - e.outline_width / 2.).max(0.);
                    let steps = (std::f64::consts::TAU * outer).ceil().clamp(16., 48.) as usize;
                    let black = bank.solid([0; 3]);
                    // Canvas strokes leave the interior clear during death fades.
                    for i in 0..steps {
                        let at = |ordinal: usize, radius: f64| {
                            let angle = std::f64::consts::TAU * ordinal as f64 / steps as f64;
                            Point {
                                x: pair.eye.x + radius * angle.cos(),
                                y: pair.eye.y + radius * angle.sin(),
                            }
                        };
                        push_draw(
                            draws,
                            polygon(
                                black,
                                [
                                    at(i, outer),
                                    at(i + 1, outer),
                                    at(i + 1, inner),
                                    at(i, inner),
                                ],
                                e.eye_alpha,
                            ),
                        )?;
                    }
                }
            }
            for (color, point, radius, alpha) in [
                (e.eye_color, pair.eye, e.eye_radius, e.eye_alpha),
                (e.pupil_color, pair.pupil, e.pupil_radius, e.pupil_alpha),
            ] {
                if radius > 0. {
                    push_draw(
                        draws,
                        image(
                            bank.disk(color)?,
                            point.x - radius,
                            point.y - radius,
                            radius * 2.,
                            radius * 2.,
                            alpha,
                        ),
                    )?;
                }
            }
        }
    }
    if let Some((geometry, asset)) = accessory {
        let layout = geometry
            .layout(
                heading,
                Point {
                    x: f64::from(asset.width),
                    y: f64::from(asset.height),
                },
                spawn,
                view,
            )
            .map_err(|e| e.to_string())?;
        push_draw(
            draws,
            placed(
                asset,
                Some(BodyRect {
                    x: 0.,
                    y: 0.,
                    width: f64::from(asset.width),
                    height: f64::from(asset.height),
                }),
                layout.destination,
                layout.translation,
                layout.rotation,
                layout.alpha,
            ),
        )?;
    }
    let Some(cosmetics) = &bank.cosmetics else {
        return Ok(());
    };
    match (face_geometry, skin.face_sources) {
        (
            FaceGeometry::SingleImage(geometry),
            SkinFaceSources::SingleImage {
                base,
                pupil: source_pupil,
            },
        ) => {
            let layout = geometry
                .layout(heading, pupil, view)
                .map_err(|e| e.to_string())?;
            for (source, rect) in [base, source_pupil].into_iter().zip(layout.destinations) {
                push_draw(
                    draws,
                    placed(
                        cosmetics
                            .role(source.source)
                            .ok_or("Missing prepared face source")?,
                        source.source_rect,
                        rect,
                        Point { x: 0., y: 0. },
                        0.,
                        layout.alpha,
                    ),
                )?;
            }
        }
        (FaceGeometry::Mouth(geometry), SkinFaceSources::Mouth { eye_marks, mouth }) => {
            let layout = geometry
                .layout(heading, pupil, view)
                .map_err(|e| e.to_string())?;
            for (source, layout) in [eye_marks, eye_marks, mouth].into_iter().zip(layout.images) {
                push_draw(
                    draws,
                    placed(
                        cosmetics
                            .role(source)
                            .ok_or("Missing prepared face source")?,
                        None,
                        layout.destination,
                        layout.translation,
                        layout.rotation,
                        1.,
                    ),
                )?;
            }
        }
        (FaceGeometry::Paired(face), SkinFaceSources::PairedSide(source)) => {
            for layout in face
                .decoration
                .ok_or("Missing paired decoration")?
                .layout(heading, view)
                .map_err(|e| e.to_string())?
            {
                push_draw(
                    draws,
                    placed(
                        cosmetics
                            .role(source)
                            .ok_or("Missing prepared face source")?,
                        None,
                        layout.destination,
                        layout.translation,
                        layout.rotation,
                        1.,
                    ),
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn submit_antenna(
    bank: &mut Bank,
    skin: &SkinDescriptor,
    antenna: &PreparedAntenna,
    remaining: usize,
    draws: &mut Vec<Draw>,
) -> Result<()> {
    let mut paths = Vec::new();
    let mut count = usize::from(antenna.bulb.is_some());
    for stroke in &antenna.strokes {
        let path = match stroke.path {
            AntennaPath::Outer => &antenna.outer_path,
            AntennaPath::Inner => &antenna.inner_path,
        };
        let Some(path) = stroke_path(path, stroke.width) else {
            return Err(DRAW_MEMORY_LIMIT.into());
        };
        count += path.segments.len() + path.fans.iter().map(|fan| fan.4).sum::<usize>();
        paths.push(path);
    }
    // Admit the complete antenna, including every cap/join fan and its bulb.
    if count > remaining {
        return Err(DRAW_MEMORY_LIMIT.into());
    }
    // The complete polygon/bulb count also bounds StrokePath's infallible pushes.
    reserve_draws(draws, count)?;
    for (stroke, path) in antenna.strokes.iter().zip(paths) {
        let color = [
            (stroke.color >> 16) as u8,
            (stroke.color >> 8) as u8,
            stroke.color as u8,
        ];
        let solid = bank.solid(color);
        path.submit(solid, antenna.center, stroke.alpha, draws);
    }
    if let Some(bulb) = antenna.bulb {
        let asset = bank
            .cosmetics
            .as_ref()
            .ok_or("Antenna images unavailable")?
            .role(skin.antenna.ok_or("Missing antenna role")?.source)
            .ok_or("Missing prepared antenna source")?;
        push_draw(
            draws,
            placed(
                asset,
                None,
                bulb.destination,
                Point {
                    x: bulb.center.x + bulb.translation.x,
                    y: bulb.center.y + bulb.translation.y,
                },
                bulb.rotation.unwrap_or(0.),
                bulb.alpha,
            ),
        )?;
    }
    Ok(())
}

/// Reuse gameplay body/face/antenna ordering for a bounded synthetic pose.
pub fn prepare_studio_preview(
    bank: &mut Bank,
    pose: &slither_client::studio_preview::PreviewPose,
    skin: &SkinDescriptor,
    time: f64,
    extent: [u32; 2],
    dpr: f64,
    alpha: f64,
) -> Result<(Frame, Vec<Resource>)> {
    let mut families = bank.bodies.clone();
    for family in &mut families {
        let len = family.variants.len();
        if !skin.custom {
            family.variants.rotate_left((time / 120.) as usize % len);
        }
    }
    let palettes: Vec<_> = families
        .iter()
        .map(|p| BodyPalette {
            variants: &p.variants,
            boost_glow: Some(p.glow),
        })
        .collect();
    let body = plan_standard_body_with_antenna(
        BodyPaintInput {
            geometry: BodyPaintGeometry {
                samples: &pose.samples,
                active_count: pose.samples.len(),
                distance_cursor_seed: pose.head,
                eligible: true,
            },
            recipe: BodyPaintRecipe {
                base_palette: skin.base_palette,
                palettes: &palettes,
                runs: &skin.runs,
                variant_mode: skin.variant_mode,
                variant_period: skin.variant_period,
                custom: skin.custom,
                style: Some(skin.body_style),
                outline: Some(bank.outline),
                shadow: Some(bank.shadow),
                death: Some(bank.death),
            },
            assets: &bank.assets,
            view: pose.view,
            quality: BodyPaintQuality {
                high_quality: true,
                blend: 1.,
            },
            visual: BodyPaintVisual {
                scale: 1.,
                spawn: 1.,
                death: 0.,
                dead: false,
                smoothed_speed: 0.,
                boost_threshold: 1.,
                cruise_speed: 0.,
                maximum_speed: 2.,
                boost_phase: 0.,
            },
            entry_alpha: 1.,
            command_limit: 1024,
        },
        pose.antenna
            .as_ref()
            .map_or(AntennaBarrierEffect::Disabled, |a| a.barrier_effect),
    )
    .map_err(|e| e.to_string())?;
    let mut draws: Vec<_> = body.commands[..body.antenna_barrier]
        .iter()
        .map(Draw::from)
        .collect();
    if let Some(antenna) = &pose.antenna {
        submit_antenna(bank, skin, antenna, 1024 - draws.len(), &mut draws)?;
    }
    draws.extend(body.commands[body.antenna_barrier..].iter().map(Draw::from));
    submit_face_geometry(
        bank,
        skin,
        skin.face,
        pose.heading,
        pose.pupil,
        1.,
        EyeLayoutInput {
            corrected_head: pose.head,
            camera: pose.view.camera,
            viewport_half: Point {
                x: pose.view.half_width,
                y: pose.view.half_height,
            },
            scale: 1.,
            zoom: pose.view.zoom,
            death_progress: 0.,
        },
        bank.cosmetics
            .as_ref()
            .and_then(|c| c.preview_accessory(skin)),
        &mut draws,
    )?;
    if draws.len() > 1024 {
        return Err(DRAW_MEMORY_LIMIT.into());
    }
    for draw in &mut draws {
        draw.effective_alpha *= alpha;
    }
    let mut frame = Frame {
        identity: 0,
        tick: 0,
        extent,
        viewport_scale: dpr,
        draws,
        snakes: vec![],
    };
    let mut resources = bank.resources(&frame)?;
    // Independent lazy disk allocation must never alias gameplay's body handles.
    let remap = |a: &mut BodyAsset| {
        if a.handle / crate::bank::HANDLE_NAMESPACE_SIZE == 1 {
            a.handle = (a.handle % crate::bank::HANDLE_NAMESPACE_SIZE) | 0x9000_0000_0000_0000;
        }
    };
    for draw in &mut frame.draws {
        remap(&mut draw.asset);
    }
    for resource in &mut resources {
        remap(&mut resource.asset);
    }
    frame.validate(
        &resources
            .iter()
            .map(|r| (r.asset.into(), r.asset))
            .collect(),
    )?;
    Ok((frame, resources))
}

struct StrokePath {
    segments: Vec<[Point; 4]>,
    fans: Vec<(Point, Point, f64, f64, usize)>,
    radius: f64,
}
impl StrokePath {
    fn submit(self, asset: BodyAsset, center: Point, alpha: f64, draws: &mut Vec<Draw>) {
        let add = |a: Point, b: Point| Point {
            x: a.x + b.x,
            y: a.y + b.y,
        };
        for points in self.segments {
            draws.push(polygon(asset, points.map(|p| add(p, center)), alpha));
        }
        for (origin, point, start, sweep, steps) in self.fans {
            for i in 0..steps {
                let arc = |t: f64| {
                    add(
                        point,
                        Point {
                            x: (start + t * sweep).cos() * self.radius,
                            y: (start + t * sweep).sin() * self.radius,
                        },
                    )
                };
                draws.push(polygon(
                    asset,
                    [
                        origin,
                        arc(i as f64 / steps as f64),
                        arc((i + 1) as f64 / steps as f64),
                        origin,
                    ]
                    .map(|p| add(p, center)),
                    alpha,
                ));
            }
        }
    }
}

/// Plan round caps/joins and their exact draw counts before emitting polygons.
fn stroke_path(path: &[Point], width: f64) -> Option<StrokePath> {
    use std::f64::consts::PI;
    let mut points: Vec<Point> = Vec::new();
    for p in path {
        if points
            .last()
            .is_none_or(|q| (p.x - q.x).hypot(p.y - q.y) > 1e-9)
        {
            points.push(*p);
        }
    }
    if points.len() < 2 {
        return Some(StrokePath {
            segments: Vec::new(),
            fans: Vec::new(),
            radius: width / 2.,
        });
    }
    let radius = width / 2.;
    let normals: Vec<_> = points
        .windows(2)
        .map(|p| {
            let dx = p[1].x - p[0].x;
            let dy = p[1].y - p[0].y;
            let length = dx.hypot(dy);
            Point {
                x: -dy / length * radius,
                y: dx / length * radius,
            }
        })
        .collect();
    let add = |a: Point, b: Point| Point {
        x: a.x + b.x,
        y: a.y + b.y,
    };
    let neg = |a: Point| Point { x: -a.x, y: -a.y };
    let mut starts: Vec<_> = points
        .windows(2)
        .zip(&normals)
        .map(|(p, n)| [add(p[0], *n), add(p[0], neg(*n))])
        .collect();
    let mut ends: Vec<_> = points
        .windows(2)
        .zip(&normals)
        .map(|(p, n)| [add(p[1], *n), add(p[1], neg(*n))])
        .collect();
    let mut fans = Vec::new();
    for i in 1..points.len() - 1 {
        let a = normals[i - 1];
        let b = normals[i];
        let turn = (a.x * b.y - a.y * b.x).atan2(a.x * b.x + a.y * b.y);
        if turn.abs() < 1e-9 {
            continue;
        }
        let denominator = 1. + (a.x * b.x + a.y * b.y) / (radius * radius);
        // Near reversal has no stable inside intersection. Bound the join locally.
        let factor = 1. / denominator.max(0.25);
        let m = Point {
            x: (a.x + b.x) * factor,
            y: (a.y + b.y) * factor,
        };
        let inner = usize::from(turn < 0.);
        let intersection = add(points[i], if inner == 0 { m } else { neg(m) });
        ends[i - 1][inner] = intersection;
        starts[i][inner] = intersection;
        let outer = if inner == 0 { neg(a) } else { a };
        fans.push((intersection, points[i], outer.y.atan2(outer.x), turn));
    }
    let segments = starts
        .into_iter()
        .zip(ends)
        .map(|(start, end)| [start[0], end[0], end[1], start[1]])
        .collect();
    for (point, normal, turn) in [
        (points[0], normals[0], PI),
        (
            *points.last().expect("path"),
            neg(*normals.last().expect("path")),
            PI,
        ),
    ] {
        fans.push((point, point, normal.y.atan2(normal.x), turn));
    }
    let fans = fans
        .into_iter()
        .map(|(origin, point, start, sweep)| {
            let steps = (sweep.abs() * radius.max(1.) * 2.).ceil().max(1.);
            (steps.is_finite() && steps <= MAX_DRAWS as f64).then_some((
                origin,
                point,
                start,
                sweep,
                steps as usize,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(StrokePath {
        segments,
        fans,
        radius,
    })
}

#[cfg(test)]
mod antenna_budget_tests {
    use super::*;

    #[test]
    fn antenna_admission_counts_segments_fans_and_bulb_before_emission() {
        let mut bank = Bank::new().unwrap();
        bank.cosmetics =
            Some(crate::cosmetics::Cosmetics::from_images(Default::default(), None).unwrap());
        let skin = resolve_skin(24, &[], false).unwrap();
        let path = vec![
            Point { x: 0., y: 0. },
            Point { x: 10., y: 0. },
            Point { x: 10., y: 10. },
        ];
        let antenna = PreparedAntenna {
            center: Point { x: 100., y: 100. },
            outer_path: path.clone(),
            inner_path: path,
            strokes: vec![
                AntennaStroke {
                    path: AntennaPath::Outer,
                    color: 0x123456,
                    width: 5.,
                    alpha: 0.5,
                },
                AntennaStroke {
                    path: AntennaPath::Inner,
                    color: 0xabcdef,
                    width: 2.,
                    alpha: 0.75,
                },
            ],
            bulb: Some(AntennaBulbCommand {
                center: Point { x: 100., y: 100. },
                translation: Point::default(),
                rotation: None,
                destination: BodyRect {
                    x: -10.,
                    y: -10.,
                    width: 20.,
                    height: 20.,
                },
                alpha: 1.,
            }),
            barrier_effect: AntennaBarrierEffect::Disabled,
        };
        let mut full = Vec::new();
        submit_antenna(&mut bank, &skin, &antenna, MAX_DRAWS, &mut full).unwrap();
        assert!(full.len() > 5); // Four segments, plus cap/join fans and the bulb.
        let mut draws = vec![image(bank.solid([255; 3]), 0., 0., 1., 1., 1.)];
        let before = draws.clone();
        assert_eq!(
            submit_antenna(&mut bank, &skin, &antenna, full.len() - 1, &mut draws),
            Err(DRAW_MEMORY_LIMIT.into())
        );
        assert_eq!(draws, before);
        submit_antenna(&mut bank, &skin, &antenna, full.len(), &mut draws).unwrap();
        assert_eq!(&draws[before.len()..], full);
    }
}

/// Numeric camera/body/face preparation is called exactly once per actual draw, including Setup/resize.
pub fn prepare(
    client: &mut AnimatedClient,
    bank: &mut Bank,
    extent: [u32; 2],
    scale: f64,
    identity: u64,
    applied: QualitySnapshot,
) -> Result<PreparedFrame> {
    prepare_composed(client, bank, extent, scale, identity, applied, None)
}
/// Production LocalGame preparation captures the announcement at this exact draw
/// boundary, including ordered Setup callbacks, ordinary work and resize.
pub fn prepare_gameplay(
    client: &mut AnimatedClient,
    bank: &mut Bank,
    extent: [u32; 2],
    scale: f64,
    identity: u64,
    applied: QualitySnapshot,
) -> Result<PreparedFrame> {
    crate::latency::build("build_begin", identity);
    preparation_phase!("build_begin", identity);
    let announcement = client.announcement_view();
    prepare_composed(
        client,
        bank,
        extent,
        scale,
        identity,
        applied,
        Some(&announcement),
    )
}
#[allow(clippy::too_many_arguments)]
fn prepare_composed(
    client: &mut AnimatedClient,
    bank: &mut Bank,
    extent: [u32; 2],
    scale: f64,
    identity: u64,
    applied: QualitySnapshot,
    announcement: Option<&AnnouncementView>,
) -> Result<PreparedFrame> {
    let logical = dimensions(extent, scale)?;
    let full_stock = bank.cosmetics.is_some();
    // Mix the draw identity before the local stream so successive draws do not
    // give the first antenna node nearly identical perturbations. Retried frames
    // reuse their already prepared plan and consume no additional randomness.
    let mut entropy = identity.wrapping_add(0x9e37_79b9_7f4a_7c15);
    entropy = (entropy ^ (entropy >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    entropy = (entropy ^ (entropy >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    entropy ^= entropy >> 31;
    let prepared = client
        .prepare_draw_with_accessory_styles::<PreparationError, _, _>(
            logical[0],
            logical[1],
            applied.quality_scale,
            |_, _, snake| {
                Ok(full_stock
                    && resolve_skin(snake.metrics().skin, snake.custom_skin(), false)
                        .expect("admitted custom bytes")
                        .body_style
                        .detailed_segments)
            },
            |request| {
                Ok((0..request.random_pair_count)
                    .map(|_| {
                        std::array::from_fn(|_| {
                            entropy = entropy.wrapping_mul(1664525).wrapping_add(1013904223)
                                & 0xffff_ffff;
                            entropy as f64 / 4294967296.
                        })
                    })
                    .collect())
            },
        )
        .map_err(|e| e.0)?;
    preparation_phase!("curves_end", identity);
    #[cfg(all(feature = "latency-rig", not(target_arch = "wasm32")))]
    let rig_sample = crate::latency_rig::snapshot(identity, &prepared, client);
    let view = BodyPaintView {
        camera: client.camera().position(),
        zoom: client.camera().zoom(),
        half_width: f64::from(extent[0]) / scale / 2.,
        half_height: f64::from(extent[1]) / scale / 2.,
    };
    let body_pixels_per_unit = body_logical_scale(extent, scale, bank.overlay_viewport)?;
    let quality = BodyPaintQuality {
        high_quality: applied.high_quality,
        blend: applied.quality_blend,
    };
    let tile = bank.background_art(crate::cosmetics::BackgroundArt::HexTile);
    let backdrop = bank.background_art(crate::cosmetics::BackgroundArt::Backdrop);
    let background = client
        .prepare_background(BackgroundInput {
            width: logical[0],
            height: logical[1],
            zoom: view.zoom,
            image_size: Some([backdrop.width, backdrop.height]),
            pattern_available: true,
            quality: applied,
            tile_width: f64::from(tile.width),
            tile_height: f64::from(tile.height),
        })
        .map_err(|e| e.to_string())?;
    let [x, y, w, h] = background.clear.rect;
    let mut draws = bank.take_draws(false);
    push_draw(
        &mut draws,
        image(
            bank.solid(background.clear.color),
            x,
            y,
            w,
            h,
            background.clear.alpha,
        ),
    )?;
    if let Some(layer) = background.image {
        let [x, y, w, h] = layer.destination;
        push_draw(&mut draws, image(backdrop, x, y, w, h, layer.alpha))?;
    }
    if let Some(layer) = background.pattern {
        // Clip the oversized Canvas fill to the viewport; keep its pattern-space
        // coordinates so translation, zoom and fractional phase remain identical.
        let mut draw = image(
            tile,
            0.,
            0.,
            f64::from(logical[0]),
            f64::from(logical[1]),
            layer.alpha,
        );
        draw.repeat_uv = Some(BodyRect {
            x: (-layer.center.x / layer.zoom - layer.translation.x) / f64::from(tile.width),
            y: (-layer.center.y / layer.zoom - layer.translation.y) / f64::from(tile.height),
            width: f64::from(logical[0]) / layer.zoom / f64::from(tile.width),
            height: f64::from(logical[1]) / layer.zoom / f64::from(tile.height),
        });
        push_draw(&mut draws, draw)?;
    }
    let [early, late] = std::mem::take(&mut bank.food_commands);
    preparation_phase!("background_end", identity);
    let user_zoom = client.camera().user_zoom_factor();
    let food = plan_food_with_zoom_and_index(
        FoodPaintInput {
            foods: client.foods().foods(),
            view,
            quality,
            bounds: client.food_bounds(),
            assets: &bank.assets,
            target_handle: None,
            entry_alpha: 1.,
            command_limit: MAX_FOOD_PAINT_COMMANDS.min(MAX_DRAWS - draws.len()),
        },
        &mut bank.food_asset_index,
        early,
        late,
        user_zoom,
    )
    .map_err(|e| {
        if e == FoodPaintError::CommandLimit {
            DRAW_MEMORY_LIMIT.into()
        } else {
            e.to_string()
        }
    })?;
    preparation_phase!("food_end", identity);
    let eaters: Vec<_> = client
        .snake_objects()
        .map(|(token, _, _, snake)| {
            let m = snake.metrics();
            let o = snake.head_corrections().offset();
            (
                token,
                PreyEaterPose {
                    head: m.raw_head,
                    head_offset: Point { x: o[0], y: o[1] },
                    heading: m.heading,
                    angle_offset: snake.angle_corrections().offset(),
                },
            )
        })
        .collect();
    let prey_plan = plan_prey_with_zoom(
        PreyPaintInput {
            preys: client.preys().preys(),
            eaters: &eaters,
            view,
            assets: &bank.assets,
            target_handle: None,
            early_entry_alpha: 1.,
            late_entry_alpha: 1.,
            command_limit: MAX_PREY_PAINT_COMMANDS
                .min(MAX_DRAWS - draws.len() - food.early.len() - food.late.len()),
        },
        user_zoom,
    )
    .map_err(|e| {
        if e == PreyPaintError::CommandLimit {
            DRAW_MEMORY_LIMIT.into()
        } else {
            e.to_string()
        }
    })?;
    let families = bank.bodies.clone();
    preparation_phase!("prey_end", identity);
    let palettes: Vec<_> = families
        .iter()
        .map(|p| BodyPalette {
            variants: &p.variants,
            boost_glow: Some(p.glow),
        })
        .collect();
    reserve_draws(&mut draws, food.early.len() + prey_plan.early.len())?;
    draws.extend(food.early.iter().map(Draw::from));
    draws.extend(prey_plan.early.iter().map(prey));
    let nickname_index = draws.len();
    let mut snakes = Vec::new();
    let mut antennas = Vec::new();
    for prepared_snake in prepared {
        let snake = prepared_snake.snake;
        let m = snake.metrics;
        let mut observation = SnakeDrawObservation {
            id: m.id,
            own: snake.own,
            body_commands: 0,
            viewport_body_commands: 0,
        };
        let raw = client
            .snake_object(snake.token)
            .ok_or("prepared appearance missing")?;
        let skin = if full_stock {
            bank.cosmetics
                .as_ref()
                .expect("full stock")
                .skin(m.skin, raw.custom_skin())
        } else {
            resolve_authored_local_skin(m.skin, raw.custom_skin())
        }
        .map_err(|e| e.to_string())?;
        let mut antenna_index = draws.len();
        if let Some(geometry) = &snake.body_geometry {
            // Below two logical pixels across, separate outline/shadow/inner
            // shimmer details cannot resolve. Reuse the existing low-quality
            // passes without removing body geometry or changing global quality.
            // Authored wheel LOD; measured workload and limits: docs/zoom.md.
            let body_quality = if user_zoom < 1. && 29. * m.scale * view.zoom < 2. {
                BodyPaintQuality {
                    high_quality: false,
                    blend: 0.,
                }
            } else {
                quality
            };
            let body_storage = std::mem::take(&mut bank.body_commands);
            let body = plan_standard_body_with_zoom_and_storage(
                BodyPaintInput {
                    geometry: geometry.as_paint_geometry(),
                    recipe: BodyPaintRecipe {
                        base_palette: skin.base_palette,
                        palettes: &palettes,
                        runs: &skin.runs,
                        variant_mode: skin.variant_mode,
                        variant_period: skin.variant_period,
                        custom: skin.custom,
                        style: Some(skin.body_style),
                        outline: Some(bank.outline),
                        shadow: Some(bank.shadow),
                        death: Some(bank.death),
                    },
                    assets: &bank.assets,
                    view,
                    quality: body_quality,
                    visual: BodyPaintVisual {
                        scale: m.scale,
                        spawn: m.spawn_progress,
                        death: m.death_progress,
                        dead: !m.alive,
                        smoothed_speed: m.visual_speed,
                        boost_threshold: m.boost_threshold_speed,
                        cruise_speed: m.cruise_speed,
                        maximum_speed: m.maximum_speed,
                        boost_phase: m.boost_phase,
                    },
                    entry_alpha: 1.,
                    command_limit: MAX_BODY_PAINT_COMMANDS.min(
                        MAX_DRAWS
                            .saturating_sub(draws.len() + food.late.len() + prey_plan.late.len()),
                    ),
                },
                snake
                    .antenna
                    .as_ref()
                    .map_or(AntennaBarrierEffect::Disabled, |a| a.barrier_effect),
                body_storage,
                if cfg!(feature = "frame-profile-no-shadow-cut") {
                    1.
                } else {
                    user_zoom
                },
                body_pixels_per_unit,
            )
            .map_err(|e| {
                if e == BodyPaintError::CommandLimit {
                    DRAW_MEMORY_LIMIT.into()
                } else {
                    e.to_string()
                }
            })?;
            for command in &body.commands {
                if command.pass == BodyPaintPass::Body {
                    observation.body_commands += 1;
                    if Draw::from(command).image_intersects_viewport(extent, scale) {
                        observation.viewport_body_commands += 1;
                    }
                }
            }
            reserve_draws(&mut draws, body.commands.len())?;
            draws.extend(body.commands[..body.antenna_barrier].iter().map(Draw::from));
            antenna_index = draws.len();
            draws.extend(body.commands[body.antenna_barrier..].iter().map(Draw::from));
            bank.body_commands = body.commands;
        }
        snakes.push(observation);
        let input = EyeLayoutInput {
            corrected_head: m.corrected_head,
            camera: view.camera,
            viewport_half: Point {
                x: view.half_width,
                y: view.half_height,
            },
            scale: m.scale,
            zoom: view.zoom,
            death_progress: m.death_progress,
        };
        let accessory = prepared_snake.accessory.and_then(|geometry| {
            bank.cosmetics
                .as_ref()
                .and_then(|c| c.accessory(snake.own))
                .map(|r| (geometry, r.asset))
        });
        submit_face(bank, &skin, &snake, input, accessory, &mut draws)?;
        check_draw_count(draws.len() + food.late.len() + prey_plan.late.len())?;
        if let Some(antenna) = snake.antenna {
            antennas.push((antenna_index, skin, antenna));
        }
    }
    reserve_draws(&mut draws, food.late.len() + prey_plan.late.len())?;
    draws.extend(food.late.iter().map(Draw::from));
    draws.extend(prey_plan.late.iter().map(prey));
    bank.food_commands = [food.early, food.late];
    preparation_phase!("bodies_end", identity);
    if let Some(boundary) = client.boundary()
        && let Some(edge) = plan_arena_edge_with_zoom(
            ArenaEdgeInput {
                view: client.arena_edge_view(),
                camera: view.camera,
                zoom: view.zoom,
                width: logical[0],
                height: logical[1],
                radius: boundary.displayed(),
            },
            user_zoom != 1.,
        )
        .map_err(|e| e.to_string())?
    {
        let fill = bank.solid([48, 0, 0]);
        let edge_fill = edge.fill();
        let segments = edge.stroke().len() - 2;
        for i in 0..segments {
            push_draw(
                &mut draws,
                polygon(
                    fill,
                    [
                        edge_fill[i + 1],
                        edge_fill[i + 2],
                        edge_fill[2 * segments + 1 - i],
                        edge_fill[2 * segments + 2 - i],
                    ],
                    0.8,
                ),
            )?;
        }
        let stroke = bank.solid([128, 0, 0]);
        let points = &edge.stroke()[1..];
        for i in 0..points.len() - 1 {
            let a = points[i];
            let b = points[i + 1];
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let length = dx.hypot(dy);
            if length == 0. {
                continue;
            }
            let n = Point {
                x: -dy / length * edge.line_width() / 2.,
                y: dx / length * edge.line_width() / 2.,
            };
            push_draw(
                &mut draws,
                polygon(
                    stroke,
                    [
                        Point {
                            x: a.x + n.x,
                            y: a.y + n.y,
                        },
                        Point {
                            x: b.x + n.x,
                            y: b.y + n.y,
                        },
                        Point {
                            x: b.x - n.x,
                            y: b.y - n.y,
                        },
                        Point {
                            x: a.x - n.x,
                            y: a.y - n.y,
                        },
                    ],
                    0.8,
                ),
            )?;
        }
    }
    let mut text_draws = Vec::new();
    preparation_phase!("arena_end", identity);
    let mut text_resources = std::collections::BTreeMap::new();
    let name_zoom = user_zoom;
    for name in client
        .nickname_draws()
        .iter()
        .take(crate::text::MAX_NICKNAME_RUNS_PER_FRAME)
    {
        // The text rasterizer requires at least one pixel. Subpixel world
        // names disappear with subpixel food instead of rejecting the picture.
        if name.font_size * name_zoom < 1. {
            continue;
        }
        use crate::text::{Align, Baseline, TextRun, TextStyle, Weight};
        for (draw, resource) in bank.text.draw(
            TextRun {
                text: &name.text,
                style: TextStyle {
                    family: TextFamily::Arial,
                    slant: TextSlant::Normal,
                    // Preserve stock sizing at default; names belong to the
                    // world when the player changes zoom (docs/zoom.md).
                    size: name.font_size * name_zoom / scale,
                    weight: Weight::Normal,
                    color: name.color,
                },
                // The core mixes a fixed backing-pixel gap with world geometry;
                // only the latter should grow when the logical frame is scaled.
                position: [
                    name.position.x,
                    name.position.y - 32. + 32. * name_zoom / scale,
                ],
                align: Align::Center,
                baseline: Baseline::Middle,
                alpha: name.alpha,
            },
            scale,
        )? {
            check_draw_count(draws.len() + text_draws.len() + 1)?;
            text_resources.insert(resource.asset.handle, resource);
            text_draws.push(draw);
        }
    }
    let nickname_count = text_draws.len();
    reserve_draws(&mut draws, nickname_count)?;
    draws.splice(nickname_index..nickname_index, text_draws);
    // Admit antennas against the complete non-antenna scene, then restore each
    // original barrier position (including its relation to death passes).
    check_draw_count(draws.len())?;
    let mut remaining = MAX_DRAWS - draws.len();
    let mut inserted = 0;
    for (index, skin, antenna) in antennas {
        let mut commands = Vec::new();
        submit_antenna(bank, &skin, &antenna, remaining, &mut commands)?;
        remaining -= commands.len();
        let count = commands.len();
        reserve_draws(&mut draws, count)?;
        draws.splice(
            index + nickname_count + inserted..index + nickname_count + inserted,
            commands,
        );
        inserted += count;
    }
    bank.stage_text(identity, text_resources.into_values().collect())?;
    preparation_phase!("names_end", identity);
    let mut frame = Frame {
        identity,
        extent,
        viewport_scale: scale,
        tick: 0,
        draws,
        snakes,
    };
    let mut resources = bank.resources(&frame)?;
    // Reference HUD/minimap are DOM siblings of the capped game canvas.
    // Nicknames have already been submitted into the world frame above.
    let mut overlay = bank.overlay_viewport.map(|(extent, viewport_scale)| Frame {
        identity: frame.identity,
        tick: frame.tick,
        extent,
        viewport_scale,
        draws: bank.take_draws(true),
        snakes: Vec::new(),
    });
    let ui = overlay.as_mut().unwrap_or(&mut frame);
    preparation_phase!("resources_end", identity);
    if let Some(view) = announcement {
        bank.announcement
            .append(view, &mut bank.text, ui, &mut resources)?;
    }
    bank.minimap
        .append(client, &mut bank.text, ui, &mut resources)?;
    preparation_phase!("minimap_end", identity);
    // Mandatory overlays must fit in full; never silently truncate a panel.
    check_draw_count(frame.draws.len() + overlay.as_ref().map_or(0, |f| f.draws.len()))?;
    if resources
        .iter()
        .map(|r| r.image.pixels.len())
        .sum::<usize>()
        > crate::bank::PIXEL_BUDGET
    {
        return Err("Whole-frame draw/pixel budget".into());
    }
    #[cfg(all(feature = "latency-rig", not(target_arch = "wasm32")))]
    crate::latency_rig::commit(rig_sample);
    Ok(PreparedFrame {
        frame,
        overlay,
        resources,
    })
}
// Clip on the same physical grid as the raster. Fractional source crops would
// reintroduce filtering even after the tile origin has been snapped.
pub(crate) fn clip_overlay_text(draw: &mut Draw, clip: BodyRect, scale: f64) -> bool {
    let right = ((clip.x + clip.width) * scale).floor() / scale;
    let bottom = ((clip.y + clip.height) * scale).floor() / scale;
    let x = (clip.x * scale).ceil() / scale;
    let y = (clip.y * scale).ceil() / scale;
    clip_text(
        draw,
        BodyRect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        },
    )
}
// Text tiles have no rotation. Intersect both the logical destination and its
// source crop, so long names cannot paint into score/rank columns or offscreen.
pub(crate) fn clip_text(draw: &mut Draw, clip: BodyRect) -> bool {
    let r = draw.destination;
    let x = r.x + draw.translation[0];
    let y = r.y + draw.translation[1];
    let left = x.max(clip.x);
    let top = y.max(clip.y);
    let right = (x + r.width).min(clip.x + clip.width);
    let bottom = (y + r.height).min(clip.y + clip.height);
    if right <= left || bottom <= top || draw.effective_alpha <= 0. {
        return false;
    }
    let mut source = draw.source.expect("text tile crop");
    let sx = source.width / r.width;
    let sy = source.height / r.height;
    let source_right = (source.x + source.width).min(f64::from(draw.asset.width));
    let source_bottom = (source.y + source.height).min(f64::from(draw.asset.height));
    source.x = (source.x + (left - x) * sx).clamp(source.x, source_right);
    source.y = (source.y + (top - y) * sy).clamp(source.y, source_bottom);
    // Fractional DPR can reconstruct an edge a few ulps beyond its tile. Keep
    // the mapped crop inside its original source before strict frame validation.
    source.width = ((right - left) * sx).min(source_right - source.x);
    source.height = ((bottom - top) * sy).min(source_bottom - source.y);
    if source.width <= 0. || source.height <= 0. {
        return false;
    }
    draw.source = Some(source);
    draw.destination = BodyRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    };
    draw.translation = [0., 0.];
    true
}
fn finish_plan(
    mut prepared: PreparedFrame,
    bank: &mut Bank,
    plan: Result<crate::hud::Plan>,
) -> Result<PreparedFrame> {
    let identity = prepared.frame.identity;
    let result = (|| {
        let plan = plan?;
        // Late overlays retain the captured scene's immutable resources. Text
        // slots may change afterward without changing queued Setup pictures.
        let total_draws = prepared.draw_count();
        let physical_overlay = prepared.overlay.is_some();
        let frame = prepared.ui_frame_mut();
        let other_draws = total_draws - frame.draws.len();
        let scale = frame.viewport_scale;
        let raster_scale = if physical_overlay {
            scale
        } else {
            let max_font_size = plan
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .map(|span| span.style.size)
                .fold(0_f64, f64::max);
            scale
                .min(crate::text::MAX_HUD_RASTER_SCALE)
                .min(128. / max_font_size)
        };
        bank.text
            .begin_hud_frame(plan.lines.iter().map(|line| line.spans.len()).sum());
        let viewport = BodyRect {
            x: 0.,
            y: 0.,
            width: f64::from(frame.extent[0]) / scale,
            height: f64::from(frame.extent[1]) / scale,
        };
        let draw_limit = frame.draws.len() + crate::hud::MAX_DRAWS;
        let mut slot = 0;
        let mut resources = std::collections::BTreeMap::new();
        for line in &plan.lines {
            let mut position = line.position;
            for span in &line.spans {
                if frame.draws.len() >= draw_limit {
                    break;
                }
                let (advance, tiles) = bank.text.draw_hud(
                    slot,
                    crate::text::TextRun {
                        text: &span.text,
                        style: span.style,
                        position,
                        align: line.align,
                        baseline: crate::text::Baseline::Alphabetic,
                        alpha: span.alpha,
                    },
                    raster_scale,
                    matches!(line.align, crate::text::Align::Left)
                        .then_some((line.clip.x + line.clip.width - position[0]).max(0.)),
                )?;
                slot += 1;
                position[0] += advance;
                for (mut draw, resource) in tiles {
                    if frame.draws.len() >= draw_limit {
                        break;
                    }
                    if clip_overlay_text(&mut draw, line.clip, scale)
                        && clip_overlay_text(&mut draw, viewport, scale)
                    {
                        check_draw_count(other_draws + frame.draws.len() + 1)?;
                        push_draw(&mut frame.draws, draw)?;
                        resources.insert(resource.asset.handle, resource);
                    }
                }
            }
        }
        prepared.resources.extend(resources.into_values());
        if prepared.pixel_bytes() > crate::bank::PIXEL_BUDGET {
            return Err("Active resource pixel budget exceeded".into());
        }
        let resident = prepared
            .resources
            .iter()
            .map(|r| (r.asset.into(), r.asset))
            .collect();
        prepared.frame.validate(&resident)?;
        if let Some(overlay) = &prepared.overlay {
            overlay.validate(&resident)?;
        }
        Ok(prepared)
    })();
    bank.discard_text(identity);
    result
}
/// Retained string-only entry for existing callers; no domain parsing of text.
pub fn finish(frame: Frame, bank: &mut Bank, hud: &str) -> Result<PreparedFrame> {
    let identity = frame.identity;
    let result = (|| {
        let plan = crate::hud::plan_text(hud, frame.extent, frame.viewport_scale)?;
        let resources = bank.resources(&frame)?;
        finish_plan(
            PreparedFrame {
                frame,
                overlay: None,
                resources,
            },
            bank,
            Ok(plan),
        )
    })();
    bank.discard_text(identity);
    result
}
/// Ordinary native/web gameplay shares this complete Rust projection and composition.
pub fn finish_hud(
    prepared: PreparedFrame,
    bank: &mut Bank,
    hud: &crate::hud::View,
) -> Result<PreparedFrame> {
    let ui = prepared.ui_frame();
    let plan = crate::hud::plan(hud, ui.extent, ui.viewport_scale);
    let identity = prepared.frame.identity;
    let result = finish_plan(prepared, bank, plan.map_err(str::to_owned));
    crate::latency::build(
        if result.is_ok() {
            "build_end"
        } else {
            "build_failed"
        },
        identity,
    );
    result
}

#[cfg(test)]
mod text_staging_tests {
    //! Original synthetic pixels and failures; no fonts, GPU or server.
    use super::*;
    use crate::{materials::RgbaImage, scene::Resource};
    #[test]
    fn fractional_crop_rounding_cannot_escape_a_synthetic_two_pixel_tile() {
        let asset = BodyAsset {
            handle: crate::bank::TEXT_HANDLE_BASE + 1,
            revision: 1,
            width: 2,
            height: 2,
            status: BodyAssetStatus::Ready,
        };
        for scale in [1.25, 1.75, 2.75, 3.5] {
            let mut draw = image(asset, -1. / scale, -1. / scale, 2. / scale, 2. / scale, 1.);
            draw.translation = [8., 8.];
            draw.source = Some(BodyRect {
                x: 0.,
                y: 0.,
                width: 2.,
                height: 2.,
            });
            assert!(clip_text(
                &mut draw,
                BodyRect {
                    x: 8.,
                    y: 8.,
                    width: 50.,
                    height: 50.
                }
            ));
            let frame = Frame {
                identity: 1,
                tick: 0,
                extent: [100, 100],
                viewport_scale: scale,
                draws: vec![draw],
                snakes: Vec::new(),
            };
            assert!(
                frame
                    .quads(&std::collections::BTreeMap::from([(asset.into(), asset)]))
                    .is_ok()
            );
        }
    }
    fn stage(bank: &mut Bank, identity: u64) -> Frame {
        let asset = BodyAsset {
            handle: crate::bank::TEXT_HANDLE_BASE + identity,
            revision: 1,
            width: 1,
            height: 1,
            status: BodyAssetStatus::Ready,
        };
        bank.stage_text(
            identity,
            vec![Resource {
                asset,
                image: RgbaImage {
                    width: 1,
                    height: 1,
                    pixels: vec![255; 4],
                }
                .into(),
            }],
        )
        .unwrap();
        Frame {
            snakes: Vec::new(),
            identity,
            extent: [960, 540],
            viewport_scale: 1.,
            tick: 0,
            draws: vec![image(asset, 10., 10., 1., 1., 1.)],
        }
    }
    #[test]
    fn every_failed_finish_discards_only_its_own_staged_text() {
        let mut bank = Bank::new().unwrap();
        let retained = stage(&mut bank, 0);
        let view = crate::game::LocalGame::new(0.).unwrap().hud_view();
        // Repeated increasing identities used to exhaust the 16-picture budget.
        for identity in 1..=25 {
            let mut frame = stage(&mut bank, identity);
            let result = match identity % 5 {
                0 => {
                    frame.viewport_scale = f64::NAN;
                    finish(frame, &mut bank, "HUD")
                }
                1 => {
                    let plan =
                        crate::hud::plan_text("HUD", frame.extent, frame.viewport_scale).unwrap();
                    frame.draws[0].destination.width = f64::NAN;
                    finish_plan(
                        PreparedFrame {
                            overlay: None,
                            frame,
                            resources: Vec::new(),
                        },
                        &mut bank,
                        Ok(plan),
                    )
                }
                2 => {
                    frame.draws[0].effective_alpha = f64::NAN;
                    finish(frame, &mut bank, "HUD")
                }
                3 => {
                    frame.draws[0].asset.handle += 1000;
                    bank.resources(&frame).map(|resources| PreparedFrame {
                        frame,
                        overlay: None,
                        resources,
                    })
                }
                _ => {
                    frame.viewport_scale = f64::NAN;
                    finish_hud(
                        PreparedFrame {
                            overlay: None,
                            frame,
                            resources: Vec::new(),
                        },
                        &mut bank,
                        &view,
                    )
                }
            };
            assert!(result.is_err());
        }
        bank.resources(&retained).unwrap();
        let frame = stage(&mut bank, 26);
        finish(frame, &mut bank, "HUD").unwrap();
    }
}
