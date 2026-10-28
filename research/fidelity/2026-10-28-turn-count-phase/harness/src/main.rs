//! Offline count reconstruction and authored geometry, using a pinned client.
use serde::Deserialize;
use serde_json::{Value, json};
use slither_client::{
    presentation::{AnchorView, AnimatedClient, FrameClock, SnakePresentation},
    protocol::{ElapsedHeader, decode_frame},
    state::{Appearance, ClientState, Direction, Point, PresentationEvent, Setup, SnakeTarget},
};
use std::{
    collections::BTreeMap,
    error::Error,
    io::{self, BufRead, Write},
};

fn count(s: &SnakePresentation) -> Value {
    let m = s.metrics();
    let physical = s.body().points().len();
    let dying = s.body().points().iter().filter(|p| p.dying()).count();
    json!({"logical":m.logical_segments,"physical":physical,"dying":dying,
        "physical_minus_dying":physical-dying,"fam":m.fam,"displayed_length":m.displayed_length,
        "scale":m.scale,"scang":m.size_turn_factor})
}
fn setup() -> Setup {
    Setup {
        protocol: 19,
        grid_center: 10000.,
        radius: 9800.,
        sector_size: 300,
        sector_count: 100,
        max_segments: 430,
        body_spacing: 42.,
        body_contraction: 0.43,
        spangdv: 4.8,
        nsp1: 5.39,
        nsp2: 0.4,
        nsp3: 14.,
        mamu: 0.033,
        mamu2: 0.029,
        server_number: None,
        game_mode: None,
        team: None,
    }
}
fn appearance(n: usize) -> Appearance {
    let mut body = vec![Point { x: 0., y: 0. }];
    for i in 1..n {
        let a = 0.04 * i as f64;
        let p = body[i - 1];
        body.push(Point {
            x: p.x - 42. * a.cos(),
            y: p.y - 42. * a.sin(),
        });
    }
    Appearance {
        id: 1,
        skin: 0,
        name: String::new(),
        custom_skin: vec![],
        cosmetic: 0,
        absolute_head: Point { x: 12., y: 0. },
        absolute_body: body,
        anchor_direction: Some(0),
        heading: 0.,
        wanted_heading: 2.,
        speed_per_32ms: 5.79,
        fam: 0.25,
        wire_direction: Direction::None,
    }
}
fn contract(points: &mut [Point]) {
    for i in 3..points.len() {
        let w = 0.43 * ((i - 2) as f64 / 4.).min(1.);
        points[i].x += (points[i - 1].x - points[i].x) * w;
        points[i].y += (points[i - 1].y - points[i].y) * w;
    }
}
fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}
fn geometry() -> Result<(), Box<dyn Error>> {
    let mut cases = vec![];
    for n in [8, 37] {
        for grow in [false, true] {
            let a = appearance(n);
            let mut s = SnakePresentation::from_appearance(&a, &setup(), 1.)?;
            s.set_visible(true);
            let before = count(&s);
            let front = Point { x: 42., y: 0. };
            s.apply_anchor(
                Some(front),
                Some(0),
                grow.then_some(0.25),
                AnchorView {
                    zoom: 1.,
                    extrapolated_distance: 0.,
                },
            )?;
            let actual: Vec<_> = s.body().points().iter().map(|p| p.position()).collect();
            let mut client = a.absolute_body.clone();
            client.insert(0, front);
            contract(&mut client);
            let exact = actual.iter().zip(&client).all(|(a, b)| a == b);
            if !exact {
                return Err("client insertion-order probe disagrees".into());
            }
            let mut server = a.absolute_body.clone();
            contract(&mut server);
            server.insert(0, front);
            if !grow {
                server.pop();
            }
            let live: Vec<_> = s
                .body()
                .points()
                .iter()
                .filter(|p| !p.dying())
                .map(|p| p.position())
                .collect();
            let max = live
                .iter()
                .zip(&server)
                .map(|(a, b)| distance(*a, *b))
                .fold(0., f64::max);
            cases.push(
                json!({"segments":n,"grew":grow,"before":before,"after":count(&s),
                "client_insert_contract_exact":exact,"live_point_max_vs_contract_insert":max,
                "server_physical_count":server.len()}),
            );
        }
    }
    let mut lifecycle = vec![];
    for n in [8, 37] {
        let mut s = SnakePresentation::from_appearance(&appearance(n), &setup(), 1.)?;
        let mut clock = FrameClock::new(0.)?;
        let mut rows = vec![json!({"action":"appearance","count":count(&s)})];
        s.set_mass(0.75)?;
        rows.push(json!({"action":"mass_only","count":count(&s)}));
        s.advance(clock.advance(8., false, false)?)?;
        rows.push(json!({"action":"advance_8ms","count":count(&s)}));
        s.advance(clock.advance(16., false, false)?)?;
        rows.push(json!({"action":"advance_16ms","count":count(&s)}));
        s.apply_anchor(
            Some(Point { x: 42., y: 0. }),
            Some(0),
            Some(0.1),
            AnchorView {
                zoom: 1.,
                extrapolated_distance: 0.,
            },
        )?;
        rows.push(json!({"action":"growth","count":count(&s)}));
        s.apply_anchor(
            Some(Point { x: 84., y: 0. }),
            Some(0),
            None,
            AnchorView {
                zoom: 1.,
                extrapolated_distance: 0.,
            },
        )?;
        rows.push(json!({"action":"non_growth","count":count(&s)}));
        s.remove_tail(Some(0.9), 1.)?;
        rows.push(json!({"action":"tail_loss","count":count(&s)}));
        for ms in (24..=6000).step_by(8) {
            s.advance(clock.advance(ms as f64, false, false)?)?;
        }
        rows.push(json!({"action":"advance_to_6000ms","count":count(&s)}));
        lifecycle.push(json!({"initial_segments":n,"rows":rows}));
    }
    println!(
        "{}",
        json!({"origin":"authored synthetic geometry; no official bytes/assets",
        "cases":cases,"lifecycle":lifecycle})
    );
    Ok(())
}

#[derive(Deserialize)]
struct Input {
    line: usize,
    connection: u32,
    t_ms: f64,
    bytes: Vec<u8>,
}
fn target_id(client: &AnimatedClient, target: SnakeTarget) -> Option<u16> {
    match target {
        SnakeTarget::Own => client.received().own_snake_id,
        SnakeTarget::Id(id) => Some(id),
    }
}
fn replay() -> Result<(), Box<dyn Error>> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    let mut client = AnimatedClient::new(0.)?;
    let mut decoder = ClientState::default();
    let mut connection = None;
    let mut ready = false;
    let mut server_ms = 0u64;
    let mut legacy_clock: Option<u64> = None;
    let mut start = 0.;
    let mut next_tick = 1000. / 60.;
    let mut tracks = BTreeMap::<u16, (usize, usize)>::new();
    let mut frames = 0;
    let mut anchors = 0;
    for line in io::stdin().lock().lines() {
        let i: Input = serde_json::from_str(&line?)?;
        if connection != Some(i.connection) {
            connection = Some(i.connection);
            ready = false;
            tracks.clear();
            client = AnimatedClient::new(0.)?;
            decoder = ClientState::default();
            legacy_clock = None;
        }
        let frame = decode_frame(&i.bytes, ElapsedHeader::TwoBytes)?;
        // The introducing study zeroed its first incoming frame, then accumulated
        // every header, including setup's initial epoch. Preserve that provenance
        // clock alongside the current setup-relative timeline, without fitting an offset.
        legacy_clock = Some(legacy_clock.map_or(0, |t| t + u64::from(frame.elapsed_ms.unwrap())));
        if frame.commands.iter().all(|p| p[0] == b'6') {
            continue;
        }
        let new_setup = frame.commands.iter().any(|p| p[0] == b'a');
        if new_setup {
            server_ms = 0;
            start = i.t_ms;
            next_tick = 1000. / 60.;
            ready = true;
        }
        if !ready {
            continue;
        }
        if !new_setup {
            server_ms += u64::from(frame.elapsed_ms.ok_or("missing header")?);
        }
        while next_tick <= i.t_ms - start + 1e-9 {
            client.advance(next_tick, false)?;
            next_tick += 1000. / 60.;
        }
        let mut validation = decoder.clone();
        validation.apply_frame_with_events(&i.bytes, ElapsedHeader::TwoBytes)?;
        frames += 1;
        for (packet, command) in frame.commands.iter().enumerate() {
            let events = decoder
                .apply_frame_with_events(command, ElapsedHeader::Absent)?
                .events;
            let mut probe = None;
            for e in &events {
                if let PresentationEvent::SnakeAnchor { target, .. } = e
                    && let Some(id) = target_id(&client, *target)
                    && let Some(s) = client.snake(id)
                {
                    probe = Some((id, count(s)));
                }
            }
            client.apply_frame(command, ElapsedHeader::Absent)?;
            for e in &events {
                match e {
                    PresentationEvent::Setup(s) => {
                        if (s.mamu - 0.033).abs() > 1e-12 {
                            return Err("unexpected turn coefficient".into());
                        }
                    }
                    PresentationEvent::SnakeAppearance(a) => {
                        let t = tracks.entry(a.id).or_default();
                        t.0 += 1;
                        t.1 = 0;
                    }
                    PresentationEvent::SnakeAnchor { .. } => {
                        if let Some((id, before)) = &probe
                            && let Some(s) = client.snake(*id)
                        {
                            let t = tracks.get_mut(id).ok_or("anchor before appearance")?;
                            let p = s
                                .body()
                                .points()
                                .first()
                                .ok_or("anchor without body")?
                                .position();
                            let wire_count = client.received().snakes.get(id).map(|s| s.body.len());
                            writeln!(
                                out,
                                "{}",
                                json!({"line":i.line,"connection":i.connection,"id":id,
                                "generation":t.0,"index":t.1,"server_ms":server_ms,"packet":packet,
                                "legacy_server_ms":legacy_clock.unwrap(),
                                "anchor":[p.x,p.y],"before":before,"after":count(s),
                                "wire_count":wire_count,"wanted":s.metrics().wanted_heading,
                                "speed_per_32ms":s.metrics().speed_per_32ms,"family":(command[0] as char).to_string()})
                            )?;
                            t.1 += 1;
                            anchors += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    writeln!(
        out,
        "{}",
        json!({"kind":"audit","frames":frames,"anchors":anchors})
    )?;
    Ok(())
}
fn main() {
    let result = if std::env::args().nth(1).as_deref() == Some("geometry") {
        geometry()
    } else {
        replay()
    };
    if let Err(e) = result {
        eprintln!("turn-count-research: {e}");
        std::process::exit(1);
    }
}
