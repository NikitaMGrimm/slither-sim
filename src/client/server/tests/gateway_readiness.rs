//! Transport/control progress must not depend on a fresh readiness edge.
use super::*;

#[derive(Default)]
struct Transport {
    output: Vec<u8>,
    writes: VecDeque<usize>,
    reads: usize,
}

impl io::Read for Transport {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        Err(io::ErrorKind::WouldBlock.into())
    }
}

impl io::Write for Transport {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let limit = self.writes.pop_front().unwrap_or(usize::MAX);
        if limit == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let n = bytes.len().min(limit);
        self.output.extend_from_slice(&bytes[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Match TcpStream: an empty transport flush has no wire effect.
        Ok(())
    }
}

fn socket(transport: Transport, buffered: Vec<u8>) -> WebSocket<ReadBudget<Transport>> {
    WebSocket::from_partially_read(
        ReadBudget::new(transport),
        buffered,
        tungstenite::protocol::Role::Server,
        Some(socket_config()),
    )
}

#[test]
fn buffered_ping_replies_without_a_new_readiness_edge() {
    let (mut gateway, _simulation) = pit_gateway();
    // A masked Ping already consumed by the HTTP upgrade's transport read.
    let mut open = Open::new(
        socket(Transport::default(), vec![0x89, 0x81, 0, 0, 0, 0, 7]),
        false,
    );
    open.phase = Phase::Playing(0);
    let now = Instant::now();
    assert!(
        open.service(&mut gateway.gate, now, 0., Duration::ZERO, false)
            .unwrap()
    );
    assert!(open.flush(&mut gateway.gate));
    assert_eq!(open.socket.get_ref().inner.output, [0x8a, 1, 7]);
}

#[test]
fn buffered_peer_close_flushes_its_reply_and_releases_the_slot() {
    let (mut gateway, _simulation) = pit_gateway();
    gateway.gate.occupied[0] = true;
    let mut open = Open::new(
        socket(Transport::default(), vec![0x88, 0x80, 0, 0, 0, 0]),
        false,
    );
    open.phase = Phase::Playing(0);
    let now = Instant::now();
    assert!(
        open.service(&mut gateway.gate, now, 0., Duration::ZERO, false)
            .unwrap()
    );
    assert!(!gateway.gate.occupied[0]);
    assert!(!open.flush(&mut gateway.gate));
    assert_eq!(open.socket.get_ref().inner.output, [0x88, 0]);
}

#[test]
fn partial_writes_retry_after_would_block_without_readability() {
    let (mut gateway, _simulation) = pit_gateway();
    let transport = Transport {
        writes: VecDeque::from([2, 0, usize::MAX]),
        ..Default::default()
    };
    let mut open = Open::new(socket(transport, Vec::new()), false);
    open.phase = Phase::Playing(0);
    let now = Instant::now();
    open.service(&mut gateway.gate, now, 0., Duration::ZERO, true)
        .unwrap();
    let reads = open.socket.get_ref().inner.reads;
    assert!(open.socket.get_ref().read_blocked);
    open.send(vec![0, 7, b'p']).unwrap();
    assert!(open.flush(&mut gateway.gate));
    assert_eq!(open.socket.get_ref().inner.output, [0x82, 3]);
    open.socket.get_mut().rearm();
    open.service(&mut gateway.gate, now, 0., Duration::ZERO, false)
        .unwrap();
    assert_eq!(open.socket.get_ref().inner.reads, reads);
    assert!(open.flush(&mut gateway.gate));
    assert_eq!(open.socket.get_ref().inner.output, [0x82, 3, 0, 7, b'p']);
}

#[test]
fn a_new_open_preserves_output_already_buffered_by_its_websocket() {
    let (mut gateway, _simulation) = pit_gateway();
    let mut socket = socket(Transport::default(), Vec::new());
    socket
        .write(Message::Binary(vec![0, 7, b'p'].into()))
        .unwrap();
    assert!(socket.get_ref().inner.output.is_empty());
    let mut open = Open::new(socket, false);
    assert!(open.flush(&mut gateway.gate));
    assert_eq!(open.socket.get_ref().inner.output, [0x82, 3, 0, 7, b'p']);
}
