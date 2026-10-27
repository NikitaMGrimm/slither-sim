//! Research-only: replay the fixed world's output through 500 actual sockets.
use super::*;

pub fn run(case: &str) {
    let n = 500;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (commands, _inbox) = mpsc::channel();
    let (_route, deliveries) = mpsc::channel();
    let mut gateway = Gateway {
        listener,
        sessions: Vec::new(),
        /* INDEX */
        readiness: readiness::Readiness::default(),
        gate: Gate {
            occupied: vec![true; n], generations: vec![0; n],
            challenge: Vec::new(), replies: [Vec::new(), Vec::new()], commands,
            shared: Arc::new(Shared {
                clock: Clock { epoch: Instant::now(), dt: 0.007, deferred: AtomicU64::new(0) },
                stop: AtomicBool::new(false), ticks: AtomicU64::new(0), busy: AtomicU64::new(0),
                players: AtomicUsize::new(0), pending: (0..n).map(|_| AtomicUsize::new(0)).collect(),
                failure: Mutex::new(None),
            }),
        },
        deliveries,
    };
    let mut clients = Vec::with_capacity(n);
    for slot in 0..n {
        let peer = TcpStream::connect(address).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let (stream, _) = gateway.listener.accept().unwrap();
        stream.set_nonblocking(true).unwrap();
        stream.set_nodelay(true).unwrap();
        let mut open = Open::new(WebSocket::from_raw_socket(
            ReadBudget::new(stream), tungstenite::protocol::Role::Server,
            Some(socket_config()),
        ), false);
        open.phase = Phase::Playing(slot);
        gateway.sessions.push(Session {
            accepted: Instant::now(), physical_food: Arc::new(AtomicBool::new(false)),
            stage: Stage::Open(Box::new(open)),
        });
        clients.push(WebSocket::from_raw_socket(peer, tungstenite::protocol::Role::Client, None));
    }
    gateway.listener.set_nonblocking(true).unwrap();
    gateway.poll().unwrap();
    let mut ticks = 0;
    let mut received = 0u64;
    crate::client::simulation::study_protocols::run_with_route(case, false, &mut |batches| {
        assert_eq!(batches.len(), n);
        let expected: Vec<Vec<Vec<u8>>> = batches.iter()
            .map(|batch| batch.iter().map(|(_, frame)| frame.clone()).collect()).collect();
        for (slot, frames) in batches.into_iter().enumerate() {
            if frames.is_empty() { continue; }
            // Remove real-time release jitter only in this replay. Payloads and
            // FIFO order remain exact; real-host holds exercise the scheduler.
            let frames = frames.into_iter().map(|(_, frame)| (0., frame)).collect();
            gateway.deliver(Delivery::Frames { slot, generation: 0, frames }, Instant::now());
        }
        gateway.poll().unwrap();
        assert_eq!(gateway.sessions.len(), n);
        for (client, frames) in clients.iter_mut().zip(expected) {
            for expected in frames {
                let Message::Binary(actual) = client.read().unwrap() else { panic!("expected binary") };
                assert_eq!(&actual[..], expected);
                received += 1;
            }
        }
        ticks += 1;
    });
    assert_eq!(ticks, 1080);
    assert!(received > 300_000);
}
