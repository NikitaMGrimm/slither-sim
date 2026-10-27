//! Fixed-work diagnostic: 500 idle TCP sockets, 4,000 actual gateway passes.
use super::*;
fn shared(dt: f64, sockets: usize) -> Arc<Shared> {
    Arc::new(Shared {
        clock: Clock { epoch: Instant::now(), dt, deferred: AtomicU64::new(0) },
        stop: AtomicBool::new(false), ticks: AtomicU64::new(0), busy: AtomicU64::new(0),
        players: AtomicUsize::new(0), pending: (0..sockets).map(|_| AtomicUsize::new(0)).collect(),
        failure: Mutex::new(None),
    })
}
pub fn run() {
    let n = 500;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (commands, _inbox) = mpsc::channel();
    let (_route, deliveries) = mpsc::channel();
    let mut gateway = Gateway {
        listener,
        sessions: Vec::new(),
        /* READINESS */
        gate: Gate {
            occupied: vec![true; n], generations: vec![0; n],
            challenge: Vec::new(), replies: [Vec::new(), Vec::new()],
            commands, shared: shared(0.007, n),
        },
        deliveries,
    };
    let mut peers = Vec::new();
    for slot in 0..n {
        peers.push(TcpStream::connect(address).unwrap());
        let (stream, _) = gateway.listener.accept().unwrap();
        stream.set_nonblocking(true).unwrap();
        let socket = WebSocket::from_raw_socket(ReadBudget::new(stream), tungstenite::protocol::Role::Server, Some(socket_config()));
        let mut open = Open::new(socket, false);
        open.phase = Phase::Playing(slot);
        gateway.sessions.push(Session {
            accepted: Instant::now(), physical_food: Arc::new(AtomicBool::new(false)),
            stage: Stage::Open(Box::new(open)),
        });
    }
    gateway.listener.set_nonblocking(true).unwrap();
    gateway.poll().unwrap();
    crate::socket_study::active(true);
    for _ in 0..4000 { gateway.poll().unwrap(); }
    crate::socket_study::active(false);
    assert_eq!(gateway.sessions.len(), n);
    assert_eq!(gateway.gate.shared.players.load(Ordering::Relaxed), n);
    std::hint::black_box(peers);
    println!("{{\"sockets\":500,\"passes\":4000}}");
}
