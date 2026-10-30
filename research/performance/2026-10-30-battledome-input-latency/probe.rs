//! Research helper: copy to examples/input_latency_probe.rs on the remote build.
//! input_latency_probe PRIVATE_IP PORT OUTPUT_DIRECTORY SECONDS SEED
//! Create START to enable attempts and STOP to finish. A capture controller must
//! acknowledge connection-N.json with capture-ready-N before the stock handshake.
//! No authoritative state or server measurement hooks are used.
use serde_json::json;
use sha2::{Digest, Sha256};
use slither_sim::{
    Action,
    client::{
        Bot, ClientState, ClientStatus, ForagerBot, challenge,
        network::login_packet,
        protocol::{action_packets, split_frame},
    },
};
use std::{
    collections::VecDeque,
    f64::consts::TAU,
    fs::{self, File},
    io::{self, BufWriter, Write},
    net::{Ipv4Addr, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tungstenite::{Message, WebSocket, client::IntoClientRequest};

fn unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

fn sha(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 15) as usize] as char);
    }
    value
}

/// Midpoint command decoding returns the exact 251-step bin. Encode that bin
/// on the wire's 256-step circle using integer arithmetic, independently of Safety.
fn expected(command: u8) -> u8 {
    (((u32::from(command) * 256 + 125) / 251) % 256) as u8
}

/// Explicit own wanted fields only, following protocol.rs's length dispatch.
/// Opponent motion includes an ID and cannot serve as an echo.
fn own_wanted(packet: &[u8]) -> Option<u8> {
    match packet {
        [b'E' | b'4', wanted] | [b'E' | b'4', wanted, _] => Some(*wanted),
        [b'3' | b'5', _, wanted] | [b'd' | b'7', _, wanted, _] => Some(*wanted),
        _ => None,
    }
}

struct Random(u64);
impl Random {
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / ((1_u64 << 53) as f64)
    }
    fn interval(&mut self) -> f64 {
        0.25 + 0.4 * self.unit()
    }
}

#[derive(Default)]
struct Echoes {
    latest: Option<u8>,
    outstanding: VecDeque<u8>,
}
impl Echoes {
    fn sent(&mut self, command: u8) -> Result<(), String> {
        let value = expected(command);
        if self.outstanding.back() != Some(&value) {
            self.outstanding.push_back(value);
        }
        if self.outstanding.len() > 4096 {
            return Err("Unbounded unmatched heading history".into());
        }
        Ok(())
    }
    fn receive(&mut self, wanted: u8) {
        self.latest = Some(wanted);
        // First occurrence is deliberately conservative when targets repeat:
        // later possible commands stay excluded from a new probe's target.
        if let Some(index) = self.outstanding.iter().position(|&value| value == wanted) {
            self.outstanding.drain(..=index);
        }
    }
    fn eligible(&self, command: u8) -> bool {
        let value = expected(command);
        self.latest != Some(value) && !self.outstanding.contains(&value)
    }
}

struct Pending {
    id: u64,
    sent: f64,
    command: u8,
}

fn send(
    socket: &mut WebSocket<TcpStream>,
    state: &mut ClientState,
    bytes: Vec<u8>,
    at: f64,
) -> Result<(), String> {
    socket
        .send(Message::Binary(bytes.clone().into()))
        .map_err(|e| e.to_string())?;
    state.record_sent(&bytes, at)
}

fn connect(
    ip: Ipv4Addr,
    port: u16,
    root: &Path,
    life: u64,
) -> Result<WebSocket<TcpStream>, String> {
    let tcp = TcpStream::connect((ip, port)).map_err(|e| e.to_string())?;
    tcp.set_nodelay(true).map_err(|e| e.to_string())?;
    tcp.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    tcp.set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let note = json!({"life":life,"localPort":tcp.local_addr().unwrap().port(),"unix":unix()});
    let temporary = root.join(format!("connection-{life}.tmp"));
    fs::write(&temporary, note.to_string()).map_err(|e| e.to_string())?;
    fs::rename(temporary, root.join(format!("connection-{life}.json")))
        .map_err(|e| e.to_string())?;
    let wait = Instant::now();
    while !root.join(format!("capture-ready-{life}")).exists() {
        if wait.elapsed().as_secs() >= 5 {
            return Err("Capture readiness timeout".into());
        }
        thread::sleep(Duration::from_millis(2));
    }
    let mut request = format!("ws://{ip}:{port}/slither")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("x-slithersim-food", "physical".parse().unwrap());
    let (mut socket, _) = tungstenite::client(request, tcp).map_err(|e| e.to_string())?;
    socket
        .send(Message::Binary(vec![99, 0].into()))
        .map_err(|e| e.to_string())?;
    socket
        .get_mut()
        .set_nonblocking(true)
        .map_err(|e| e.to_string())?;
    Ok(socket)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [ip, port, directory, seconds, seed] = args.as_slice() else {
        return Err("PRIVATE_IP PORT OUTPUT_DIRECTORY SECONDS SEED".into());
    };
    let ip: Ipv4Addr = ip
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    if !ip.is_private() && !ip.is_loopback() {
        return Err("Study-owned private/loopback only".into());
    }
    let port = port.parse::<u16>().map_err(|e| e.to_string())?;
    let limit = seconds.parse::<f64>().map_err(|e| e.to_string())?;
    if !limit.is_finite() || !(0. ..=7200.).contains(&limit) {
        return Err("Invalid duration".into());
    }
    let mut random = Random(seed.parse::<u64>().map_err(|e| e.to_string())?);
    if random.0 == 0 {
        return Err("Seed must be nonzero".into());
    }
    let root = PathBuf::from(directory);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let epoch = Instant::now();
    let mut records = vec![json!({"type":"start","unix":unix(),"limitSeconds":limit})];
    let mut life = 0_u64;
    let mut sample = 0_u64;
    let mut ping_token = 0_u64;
    while epoch.elapsed().as_secs_f64() < limit && !root.join("STOP").exists() {
        life += 1;
        let mut socket = connect(ip, port, &root, life)?;
        let mut state = ClientState::new();
        state.new_connection(epoch.elapsed().as_secs_f64())?;
        let mut policy = ForagerBot::new();
        let mut echoes = Echoes::default();
        let mut pending: Option<Pending> = None;
        let mut control_ping: Option<(u64, f64, f64)> = None;
        let mut game_ping: Option<(f64, f64)> = None;
        let mut next_ping = epoch.elapsed().as_secs_f64() + 0.2;
        let mut next_probe = epoch.elapsed().as_secs_f64() + random.interval();
        let mut next_decision = 0.;
        let mut action = Action {
            heading: 0.,
            boost: false,
        };
        let mut boost = None;
        let mut frame_sequence = 0_u64;
        let mut header_ms = 0_u64;
        let mut previous_pass = epoch.elapsed().as_secs_f64();
        let mut previous_frame: Option<f64> = None;
        let mut started = false;
        loop {
            let now = epoch.elapsed().as_secs_f64();
            if now >= limit || root.join("STOP").exists() {
                break;
            }
            if !started && root.join("START").exists() {
                started = true;
                next_probe = now + random.interval();
                records.push(json!({"type":"enabled","life":life,"at":now,"unix":unix()}));
            }
            let pass_gap = now - previous_pass;
            previous_pass = now;
            for _ in 0..512 {
                match socket.read() {
                    Ok(Message::Binary(bytes)) => {
                        let received = epoch.elapsed().as_secs_f64();
                        let receipt_unix = unix();
                        let packets = split_frame(&bytes)?;
                        let was_playing = state.status() == ClientStatus::Playing;
                        state.receive_frame(&bytes, received)?;
                        let decode_seconds = epoch.elapsed().as_secs_f64() - received;
                        header_ms += u64::from(u16::from_be_bytes([bytes[0], bytes[1]]));
                        frame_sequence += 1;
                        let wanted: Vec<_> = packets
                            .iter()
                            .filter_map(|packet| own_wanted(packet))
                            .collect();
                        records.push(json!({"type":"frame","life":life,"seq":frame_sequence,"at":received,
                            "unix":receipt_unix,"bytes":bytes.len(),"sha256":sha(&bytes),
                            "wasPlaying":was_playing,"playing":state.status()==ClientStatus::Playing,
                            "headerMs":header_ms,"wanted":wanted,"decodeSeconds":decode_seconds,
                            "passGapSeconds":pass_gap,"gapSeconds":previous_frame.map(|at|received-at)}));
                        previous_frame = Some(received);
                        for packet in packets {
                            if packet.first() == Some(&b'6') {
                                send(
                                    &mut socket,
                                    &mut state,
                                    challenge::solve_challenge(&packet[1..])?,
                                    received,
                                )?;
                                send(
                                    &mut socket,
                                    &mut state,
                                    login_packet("LatencyProbe", 0)?,
                                    received,
                                )?;
                            }
                            if packet == [b'p']
                                && let Some((sent, sent_unix)) = game_ping.take()
                            {
                                records.push(json!({"type":"gameRtt","life":life,"at":received,"unix":receipt_unix,
                                    "sentUnix":sent_unix,"milliseconds":(received-sent)*1000.}));
                            }
                            if let Some(value) = own_wanted(packet) {
                                if pending
                                    .as_ref()
                                    .is_some_and(|probe| expected(probe.command) == value)
                                {
                                    let probe = pending.take().unwrap();
                                    records.push(json!({"type":"echo","life":life,"id":probe.id,"at":received,
                                        "unix":receipt_unix,"frame":frame_sequence,"wanted":value,
                                        "milliseconds":(received-probe.sent)*1000.}));
                                }
                                echoes.receive(value);
                            }
                        }
                    }
                    Ok(Message::Pong(bytes)) => {
                        let at = epoch.elapsed().as_secs_f64();
                        if let Some((token, sent, sent_unix)) = control_ping
                            && bytes.as_ref() == token.to_be_bytes()
                        {
                            records.push(
                                json!({"type":"controlRtt","life":life,"token":token,"at":at,
                                "unix":unix(),"sentUnix":sent_unix,"milliseconds":(at-sent)*1000.}),
                            );
                            control_ping = None;
                        } else {
                            return Err("Unexpected/mismatched control pong".into());
                        }
                    }
                    Ok(Message::Ping(_)) => {
                        socket.flush().map_err(|e| e.to_string())?;
                    }
                    Ok(Message::Close(_)) | Err(tungstenite::Error::ConnectionClosed) => {
                        return Err("Unexpected probe close".into());
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(error))
                        if error.kind() == io::ErrorKind::WouldBlock =>
                    {
                        break;
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
            let now = epoch.elapsed().as_secs_f64();
            if state.status() == ClientStatus::Dead {
                if let Some(probe) = pending.take() {
                    records.push(
                        json!({"type":"censored","reason":"death","life":life,"id":probe.id,
                        "at":now,"unix":unix()}),
                    );
                }
                records.push(json!({"type":"death","life":life,"at":now,"unix":unix()}));
                let _ = socket.close(None);
                break;
            }
            if pending.as_ref().is_some_and(|probe| now - probe.sent >= 1.) {
                let probe = pending.take().unwrap();
                records.push(
                    json!({"type":"censored","reason":"timeout","life":life,"id":probe.id,
                    "at":now,"unix":unix()}),
                );
            }
            if now >= next_decision && state.status() == ClientStatus::Playing {
                let frame = state.frame(now)?;
                if let Some(mut decision) = policy.decide(&frame) {
                    if frame
                        .rules
                        .as_ref()
                        .is_some_and(|rules| rules.free_boost_at_floor)
                    {
                        decision.boost = true;
                    }
                    action = decision;
                    if pending.is_none() {
                        for packet in action_packets(action, boost)? {
                            if packet.len() == 1 && packet[0] <= 250 {
                                echoes.sent(packet[0])?;
                            }
                            send(&mut socket, &mut state, packet, now)?;
                        }
                        boost = Some(action.boost);
                    }
                }
                records.push(json!({"type":"decision","life":life,"at":now,"unix":unix(),
                    "lateSeconds":(now-next_decision).max(0.),"boost":action.boost}));
                next_decision = now + 0.042;
            }
            if started
                && now >= next_probe
                && pending.is_none()
                && state.status() == ClientStatus::Playing
            {
                let due = next_probe;
                next_probe = now + random.interval();
                let offset =
                    (0.25 + 0.4 * random.unit()) * if random.unit() < 0.5 { -1. } else { 1. };
                let first = ((action.heading + offset).rem_euclid(TAU) * 251. / TAU).floor() as u8;
                let command = (0..251)
                    .map(|step| ((u16::from(first) + step) % 251) as u8)
                    .find(|&command| echoes.eligible(command));
                if let Some(command) = command {
                    sample += 1;
                    let sent = epoch.elapsed().as_secs_f64();
                    let sent_unix = unix();
                    send(&mut socket, &mut state, vec![command], sent)?;
                    let returned = epoch.elapsed().as_secs_f64();
                    records.push(
                        json!({"type":"send","life":life,"id":sample,"at":sent,"unix":sent_unix,
                        "command":command,"expected":expected(command),"priorWanted":echoes.latest,
                        "callSeconds":returned-sent,"lateSeconds":(sent-due).max(0.)}),
                    );
                    echoes.sent(command)?;
                    pending = Some(Pending {
                        id: sample,
                        sent,
                        command,
                    });
                } else {
                    records.push(json!({"type":"skipped","reason":"no-unambiguous-target","at":now,"unix":unix()}));
                }
            }
            if now >= next_ping && control_ping.is_none() {
                ping_token += 1;
                let sent = epoch.elapsed().as_secs_f64();
                let sent_unix = unix();
                socket
                    .send(Message::Ping(ping_token.to_be_bytes().to_vec().into()))
                    .map_err(|e| e.to_string())?;
                control_ping = Some((ping_token, sent, sent_unix));
                next_ping = now + 0.8 + 0.4 * random.unit();
            }
            if control_ping.is_some_and(|(_, sent, _)| now - sent > 5.) {
                return Err("Control pong timeout".into());
            }
            let ping_at = epoch.elapsed().as_secs_f64();
            if state.ping_due(ping_at)? {
                game_ping = Some((ping_at, unix()));
                send(&mut socket, &mut state, vec![251], ping_at)?;
            }
            socket.flush().map_err(|e| e.to_string())?;
            thread::sleep(Duration::from_micros(100));
        }
        if let Some(probe) = pending.take() {
            records.push(
                json!({"type":"censored","reason":"window-end","life":life,"id":probe.id,
                "at":epoch.elapsed().as_secs_f64(),"unix":unix()}),
            );
        }
        records.push(
            json!({"type":"lifeEnd","life":life,"at":epoch.elapsed().as_secs_f64(),"unix":unix()}),
        );
        thread::sleep(Duration::from_millis(30));
    }
    records.push(json!({"type":"complete","at":epoch.elapsed().as_secs_f64(),"unix":unix()}));
    let mut output =
        BufWriter::new(File::create(root.join("probe.jsonl")).map_err(|e| e.to_string())?);
    for record in records {
        writeln!(output, "{record}").map_err(|e| e.to_string())?;
    }
    output.flush().map_err(|e| e.to_string())?;
    fs::write(root.join("COMPLETE"), "complete\n").map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> Result<(), String> {
    run()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_command_matches_midpoint_quantization_and_wire_encoding() {
        for command in 0..=250_u8 {
            let midpoint = (f64::from(command) + 0.5) * TAU / 251.;
            let quantized = slither_sim::physics::quantize_heading(midpoint);
            let byte = ((quantized.rem_euclid(TAU) / TAU * 256.).round() as u32 % 256) as u8;
            assert_eq!(expected(command), byte);
        }
    }
    #[test]
    fn all_own_forms_and_foreign_or_stale_fields() {
        for value in 0..=255_u8 {
            for packet in [
                vec![b'E', value],
                vec![b'4', value],
                vec![b'E', value, 9],
                vec![b'4', value, 9],
                vec![b'3', 12, value],
                vec![b'5', 12, value],
                vec![b'd', 12, value, 9],
                vec![b'7', 12, value, 9],
            ] {
                assert_eq!(own_wanted(&packet), Some(value));
            }
        }
        for packet in [
            vec![],
            vec![b'E'],
            vec![b'e', 42],
            vec![b'3', 42],
            vec![b'G', 0, 42],
            vec![b'E', 0, 1, 42],
            vec![b'3', 0, 1, 20, 42],
            vec![b'E', 0, 1, 20, 42, 9],
        ] {
            assert_eq!(own_wanted(&packet), None);
        }
    }
    #[test]
    fn earlier_outstanding_targets_are_never_selected_and_echo_is_conservative() {
        let mut echoes = Echoes::default();
        echoes.sent(10).unwrap();
        echoes.sent(20).unwrap();
        echoes.sent(10).unwrap();
        assert!(!echoes.eligible(10));
        assert!(!echoes.eligible(20));
        echoes.receive(expected(10));
        assert_eq!(
            echoes.outstanding,
            VecDeque::from([expected(20), expected(10)])
        );
        assert!(!echoes.eligible(10));
        assert!(echoes.eligible(30));
    }
}
