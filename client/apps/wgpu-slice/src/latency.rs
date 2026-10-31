//! Passive CPU observations; never feed these clocks back into policy.
//! Schema and boundary limits: [latency stages](../../../docs/latency-trace.md).
use std::{
    cell::RefCell,
    io::Write,
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

static EPOCH: OnceLock<(Instant, u128)> = OnceLock::new();
const CAPACITY: usize = 262_144;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct Record {
    pub ns: u64,
    pub stage: &'static str,
    pub generation: u32,
    pub id: u64,
    pub related: u64,
    pub detail: u64,
}
#[derive(Default)]
struct State {
    buffer: Option<Buffer>,
    generation: u32,
    packet: u64,
    inputs: [u64; 3],
    issued: u64,
    poll: u64,
    acquire: u64,
    acquire_generation: u32,
    callback: u64,
    callback_admitted: bool,
    input_pending: [bool; 3],
    writing: u64,
    pending_writes: Vec<u64>,
}
struct Buffer {
    rows: Vec<Record>,
    dropped: u64,
    shared: Shared,
    slot: usize,
}
type Shared = Arc<Mutex<Vec<(Vec<Record>, u64, bool)>>>;
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

/// Each participating thread allocates once, before observing any hot path.
#[derive(Clone)]
pub struct Recorder {
    shared: Shared,
    capacity: usize,
}
impl Recorder {
    pub fn new(capacity: usize) -> Self {
        EPOCH.get_or_init(|| {
            (
                Instant::now(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock after epoch")
                    .as_nanos(),
            )
        });
        Self {
            shared: Arc::default(),
            capacity,
        }
    }
    pub fn attach(&self) -> ThreadBuffer {
        let slot = {
            let mut buffers = self.shared.lock().expect("buffers");
            let slot = buffers.len();
            buffers.push((Vec::new(), 0, false));
            slot
        };
        // Commit/touch the reserved pages before the first hot-path append. Merely
        // reserving address space can defer page faults into measured frames.
        let mut rows = vec![
            Record {
                ns: 0,
                stage: "",
                generation: 0,
                id: 0,
                related: 0,
                detail: 0
            };
            self.capacity
        ];
        std::hint::black_box(&mut rows);
        rows.clear();
        STATE.with_borrow_mut(|s| {
            assert!(s.buffer.is_none(), "latency recorder already attached");
            *s = State {
                buffer: Some(Buffer {
                    rows,
                    dropped: 0,
                    shared: self.shared.clone(),
                    slot,
                }),
                pending_writes: Vec::with_capacity(32),
                ..State::default()
            };
        });
        ThreadBuffer(std::marker::PhantomData)
    }
    pub fn snapshot(&self) -> (Vec<Record>, u64) {
        let buffers = self.shared.lock().expect("latency buffers");
        (
            buffers.iter().flat_map(|b| b.0.clone()).collect(),
            buffers.iter().map(|b| b.1).sum(),
        )
    }
    /// Only after all thread guards have retired. No serialization on hot paths.
    pub fn write(&self, path: &Path, revision: &str, completed: bool) -> std::io::Result<()> {
        let buffers = self.shared.lock().expect("buffers");
        let count: usize = buffers.iter().map(|b| b.0.len()).sum();
        let dropped: u64 = buffers.iter().map(|b| b.1).sum();
        let retired = buffers.iter().all(|b| b.2);
        let mut out = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?,
        );
        // Revision is supplied by the opt-in launcher; reject strings requiring JSON escaping.
        if revision.len() < 8 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(std::io::Error::other("expected source revision hex"));
        }
        writeln!(
            out,
            "{{\"format\":\"latency-trace-v1\",\"run\":\"{}-{}\",\"clockDomain\":\"process-instant-{}-{}\",\"sourceRevision\":\"{}\",\"complete\":{},\"dropped\":{},\"threads\":{}}}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos(),
            std::process::id(),
            EPOCH.get().expect("epoch").1,
            revision,
            completed && retired && dropped == 0,
            dropped,
            buffers.len()
        )?;
        for (thread, (buffer, _, _)) in buffers.iter().enumerate() {
            for (seq, r) in buffer.iter().enumerate() {
                writeln!(
                    out,
                    "{{\"thread\":{thread},\"event\":{seq},\"ns\":\"{}\",\"stage\":\"{}\",\"generation\":{},\"id\":\"{}\",\"related\":\"{}\",\"detail\":\"{}\"}}",
                    r.ns, r.stage, r.generation, r.id, r.related, r.detail
                )?;
            }
        }
        writeln!(out, "{{\"end\":true,\"records\":{}}}", count)?;
        out.flush()
    }
}
pub struct ThreadBuffer(std::marker::PhantomData<std::rc::Rc<()>>);
impl Drop for ThreadBuffer {
    fn drop(&mut self) {
        STATE.with_borrow_mut(|s| {
            push(s, "thread_end", 0, 0, 0);
            if let Some(b) = s.buffer.take() {
                b.shared.lock().expect("latency buffers")[b.slot] = (b.rows, b.dropped, true);
            }
        });
    }
}
pub fn worker() -> Option<Recorder> {
    STATE.with_borrow(|s| {
        s.buffer.as_ref().map(|b| Recorder {
            shared: b.shared.clone(),
            capacity: b.rows.capacity(),
        })
    })
}
fn push(s: &mut State, stage: &'static str, id: u64, related: u64, detail: u64) {
    if let Some(b) = &mut s.buffer {
        if b.rows.len() == b.rows.capacity() {
            b.dropped += 1;
            return;
        }
        b.rows.push(Record {
            ns: EPOCH
                .get()
                .expect("epoch")
                .0
                .elapsed()
                .as_nanos()
                .min(u64::MAX as u128) as u64,
            stage,
            generation: s.generation,
            id,
            related,
            detail,
        });
    }
}
pub fn event(stage: &'static str, id: u64, related: u64, detail: u64) {
    STATE.with_borrow_mut(|s| push(s, stage, id, related, detail));
}
pub fn next() -> u64 {
    STATE.with_borrow_mut(|s| {
        if s.buffer.is_none() {
            return 0;
        }
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    })
}
pub fn generation(g: u32) {
    STATE.with_borrow_mut(|s| s.generation = g);
}
pub fn connection(g: u32) {
    STATE.with_borrow_mut(|s| {
        s.generation = g;
        s.packet = 0;
        // Pointer and held turning keys survive DesktopInput::reset. Keep their
        // process-unique source IDs across connection generations as well.
        s.inputs[2] = 0;
        s.input_pending[2] = false;
    });
}
pub fn current_generation() -> u32 {
    STATE.with_borrow(|s| s.generation)
}
pub fn stamp() -> u64 {
    STATE.with_borrow(|s| {
        if s.buffer.is_some() {
            EPOCH.get().expect("epoch").0.elapsed().as_nanos() as u64
        } else {
            0
        }
    })
}
pub struct PictureSpan(&'static str);
impl PictureSpan {
    pub fn new(begin: &'static str, end: &'static str) -> Self {
        picture(begin);
        Self(end)
    }
}
impl Drop for PictureSpan {
    fn drop(&mut self) {
        picture(self.0);
    }
}
pub fn writing(id: u64) {
    STATE.with_borrow_mut(|s| s.writing = id);
}
pub fn write_completed(count: usize) {
    STATE.with_borrow_mut(|s| push(s, "write", s.writing, s.poll, count as u64));
}
pub fn write_blocked() {
    STATE.with_borrow_mut(|s| push(s, "write_blocked", s.writing, s.poll, 0));
}
pub fn accepted(id: u64) {
    STATE.with_borrow_mut(|s| {
        if s.buffer.is_none() {
            return;
        }
        push(s, "accepted", id, 0, 0);
        if s.pending_writes.len() < s.pending_writes.capacity() {
            s.pending_writes.push(id);
        } else if let Some(b) = &mut s.buffer {
            b.dropped += 1;
        }
    });
}
pub fn flushed() {
    STATE.with_borrow_mut(|s| {
        for index in 0..s.pending_writes.len() {
            let id = s.pending_writes[index];
            push(s, "flushed", id, 0, 0);
        }
        s.pending_writes.clear();
    });
}
pub fn abandoned() {
    STATE.with_borrow_mut(|s| {
        for index in 0..s.pending_writes.len() {
            let id = s.pending_writes[index];
            push(s, "send_abandoned", id, 0, 0);
        }
        s.pending_writes.clear();
    });
}
pub fn acquire_begin() -> u64 {
    let id = next();
    STATE.with_borrow_mut(|s| {
        s.acquire = id;
        s.acquire_generation = s.generation;
        push(s, "acquire_begin", id, 0, 0);
    });
    id
}
pub fn last_ingress() {
    STATE.with_borrow_mut(|s| {
        push(
            s,
            "last_ingress",
            s.acquire,
            s.packet,
            u64::from(s.acquire_generation),
        )
    });
}
pub fn poll() -> u64 {
    let id = next();
    STATE.with_borrow_mut(|s| s.poll = id);
    event("poll", id, 0, 0);
    id
}
pub fn raw_read(count: usize, handshake: bool) -> u64 {
    let id = next();
    STATE.with_borrow_mut(|s| {
        push(
            s,
            if handshake { "handshake_read" } else { "read" },
            id,
            s.poll,
            count as u64,
        )
    });
    id
}
pub fn message() -> u64 {
    message_reads(0, 0)
}
pub fn message_reads(first: u64, last: u64) -> u64 {
    let id = next();
    event("message", id, first, last);
    id
}
pub fn applying(id: u64) {
    STATE.with_borrow_mut(|s| {
        s.packet = id;
        push(s, "apply_begin", id, 0, 0);
    });
}
pub fn build(stage: &'static str, picture: u64) {
    STATE.with_borrow_mut(|s| push(s, stage, picture, s.packet, 0));
}
pub fn issue(picture: u64) {
    STATE.with_borrow_mut(|s| {
        s.issued = picture;
        push(
            s,
            "issued",
            picture,
            s.acquire,
            u64::from(s.acquire_generation),
        );
    });
}
pub fn submit_begin() -> u64 {
    let id = next();
    STATE.with_borrow_mut(|s| push(s, "submit_begin", s.issued, id, 0));
    id
}
pub fn submit_end(id: u64) {
    STATE.with_borrow_mut(|s| push(s, "submit_end", s.issued, id, 0));
}
pub fn picture(stage: &'static str) {
    STATE.with_borrow_mut(|s| push(s, stage, s.issued, 0, 0));
}
pub fn callback() -> Input {
    let id = next();
    STATE.with_borrow_mut(|s| {
        s.callback = id;
        s.callback_admitted = false;
    });
    event("callback", id, 0, 0);
    Input(id)
}
pub struct Input(u64);
impl Drop for Input {
    fn drop(&mut self) {
        STATE.with_borrow_mut(|s| {
            if self.0 != 0 && !s.callback_admitted {
                push(s, "suppressed", self.0, 0, 0);
            }
            s.callback = 0;
        });
    }
}
/// Channels are pointer (0), held turning keys (1), and boost (2).
pub fn admitted(channel: usize) {
    STATE.with_borrow_mut(|s| {
        if s.buffer.is_none() {
            return;
        }
        let id = if s.callback == 0 {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            push(s, "callback", id, 0, 0);
            id
        } else {
            s.callback
        };
        if s.input_pending[channel] && s.inputs[channel] != id {
            push(s, "coalesced", s.inputs[channel], id, channel as u64);
        }
        s.inputs[channel] = id;
        s.input_pending[channel] = true;
        if !s.callback_admitted || s.callback == 0 {
            push(s, "admitted", id, 0, 0);
        }
        s.callback_admitted = true;
    });
}
pub fn decision(effect: u64, phase: u64) {
    STATE.with_borrow_mut(|s| {
        let channel = match phase {
            0 => Some(1),
            2 => Some(2),
            3 => Some(0),
            _ => None,
        };
        let input = channel.map_or(0, |c| s.inputs[c]);
        push(s, "decision", effect, input, phase);
        if let Some(c) = channel {
            s.input_pending[c] = false;
        }
        if phase == 3 && s.inputs[1] != 0 {
            push(s, "decision_input", effect, s.inputs[1], 1);
            s.input_pending[1] = false;
        }
    });
}

/// Explicit opt-in, with a pinned source identity. The guard is owned by the finite host run.
pub struct Run {
    recorder: Recorder,
    thread: Option<ThreadBuffer>,
    path: std::path::PathBuf,
    revision: String,
    finished: bool,
}
impl Run {
    pub fn from_env() -> Result<Option<Self>, String> {
        let Some(path) = std::env::var_os("SLITHER_LATENCY_TRACE") else {
            return Ok(None);
        };
        let revision = std::env::var("SLITHER_LATENCY_REVISION")
            .map_err(|_| "SLITHER_LATENCY_REVISION is required")?;
        if revision.len() < 8 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid latency source revision".into());
        }
        if Path::new(&path).exists() {
            return Err("Latency output already exists".into());
        }
        let recorder = Recorder::new(CAPACITY);
        let thread = Some(recorder.attach());
        Ok(Some(Self {
            recorder,
            thread,
            path: path.into(),
            revision,
            finished: false,
        }))
    }
    pub fn finish(mut self, complete: bool) -> Result<(), String> {
        self.thread.take();
        self.finished = true;
        self.recorder
            .write(&self.path, &self.revision, complete)
            .map_err(|e| e.to_string())
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if !self.finished {
            self.thread.take();
            if let Err(error) = self.recorder.write(&self.path, &self.revision, false) {
                eprintln!("Latency trace finalization failed: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_full_records_without_subsampling() {
        let r = Recorder::new(2);
        {
            let _b = r.attach();
            for id in 0..5 {
                event("test", id, 0, 0);
            }
        }
        let (rows, dropped) = r.snapshot();
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(dropped, 4);
    }
}
