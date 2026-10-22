//! A real-time protocol-19 connection using the same decoder as simulation.
use super::{
    Backend, ClientFrame, ClientState, ClientStatus, WireEvent, WireRecord, challenge,
    protocol::{action_packets, split_frame, split_payload},
};
use crate::Action;
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
use tungstenite::{
    Message, WebSocket, client::IntoClientRequest, http::HeaderValue, protocol::WebSocketConfig,
    stream::MaybeTlsStream,
};

const MAX_MESSAGE_BYTES: usize = 1 << 20;
const MAX_RECORD_BYTES: usize = 32 << 20;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Handshake {
    #[default]
    Challenge,
    Direct,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct NetworkConfig {
    pub url: String,
    pub origin: Option<String>,
    pub nickname: String,
    pub skin: u8,
    pub custom_skin: Vec<u8>,
    /// Request an opaque connection identifier; this does not resume a life.
    pub request_session_id: bool,
    pub decision_interval_seconds: f64,
    pub connect_timeout_seconds: f64,
    pub idle_timeout_seconds: f64,
    pub handshake: Handshake,
    pub record_wire: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            origin: None,
            nickname: "SlitherSim".into(),
            skin: 0,
            custom_skin: Vec::new(),
            request_session_id: false,
            decision_interval_seconds: 0.042,
            connect_timeout_seconds: 5.,
            idle_timeout_seconds: 10.,
            handshake: Handshake::Challenge,
            record_wire: false,
        }
    }
}

impl NetworkConfig {
    pub fn validate(&self) -> Result<(), String> {
        let request = self
            .url
            .as_str()
            .into_client_request()
            .map_err(|e| e.to_string())?;
        if !matches!(request.uri().scheme_str(), Some("ws" | "wss"))
            || request.uri().host().is_none()
            || request
                .uri()
                .authority()
                .is_some_and(|a| a.as_str().contains('@'))
        {
            return Err("Need an explicit ws:// or wss:// server URL without credentials".into());
        }
        for (name, value, maximum) in [
            (
                "decisionIntervalSeconds",
                self.decision_interval_seconds,
                10.,
            ),
            ("connectTimeoutSeconds", self.connect_timeout_seconds, 60.),
            ("idleTimeoutSeconds", self.idle_timeout_seconds, 600.),
        ] {
            if !value.is_finite() || !(0.001..=maximum).contains(&value) {
                return Err(format!("Invalid {name}: need 0.001..{maximum}"));
            }
        }
        if self.nickname.chars().count() > 24 || self.nickname.chars().any(|c| c as u32 > 255) {
            return Err("Nickname must have at most 24 Latin-1 characters".into());
        }
        if self.custom_skin.len() > 255 {
            return Err("Custom skin must have at most 255 bytes".into());
        }
        if let Some(origin) = &self.origin {
            HeaderValue::from_str(origin).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Build the protocol-19 login, without executing any downloaded client code.
pub fn login_packet(nickname: &str, skin: u8) -> Result<Vec<u8>, String> {
    if nickname.chars().count() > 24 || nickname.chars().any(|c| c as u32 > 255) {
        return Err("Nickname must have at most 24 Latin-1 characters".into());
    }
    let name: Vec<_> = nickname.chars().map(|c| c as u8).collect();
    let mut packet = vec![
        115,
        30,
        1,
        35,
        54,
        206,
        204,
        169,
        97,
        178,
        74,
        136,
        124,
        117,
        14,
        210,
        106,
        236,
        8,
        208,
        136,
        213,
        140,
        111,
        skin,
        name.len() as u8,
    ];
    packet.extend(name);
    packet.extend([0, 255]);
    Ok(packet)
}

pub struct NetworkBackend {
    config: NetworkConfig,
    socket: Option<WebSocket<MaybeTlsStream<DeadlineStream>>>,
    state: ClientState,
    epoch: Instant,
    next_decision: f64,
    last_received: f64,
    incoming_frames: usize,
    raw_framing: bool,
    challenge_seen: bool,
    boost: Option<bool>,
    records: Vec<WireRecord>,
    record_bytes: usize,
}

impl NetworkBackend {
    pub fn connect(config: NetworkConfig) -> Result<Self, String> {
        config.validate()?;
        let mut backend = Self {
            config,
            socket: None,
            state: ClientState::new(),
            epoch: Instant::now(),
            next_decision: 0.,
            last_received: 0.,
            incoming_frames: 0,
            raw_framing: false,
            challenge_seen: false,
            boost: None,
            records: Vec::new(),
            record_bytes: 0,
        };
        backend.open()?;
        Ok(backend)
    }

    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }

    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    fn open(&mut self) -> Result<(), String> {
        let mut request = self
            .config
            .url
            .as_str()
            .into_client_request()
            .map_err(|e| e.to_string())?;
        if let Some(origin) = &self.config.origin {
            request.headers_mut().insert(
                "Origin",
                HeaderValue::from_str(origin).map_err(|e| e.to_string())?,
            );
        }
        let uri = request.uri();
        let host = uri.host().ok_or("Missing server host")?;
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let port = uri
            .port_u16()
            .unwrap_or(if uri.scheme_str() == Some("wss") {
                443
            } else {
                80
            });
        // DNS uses the system resolver; the socket and HTTP/TLS handshakes are bounded.
        let addresses = (host, port).to_socket_addrs().map_err(|e| e.to_string())?;
        let deadline =
            Instant::now() + Duration::from_secs_f64(self.config.connect_timeout_seconds);
        let mut last_error = "Server resolved to no addresses".to_string();
        let mut connected = None;
        for address in addresses {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            match TcpStream::connect_timeout(&address, remaining) {
                Ok(stream) => {
                    connected = Some(stream);
                    break;
                }
                Err(error) => last_error = error.to_string(),
            }
        }
        let stream = connected.ok_or(last_error)?;
        stream.set_nodelay(true).map_err(|e| e.to_string())?;
        let stream = DeadlineStream {
            socket: stream,
            read_deadline: deadline,
            write_deadline: deadline,
        };
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let (socket, _) = tungstenite::client_tls_with_config(request, stream, Some(config), None)
            .map_err(|e| e.to_string())?;
        self.socket = Some(socket);
        let now = self.now();
        self.state.new_connection(now)?;
        self.last_received = now;
        self.next_decision = now + self.config.decision_interval_seconds;
        self.incoming_frames = 0;
        self.raw_framing = false;
        self.challenge_seen = false;
        self.boost = None;
        self.record(now, WireEvent::Connect, &[])?;
        if self.config.request_session_id {
            self.send_raw(&[4])?;
        }
        match self.config.handshake {
            Handshake::Challenge => self.send_raw(&[99, 0]),
            Handshake::Direct => self.send_login(),
        }
    }

    fn send_login(&mut self) -> Result<(), String> {
        let mut packet = login_packet(&self.config.nickname, self.config.skin)?;
        packet.extend(&self.config.custom_skin);
        self.send_raw(&packet)
    }

    fn record(&mut self, time: f64, event: WireEvent, bytes: &[u8]) -> Result<(), String> {
        if self.config.record_wire {
            self.record_bytes += bytes.len() + 32;
            self.records.push(WireRecord {
                time,
                event,
                bytes: bytes.to_vec(),
            });
            if self.record_bytes > MAX_RECORD_BYTES {
                return Err(
                    "Wire recording reached 32 MiB; drain records before continuing".into(),
                );
            }
        }
        Ok(())
    }

    fn send_raw(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.config.record_wire && self.record_bytes + bytes.len() + 32 > MAX_RECORD_BYTES {
            return Err("Wire recording reached 32 MiB; drain records before continuing".into());
        }
        let at = self.now();
        let socket = self
            .socket
            .as_mut()
            .ok_or("Disconnected; reconnect explicitly")?;
        transport_stream(socket.get_mut())?.write_deadline =
            Instant::now() + Duration::from_secs_f64(self.config.connect_timeout_seconds);
        socket
            .send(Message::Binary(bytes.to_vec().into()))
            .map_err(|e| e.to_string())?;
        self.record(at, WireEvent::Send, bytes)?;
        self.state.record_sent(bytes, at)
    }

    fn receive_binary(&mut self, bytes: &[u8], at: f64) -> Result<(), String> {
        self.record(at, WireEvent::Receive, bytes)?;
        self.incoming_frames += 1;
        if self.incoming_frames == 1 && bytes.first() == Some(&b'6') {
            self.raw_framing = true;
        }
        let packets = if self.raw_framing {
            split_payload(bytes)?
        } else {
            split_frame(bytes)?
        };
        self.state.receive_frame(bytes, at)?;
        for packet in packets {
            if packet.first() == Some(&b'6') {
                if self.challenge_seen || self.config.handshake == Handshake::Direct {
                    return Err("Unexpected repeated login challenge".into());
                }
                self.challenge_seen = true;
                self.send_raw(&challenge::solve_challenge(&packet[1..])?)?;
                self.send_login()?;
            }
        }
        Ok(())
    }

    /// Poll the already received view, without reading the socket or changing receipts.
    pub fn frame(&self) -> Result<ClientFrame, String> {
        self.state.frame(self.now())
    }

    pub(super) fn check_send(&self, action: Action) -> Result<(), String> {
        if self.state.status() != ClientStatus::Playing {
            return Err("Cannot send an action before a live self appearance".into());
        }
        if !action.heading.is_finite() {
            return Err("Action heading must be finite".into());
        }
        Ok(())
    }

    pub fn drain_records(&mut self) -> Vec<WireRecord> {
        self.record_bytes = 0;
        std::mem::take(&mut self.records)
    }

    pub fn set_processing_seconds(&mut self, seconds: f64) -> Result<(), String> {
        self.state.set_processing_seconds(seconds)
    }

    pub fn disconnect(&mut self) {
        if let Some(mut socket) = self.socket.take() {
            // Dropping the socket bounds cleanup even if a peer stops reading.
            let _ = socket.close(None);
            let now = self.now();
            let _ = self.record(now, WireEvent::Disconnect, &[]);
        }
        self.state.disconnect();
    }

    fn read_until_decision(&mut self) -> Result<ClientFrame, String> {
        if self.socket.is_none() {
            return self.frame();
        }
        let deadline = self.next_decision.max(self.now() + 0.001);
        loop {
            let now = self.now();
            if now - self.last_received >= self.config.idle_timeout_seconds {
                return Err("Server receive timeout".into());
            }
            if self.state.ping_due(now)? {
                self.send_raw(&[251])?;
            }
            if now >= deadline {
                break;
            }
            let read_until = deadline.min(self.last_received + self.config.idle_timeout_seconds);
            let socket = self.socket.as_mut().ok_or("Disconnected")?;
            let transport = transport_stream(socket.get_mut())?;
            transport.read_deadline = self.epoch + Duration::from_secs_f64(read_until);
            transport.write_deadline =
                Instant::now() + Duration::from_secs_f64(self.config.connect_timeout_seconds);
            match socket.read() {
                Ok(Message::Binary(bytes)) => {
                    let at = self.now();
                    self.last_received = at;
                    self.receive_binary(&bytes, at)?;
                }
                Ok(Message::Ping(_) | Message::Pong(_)) => {
                    self.socket
                        .as_mut()
                        .ok_or("Disconnected")?
                        .flush()
                        .map_err(|e| e.to_string())?;
                }
                Ok(Message::Close(_))
                | Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    self.disconnect();
                    break;
                }
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                Ok(_) => return Err("Expected binary game messages".into()),
                Err(error) => return Err(error.to_string()),
            }
        }
        self.next_decision = self.now() + self.config.decision_interval_seconds;
        self.frame()
    }
}

impl Backend for NetworkBackend {
    fn next_frame(&mut self) -> Result<ClientFrame, String> {
        let result = self.read_until_decision();
        if result.is_err() {
            self.disconnect();
        }
        result
    }

    fn send_action(&mut self, action: Action) -> Result<(), String> {
        self.check_send(action)?;
        let packets = action_packets(action, self.boost)?;
        for packet in packets {
            if let Err(error) = self.send_raw(&packet) {
                self.disconnect();
                return Err(error);
            }
        }
        self.boost = Some(action.boost);
        Ok(())
    }

    fn reconnect(&mut self) -> Result<(), String> {
        self.disconnect();
        if let Err(error) = self.open() {
            self.disconnect();
            return Err(error);
        }
        Ok(())
    }

    fn set_processing_seconds(&mut self, seconds: f64) -> Result<(), String> {
        self.set_processing_seconds(seconds)
    }
}

impl Drop for NetworkBackend {
    fn drop(&mut self) {
        self.disconnect();
    }
}

fn transport_stream(
    stream: &mut MaybeTlsStream<DeadlineStream>,
) -> Result<&mut DeadlineStream, String> {
    match stream {
        MaybeTlsStream::Plain(stream) => Ok(stream),
        MaybeTlsStream::Rustls(stream) => Ok(&mut stream.sock),
        _ => Err("Unsupported TLS stream".into()),
    }
}

/// Bound the complete operation, including repeated reads while a WebSocket or
/// TLS message is partial. A per-read socket timeout alone can be extended
/// forever by a peer that keeps trickling bytes.
#[derive(Debug)]
struct DeadlineStream {
    socket: TcpStream,
    read_deadline: Instant,
    write_deadline: Instant,
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "Operation deadline reached"))
}

impl Read for DeadlineStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.socket
            .set_read_timeout(Some(remaining(self.read_deadline)?))?;
        self.socket.read(bytes)
    }
}

impl Write for DeadlineStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.socket
            .set_write_timeout(Some(remaining(self.write_deadline)?))?;
        self.socket.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}
