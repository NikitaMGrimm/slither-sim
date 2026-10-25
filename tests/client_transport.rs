#![cfg(feature = "networking")]

use serde_json::{Value, json};
use slither_sim::client::{CONTRACT, ClientFrame, ClientStatus, WireEvent, WireRecord, transport};
use std::io::{BufRead, BufReader, Cursor, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

fn exchange_bytes(input: &[u8]) -> Vec<Value> {
    let mut output = Vec::new();
    transport::serve(Cursor::new(input), &mut output).unwrap();
    String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| {
            let reply: Value = serde_json::from_str(line).unwrap();
            // Replies keep the canonical bytes of a sorted-key `Value`.
            assert_eq!(serde_json::to_string(&reply).unwrap(), line);
            assert_eq!(reply["protocol"], transport::PROTOCOL);
            assert_ne!(reply.get("result").is_some(), reply.get("error").is_some());
            reply
        })
        .collect()
}

fn exchange(requests: &[Value]) -> Vec<Value> {
    let mut input = String::new();
    for request in requests {
        input.push_str(&serde_json::to_string(request).unwrap());
        input.push('\n');
    }
    exchange_bytes(input.as_bytes())
}

fn frame(reply: &Value) -> ClientFrame {
    assert!(reply.get("error").is_none(), "{reply}");
    let frame: ClientFrame = serde_json::from_value(reply["result"]["frame"].clone()).unwrap();
    assert_eq!(frame.contract, CONTRACT);
    frame
}

/// A loopback port with no listener, so connection attempts fail locally.
fn closed_loopback_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("ws://127.0.0.1:{port}/slither")
}

#[test]
fn info_works_without_a_backend() {
    let replies = exchange(&[json!({"op":"info"})]);
    let result = &replies[0]["result"];
    assert_eq!(result["contract"], CONTRACT);
    assert_eq!(result["wireProtocol"], 19);
    assert_eq!(result["stability"], "experimental");
    assert_eq!(result["backend"], Value::Null);
    assert_eq!(result["defaultSeed"], 1);
    assert!(result["defaultSimulationConfig"].is_object());
    assert!(result["defaultNetworkConfig"].is_object());
}

#[test]
fn current_json_contract_rejects_removed_request_fields() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/client-contract-v2.json")).unwrap();
    assert_eq!(fixture["protocol"], transport::PROTOCOL);
    assert_eq!(fixture["frame"]["contract"], CONTRACT);
    let requests = fixture["rejectedRequests"].as_array().unwrap();
    for (request, reply) in requests.iter().zip(exchange(requests)) {
        assert_eq!(reply["protocol"], "slither-client-json-v2");
        assert!(
            reply["error"].as_str().unwrap().contains("unknown field"),
            "{request}: {reply}"
        );
    }
}

#[test]
fn every_line_gets_one_reply_and_errors_do_not_stop_the_stream() {
    let input = b"not json\n\
        {\"op\":\"bogus\"}\n\
        {\"op\":\"info\",\"extra\":1}\n\
        \n\
        \xff\xfe\n\
        {\"op\":\"step\"}\n\
        {\"op\":\"observe\"}\n\
        {\"op\":\"reconnect\"}\n\
        {\"op\":\"wire\"}\n\
        {\"op\":\"network\"}\r\n\
        {\"op\":\"info\"}\r\n";
    let replies = exchange_bytes(input);
    assert_eq!(replies.len(), 11);
    for reply in &replies[..10] {
        assert!(reply["error"].is_string(), "{reply}");
    }
    assert_eq!(replies[10]["result"]["contract"], CONTRACT);
}

#[test]
fn only_step_advances_and_observe_repeats_the_current_frame() {
    let replies = exchange(&[
        json!({"op":"simulation","seed":7}),
        json!({"op":"observe"}),
        json!({"op":"observe"}),
        json!({"op":"step"}),
        json!({"op":"observe"}),
        json!({"op":"info"}),
    ]);
    assert_eq!(replies[0]["result"]["backend"], "simulation");
    let started = frame(&replies[0]);
    assert_eq!(frame(&replies[1]), started);
    assert_eq!(frame(&replies[2]), started);
    let stepped = frame(&replies[3]);
    assert!(stepped.time > started.time);
    assert_eq!(frame(&replies[4]), stepped);
    assert_eq!(replies[5]["result"]["backend"], "simulation");
}

#[test]
fn identical_simulation_requests_are_deterministic() {
    // Early actions may be rejected before play; rejections must replay identically too.
    let mut requests = vec![json!({"op":"simulation","seed":3})];
    for step in 0..40 {
        requests.push(json!({"op":"step"}));
        requests.push(json!({"op":"step","action":{"heading":step as f64 * 0.1,"boost":false}}));
    }
    let first = exchange(&requests);
    assert!(first.iter().any(|reply| reply.get("result").is_some()));
    assert_eq!(first, exchange(&requests));
}

#[test]
fn rejected_steps_leave_the_backend_unchanged() {
    let replies = exchange(&[
        json!({"op":"simulation","seed":7}),
        json!({"op":"step","action":{"heading":0,"boost":false},"processingSeconds":-0.01}),
        json!({"op":"step","processingSeconds":-0.01}),
        json!({"op":"step","action":{"heading":0,"boost":false,"extra":1}}),
        json!({"op":"step","action":{"heading":0}}),
        json!({"op":"step","action":{"heading":"north","boost":false}}),
        json!({"op":"wire"}),
        json!({"op":"observe"}),
        json!({"op":"step","action":null}),
    ]);
    let started = frame(&replies[0]);
    for reply in &replies[1..7] {
        assert!(reply["error"].is_string(), "{reply}");
    }
    assert_eq!(frame(&replies[7]), started);
    assert!(frame(&replies[8]).time > started.time);
}

#[test]
fn rejected_action_does_not_record_processing_or_delay_a_later_step() {
    let replies = exchange(&[
        json!({"op":"simulation","seed":7,"config":{"latency":{"roundTripSeconds":[0.2]}}}),
        json!({"op":"step","action":{"heading":0,"boost":false},"processingSeconds":0.12}),
        json!({"op":"observe"}),
        json!({"op":"step"}),
    ]);
    let before = frame(&replies[0]);
    assert_eq!(before.status, ClientStatus::Waiting);
    assert!(replies[1]["error"].is_string(), "{:?}", replies[1]);
    assert_eq!(frame(&replies[2]), before);
    assert!(frame(&replies[3]).time - before.time < 0.05);
}

#[test]
fn measured_processing_is_accepted_with_an_action_in_play() {
    let controller = Controller::start();
    let mut current = frame(&controller.request(json!({"op":"simulation","seed":7})));
    for _ in 0..200 {
        if current.status == ClientStatus::Playing {
            break;
        }
        current = frame(&controller.request(json!({"op":"step"})));
    }
    assert_eq!(current.status, ClientStatus::Playing);
    for _ in 0..10 {
        let reply = controller.request(json!({
            "op": "step",
            "action": {"heading": 1, "boost": false},
            "processingSeconds": 0.003,
        }));
        let next = frame(&reply);
        assert!(next.time > current.time);
        current = next;
    }
}

#[test]
fn measured_processing_advances_time_when_the_bot_holds_its_action() {
    let replies = exchange(&[
        json!({"op":"simulation","seed":7}),
        json!({"op":"step"}),
        json!({"op":"step","action":null,"processingSeconds":0.12}),
        json!({"op":"step"}),
    ]);
    let before = frame(&replies[1]);
    let completed = frame(&replies[2]);
    let next = frame(&replies[3]);
    assert!(completed.time - before.time >= 0.12);
    assert_eq!(completed.timing.processing_seconds, 0.12);
    assert!(completed.timing.sent_commands.is_empty());
    assert!(
        next.time - completed.time < 0.05,
        "one measured duration must not delay later decisions"
    );
}

#[test]
fn simulation_wire_records_drain_once() {
    let replies = exchange(&[
        json!({"op":"simulation","seed":7,"config":{"recordWire":true}}),
        json!({"op":"step"}),
        json!({"op":"step"}),
        json!({"op":"wire"}),
        json!({"op":"wire"}),
        json!({"op":"observe"}),
    ]);
    let records: Vec<WireRecord> =
        serde_json::from_value(replies[3]["result"]["records"].clone()).unwrap();
    assert_eq!(
        records.first().map(|record| record.event),
        Some(WireEvent::Connect)
    );
    assert!(records.windows(2).all(|pair| pair[0].time <= pair[1].time));
    assert_eq!(replies[4]["result"]["records"], json!([]));
    assert_eq!(frame(&replies[5]), frame(&replies[2]));
}

#[test]
fn failed_replacement_keeps_the_previous_backend() {
    let replies = exchange(&[
        json!({"op":"simulation","seed":7}),
        json!({"op":"step"}),
        json!({"op":"simulation","seed":8,"config":{"notAField":1}}),
        json!({"op":"simulation","seed":-1}),
        json!({"op":"simulation","seed":8,"config":{"agents":0}}),
        json!({"op":"network","config":{"url":"http://127.0.0.1:9/slither"}}),
        json!({"op":"network","config":{"url":closed_loopback_url(),"connectTimeoutSeconds":1}}),
        json!({"op":"network","config":{"url":"ws://127.0.0.1:9/slither","unknown":true}}),
        json!({"op":"observe"}),
    ]);
    let stepped = frame(&replies[1]);
    for reply in &replies[2..8] {
        assert!(reply["error"].is_string(), "{reply}");
    }
    assert_eq!(replies[8]["result"]["backend"], "simulation");
    assert_eq!(frame(&replies[8]), stepped);
}

#[test]
fn unencodable_prey_speeds_cannot_replace_a_working_backend() {
    let replies = exchange(&[
        json!({"op":"simulation","seed":7}),
        json!({"op":"step"}),
        json!({"op":"simulation","seed":8,"config":{"world":{
            "vision":2000,"prey":{"count":1,"cruiseSpeed":2200,"escapeSpeed":2200,"sprintSpeed":2200}
        }}}),
        json!({"op":"observe"}),
        json!({"op":"step"}),
    ]);
    assert!(replies[2]["error"].is_string(), "{}", replies[2]);
    assert_eq!(frame(&replies[3]), frame(&replies[1]));
    assert!(frame(&replies[4]).time > frame(&replies[3]).time);
}

/// Accept loopback WebSocket connections, read the direct login, then close.
fn closing_server(connections: usize) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("ws://{}/slither", listener.local_addr().unwrap());
    let accepted = Arc::new(AtomicUsize::new(0));
    let counter = accepted.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(connections) {
            let Ok(stream) = stream else { continue };
            counter.fetch_add(1, Ordering::SeqCst);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let Ok(mut socket) = tungstenite::accept(stream) else {
                continue;
            };
            let _ = socket.read();
            let _ = socket.close(None);
            // Complete the closing handshake once the client acknowledges it.
            while socket.read().is_ok() {}
        }
    });
    (url, accepted)
}

/// Lines delivered to a live serve loop one request at a time.
struct ChannelInput {
    lines: mpsc::Receiver<Vec<u8>>,
    current: Cursor<Vec<u8>>,
}

impl std::io::Read for ChannelInput {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let available = self.fill_buf()?;
        let count = available.len().min(buffer.len());
        buffer[..count].copy_from_slice(&available[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl BufRead for ChannelInput {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.current.position() as usize == self.current.get_ref().len() {
            // A closed channel is EOF.
            self.current = Cursor::new(self.lines.recv().unwrap_or_default());
        }
        let position = self.current.position() as usize;
        Ok(&self.current.get_ref()[position..])
    }

    fn consume(&mut self, count: usize) {
        self.current
            .set_position(self.current.position() + count as u64);
    }
}

/// Collects one reply per flush, which serve performs after every reply line.
struct ChannelOutput {
    replies: mpsc::Sender<Vec<u8>>,
    pending: Vec<u8>,
}

impl Write for ChannelOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.pending.is_empty() {
            let _ = self.replies.send(std::mem::take(&mut self.pending));
        }
        Ok(())
    }
}

struct Controller {
    requests: mpsc::Sender<Vec<u8>>,
    replies: mpsc::Receiver<Vec<u8>>,
}

impl Controller {
    fn start() -> Self {
        let (requests, lines) = mpsc::channel();
        let (sender, replies) = mpsc::channel();
        thread::spawn(move || {
            let input = ChannelInput {
                lines,
                current: Cursor::new(Vec::new()),
            };
            let output = ChannelOutput {
                replies: sender,
                pending: Vec::new(),
            };
            transport::serve(input, output).unwrap();
        });
        Self { requests, replies }
    }

    fn request(&self, request: Value) -> Value {
        let mut line = serde_json::to_vec(&request).unwrap();
        line.push(b'\n');
        self.requests.send(line).unwrap();
        let reply = self.replies.recv_timeout(Duration::from_secs(30)).unwrap();
        let mut lines = BufReader::new(&reply[..]).lines();
        let reply: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
        assert!(lines.next().is_none(), "one reply per request");
        assert_eq!(reply["protocol"], transport::PROTOCOL);
        reply
    }
}

#[test]
fn network_reads_only_on_step_and_reconnects_only_on_request() {
    let (url, accepted) = closing_server(2);
    let controller = Controller::start();
    let config = json!({
        "url": url,
        "handshake": "direct",
        "recordWire": true,
        "connectTimeoutSeconds": 2,
        "idleTimeoutSeconds": 2,
    });
    let started = controller.request(json!({"op":"network","config":config}));
    assert_eq!(started["result"]["backend"], "network");
    let first = frame(&started);
    assert_eq!(accepted.load(Ordering::SeqCst), 1);
    // Network frames are stamped at poll time, so compare identity, not bytes.
    let polled = frame(&controller.request(json!({"op":"observe"})));
    assert_eq!(polled.status, first.status);
    assert_eq!(
        polled.received.connection_generation,
        first.received.connection_generation
    );

    // The server has closed; stepping may report the loss but must not redial.
    for _ in 0..5 {
        let reply = controller.request(json!({"op":"step"}));
        if reply.get("result").is_some() {
            frame(&reply);
        }
    }
    assert_eq!(accepted.load(Ordering::SeqCst), 1);
    let disconnected = controller.request(json!({"op":"observe"}));
    assert_eq!(frame(&disconnected).status, ClientStatus::Disconnected);

    let records: Vec<WireRecord> = serde_json::from_value(
        controller.request(json!({"op":"wire"}))["result"]["records"].clone(),
    )
    .unwrap();
    let events: Vec<_> = records.iter().map(|record| record.event).collect();
    assert_eq!(events.first(), Some(&WireEvent::Connect));
    assert!(
        events.contains(&WireEvent::Send),
        "the direct login is recorded"
    );
    assert_eq!(events.last(), Some(&WireEvent::Disconnect));
    assert!(records.windows(2).all(|pair| pair[0].time <= pair[1].time));
    let drained = controller.request(json!({"op":"wire"}));
    assert_eq!(drained["result"]["records"], json!([]));

    let reconnected = frame(&controller.request(json!({"op":"reconnect"})));
    assert_eq!(accepted.load(Ordering::SeqCst), 2);
    assert!(reconnected.received.connection_generation > first.received.connection_generation);
}

#[test]
fn wire_distinguishes_disabled_recording_from_an_empty_drain_on_both_backends() {
    for record in [false, true] {
        let (url, _) = closing_server(1);
        for request in [
            json!({"op":"simulation","config":{"recordWire":record}}),
            json!({"op":"network","config":{"url":url,"handshake":"direct","recordWire":record}}),
        ] {
            let replies = exchange(&[request, json!({"op":"wire"}), json!({"op":"wire"})]);
            assert!(replies[0].get("error").is_none(), "{}", replies[0]);
            if record {
                assert!(replies[1]["result"]["records"].is_array());
                assert_eq!(replies[2]["result"]["records"], json!([]));
            } else {
                for reply in &replies[1..] {
                    assert_eq!(reply["error"], "Wire recording is disabled");
                }
            }
        }
    }
}
