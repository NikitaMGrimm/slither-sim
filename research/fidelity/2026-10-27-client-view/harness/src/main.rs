//! Offline numeric replay through the pinned client's decoder and presentation.
use serde::Deserialize;
use serde_json::{Value, json};
use slither_client::{
    presentation::{
        AnimatedClient, BodyAsset, BodyAssetStatus, ClientPresentationError, FoodFactory,
        FoodSource, PairedFaceGeometry, PreyFactory, SnakeMetrics,
    },
    protocol::{ElapsedHeader, decode_frame},
    state::{ClientState, Point, PresentationEvent, SnakeTarget},
};
use std::{
    collections::BTreeMap,
    error::Error,
    io::{self, BufRead, Write},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    line: usize,
    connection: usize,
    t_ms: f64,
    bytes: Vec<u8>,
    #[serde(default)]
    unframed: bool,
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}
fn angle(a: f64, b: f64) -> f64 {
    ((a - b + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI).abs()
}
fn source() -> FoodSource {
    // Synthetic metadata, without pixels. Source selection cannot affect motion.
    FoodSource::new(
        BodyAsset {
            handle: 1,
            revision: 1,
            width: 2,
            height: 2,
            status: BodyAssetStatus::Ready,
        },
        2.,
        2.,
        1.,
        1.,
    )
    .unwrap()
}
fn food(raw: u8) -> FoodFactory {
    FoodFactory {
        random_phase_unit: 0.25,
        random_wobble_unit: 0.5,
        palette: slither_client::presentation::food_palette(raw),
        image_count: 1,
        bodies: vec![source()],
        outlines: vec![source()],
        glows: vec![source()],
    }
}
fn prey(raw: u8) -> PreyFactory {
    PreyFactory {
        random_phase_unit: 0.25,
        random_rate_unit: 0.5,
        palette: raw % 9,
        bodies: vec![source()],
        glows: vec![source()],
    }
}
fn apply(
    client: &mut AnimatedClient,
    bytes: &[u8],
    header: ElapsedHeader,
) -> Result<(), ClientPresentationError> {
    client.apply_frame_with_all_factories::<ClientPresentationError, _, _, _>(
        bytes,
        header,
        |_| Ok(PairedFaceGeometry::default()),
        |s, _| Ok(food(s.raw_color)),
        |s, _| Ok(prey(s.raw_color)),
    )?;
    Ok(())
}

#[derive(Clone)]
struct Before {
    token: u64,
    m: SnakeMetrics,
    body: Vec<(Point, Point)>,
}
fn snapshot(client: &AnimatedClient, target: SnakeTarget) -> Option<Before> {
    let snake = match target {
        SnakeTarget::Own => client.own_snake(),
        SnakeTarget::Id(id) => client.snake(id),
    }?;
    let token = client
        .snake_objects()
        .find(|(_, _, _, s)| std::ptr::eq(*s, snake))?
        .0
        .serial();
    Some(Before {
        token,
        m: snake.metrics(),
        body: snake
            .body()
            .points()
            .iter()
            .map(|p| {
                let a = p.position();
                let o = p.position_offset();
                (
                    a,
                    Point {
                        x: a.x + o.x,
                        y: a.y + o.y,
                    },
                )
            })
            .collect(),
    })
}
fn strata(m: SnakeMetrics) -> Value {
    json!({"segments":m.logical_segments,"speed_per_32ms":m.speed_per_32ms,
        "visible":m.visible,"scale":m.scale})
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let clock = args.get(1).map(String::as_str).unwrap_or("receipt");
    let step_ms: f64 = args
        .get(2)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(1000. / 60.);
    if !["receipt", "server"].contains(&clock)
        || !step_ms.is_finite()
        || !(1. ..=40.).contains(&step_ms)
    {
        return Err("usage: client-view-research [receipt|server] [tick_ms in 1..40]".into());
    }
    let mut out = io::BufWriter::new(io::stdout().lock());
    let mut client = AnimatedClient::new(0.)?;
    let mut decoder = ClientState::default();
    let mut connection = None;
    let mut ready = false;
    let mut session_live = false;
    let mut start = 0.;
    let mut next_tick = step_ms;
    let mut server_ms = 0.;
    let mut previous_t = None;
    let mut counts = BTreeMap::<String, u64>::new();
    let mut frames = 0u64;
    let mut setups = 0u64;
    let mut max_t: f64 = 0.;
    let mut prey_times = BTreeMap::new();
    for raw in io::stdin().lock().lines() {
        let input: Input = serde_json::from_str(&raw?)?;
        if !input.t_ms.is_finite() {
            return Err("nonfinite receipt time".into());
        }
        if connection != Some(input.connection) {
            connection = Some(input.connection);
            ready = false;
            previous_t = None;
        }
        if previous_t.is_some_and(|t| input.t_ms < t) {
            return Err("regressing receipt time".into());
        }
        previous_t = Some(input.t_ms);
        let header = if input.unframed {
            ElapsedHeader::Absent
        } else {
            ElapsedHeader::TwoBytes
        };
        let frame = decode_frame(&input.bytes, header)?;
        // A challenge is never interpreted, evaluated, echoed or used to connect.
        if frame.commands.iter().all(|p| p[0] == b'6') {
            continue;
        }
        let setup = frame.commands.iter().any(|p| p[0] == b'a');
        if setup {
            // Each setup is an explicitly separate replay epoch, including reconnects.
            client = AnimatedClient::new(0.)?;
            decoder = ClientState::default();
            start = input.t_ms;
            next_tick = step_ms;
            server_ms = 0.;
            ready = true;
            session_live = true;
            setups += 1;
            prey_times.clear();
        }
        if !ready {
            *counts.entry("pre_setup_frames".into()).or_default() += 1;
            continue;
        }
        if !setup {
            server_ms += f64::from(
                frame
                    .elapsed_ms
                    .ok_or("server clock requires timed framing")?,
            );
        }
        let now = if clock == "server" {
            server_ms
        } else {
            input.t_ms - start
        };
        max_t = max_t.max(now);
        while next_tick <= now + 1e-9 {
            client.advance(next_tick, false)?;
            client.prepare_draw(1100, 800)?;
            next_tick += step_ms;
        }
        // Validate the complete original frame before exposing any packet measurements.
        let mut validation = decoder.clone();
        validation.apply_frame_with_events(&input.bytes, header)?;
        let mut whole_frame = client.clone();
        apply(&mut whole_frame, &input.bytes, header)?;
        frames += 1;
        for (packet_index, command) in frame.commands.iter().enumerate() {
            // Generic native presentation leaves game-over opaque. Stop the live-view
            // measurement at v, before following bundled packets (census convention).
            if command[0] == b'v' {
                session_live = false;
            }
            let events = decoder
                .apply_frame_with_events(command, ElapsedHeader::Absent)?
                .events;
            *counts.entry((command[0] as char).to_string()).or_default() += 1;
            let mut records = Vec::new();
            for event in &events {
                match event {
                    PresentationEvent::SnakeMotion { target, .. }
                    | PresentationEvent::SnakeAnchor { target, .. } => {
                        if let Some(before) = snapshot(&client, *target) {
                            let own = client
                                .own_token()
                                .is_some_and(|t| t.serial() == before.token);
                            if !own && before.m.alive {
                                records.push((event.clone(), before));
                            }
                        }
                    }
                    _ => {}
                }
            }
            let prey_before: Vec<_> = client.preys().snapshots().collect();
            apply(&mut client, command, ElapsedHeader::Absent)?;
            for (event, before) in records {
                let target = match event {
                    PresentationEvent::SnakeMotion { target, .. }
                    | PresentationEvent::SnakeAnchor { target, .. } => target,
                    _ => unreachable!(),
                };
                let Some(after) = snapshot(&client, target) else {
                    continue;
                };
                if before.token != after.token {
                    return Err("identity changed inside an incremental update".into());
                }
                let mut row = json!({"line":input.line,"connection":input.connection,"epoch":setups,
                    "packet":packet_index,"time_ms":now,"id":before.m.id,"token":before.token,
                    "own_alive":session_live && client.active_own_snake().is_some_and(|s|s.metrics().alive),
                    "stratum":strata(before.m)});
                match event {
                    PresentationEvent::SnakeAnchor { grew, .. } => {
                        row["kind"] = json!("snake_anchor");
                        row["grew"] = json!(grew);
                        row["head_innovation"] =
                            json!(distance(before.m.raw_head, after.m.raw_head));
                        row["head_immediate_display_shift"] =
                            json!(distance(before.m.corrected_head, after.m.corrected_head));
                        // Native BodyGeometry inserts at the FRONT. Existing physical identities shift by one.
                        let pairs = before.body.iter().zip(after.body.iter().skip(1));
                        let mut raw_max: f64 = 0.;
                        let mut display_max: f64 = 0.;
                        let mut n = 0;
                        for ((a, ad), (b, bd)) in pairs {
                            raw_max = raw_max.max(distance(*a, *b));
                            display_max = display_max.max(distance(*ad, *bd));
                            n += 1;
                        }
                        row["body_existing_points"] = json!(n);
                        row["body_raw_contraction_max"] = json!(raw_max);
                        row["body_immediate_display_shift_max"] = json!(display_max);
                        let (p, pd) = after.body[0];
                        row["body_new_front_correction"] = json!(distance(p, pd));
                    }
                    PresentationEvent::SnakeMotion {
                        heading,
                        speed_per_32ms,
                        ..
                    } => {
                        row["kind"] = json!("snake_motion");
                        if let Some(h) = heading {
                            row["heading_error_rad"] = json!(angle(before.m.heading, h));
                        }
                        if let Some(s) = speed_per_32ms {
                            row["speed_error_per_32ms"] =
                                json!((before.m.speed_per_32ms - s).abs());
                        }
                        row["heading_immediate_display_shift_rad"] =
                            json!(angle(before.m.corrected_heading, after.m.corrected_heading));
                    }
                    _ => unreachable!(),
                }
                writeln!(out, "{row}")?;
            }
            for event in events {
                if let PresentationEvent::PreyMotion(m) = event {
                    let before = prey_before.iter().rev().find(|p| p.id == m.id && !p.eaten);
                    let after = client.preys().snapshots().filter(|p| p.id == m.id).last();
                    if let (Some(a), Some(b)) = (before, after) {
                        if a.instance != b.instance {
                            return Err("prey identity changed inside motion".into());
                        }
                        let ao = a.corrections.offset();
                        let bo = b.corrections.offset();
                        let camera = client.camera().position();
                        let zoom = client.camera().zoom();
                        let in_viewport = (a.position.x + ao[0] - camera.x).abs() <= 550. / zoom
                            && (a.position.y + ao[1] - camera.y).abs() <= 400. / zoom;
                        let mut row = json!({"kind":"prey_motion","line":input.line,"connection":input.connection,
                            "epoch":setups,"packet":packet_index,"time_ms":now,"id":m.id,
                            "innovation":distance(a.position,b.position),"immediate_display_shift":distance(
                                Point{x:a.position.x+ao[0],y:a.position.y+ao[1]},
                                Point{x:b.position.x+bo[0],y:b.position.y+bo[1]}),
                            "speed_per_32ms":a.speed_per_32ms,
                            "own_alive":session_live && client.active_own_snake().is_some_and(|s|s.metrics().alive),
                            "center_in_viewport":in_viewport});
                        row["prior_raw_position"] = json!([a.position.x, a.position.y]);
                        row["reported_position"] = json!([b.position.x, b.position.y]);
                        row["prior_heading"] = json!(a.heading);
                        row["reported_heading"] = json!(b.heading);
                        if let Some(last) = prey_times.insert(a.instance.serial(), now) {
                            row["correction_gap_ms"] = json!(now - last);
                        }
                        if let Some(h) = m.heading {
                            row["heading_error_rad"] = json!(angle(a.heading, h));
                        }
                        if let Some(s) = m.speed_per_32ms {
                            row["speed_error_per_32ms"] = json!((a.speed_per_32ms - s).abs());
                        }
                        writeln!(out, "{row}")?;
                    } else {
                        *counts.entry("unmatched_prey_motion".into()).or_default() += 1;
                    }
                }
            }
        }
        // Packet probes must reproduce original complete-frame presentation transitions.
        let a: Vec<_> = client.snake_objects().collect();
        let b: Vec<_> = whole_frame.snake_objects().collect();
        if a.len() != b.len()
            || !a
                .iter()
                .zip(&b)
                .all(|(a, b)| a.0 == b.0 && a.1 == b.1 && a.2 == b.2 && a.3 == b.3)
            || client.preys() != whole_frame.preys()
            || client.foods() != whole_frame.foods()
        {
            return Err(format!(
                "packet/full-frame presentation mismatch at input line {}",
                input.line
            )
            .into());
        }
    }
    writeln!(
        out,
        "{}",
        json!({"kind":"audit","frames":frames,"setups":setups,
        "packet_counts":counts,"clock":clock,"step_ms":step_ms,"max_epoch_ms":max_t,
        "whole_frame_parity_checks":frames})
    )?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("client-view-research: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn angles_wrap_across_wire_boundary() {
        assert!((angle(0.01, std::f64::consts::TAU - 0.01) - 0.02).abs() < 1e-12);
    }
}
