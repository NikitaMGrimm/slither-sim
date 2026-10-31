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
const RETAINED_RECORD_LIMIT: usize = 1_048_576;
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
    slot: Option<usize>,
}
#[derive(Default)]
struct RunBuffers {
    rows: Vec<Box<[Record]>>,
    retained: usize,
    dropped: u64,
    active: usize,
    limit: usize,
}
type Shared = Arc<Mutex<RunBuffers>>;
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

/// Each participating thread allocates once, before observing any hot path.
#[derive(Clone)]
pub struct Recorder {
    shared: Shared,
    capacity: usize,
}
impl Recorder {
    pub fn new(capacity: usize) -> Self {
        Self::with_limit(capacity, RETAINED_RECORD_LIMIT)
    }
    fn with_limit(capacity: usize, limit: usize) -> Self {
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
            shared: Arc::new(Mutex::new(RunBuffers {
                limit,
                ..RunBuffers::default()
            })),
            capacity,
        }
    }
    pub fn attach(&self) -> ThreadBuffer {
        let (slot, capacity) = {
            let mut buffers = self.shared.lock().expect("buffers");
            buffers.active += 1;
            let remaining = buffers.limit - buffers.retained;
            // Bound registration metadata too, including zero-capacity buffers.
            // Once full, count every attempted record without allocating another slot.
            if remaining != 0 && buffers.rows.len() < buffers.limit {
                let slot = buffers.rows.len();
                buffers.rows.push(Box::default());
                (Some(slot), self.capacity.min(remaining))
            } else {
                (None, 0)
            }
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
            capacity
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
            buffers
                .rows
                .iter()
                .flat_map(|b| b.iter().cloned())
                .collect(),
            buffers.dropped,
        )
    }
    /// Only after all thread guards have retired. No serialization on hot paths.
    pub fn write(&self, path: &Path, revision: &str, completed: bool) -> std::io::Result<()> {
        let buffers = self.shared.lock().expect("buffers");
        let count = buffers.retained;
        let dropped = buffers.dropped;
        let retired = buffers.active == 0;
        let mut out = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?,
        );
        // Revision is supplied by the opt-in launcher; reject strings requiring JSON escaping.
        if !(8..=64).contains(&revision.len()) || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
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
            buffers.rows.len()
        )?;
        for (thread, buffer) in buffers.rows.iter().enumerate() {
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
            if let Some(mut b) = s.buffer.take() {
                let mut buffers = b.shared.lock().expect("latency buffers");
                let keep = b.rows.len().min(buffers.limit - buffers.retained);
                buffers.dropped += b.dropped + (b.rows.len() - keep) as u64;
                b.rows.truncate(keep);
                if let Some(slot) = b.slot {
                    // Compaction and the run-wide budget are retirement work, never
                    // hot-path work. Retain a prefix, not a sample, and reject any loss.
                    buffers.rows[slot] = b.rows.into_boxed_slice();
                    buffers.retained += keep;
                }
                buffers.active -= 1;
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
    /// Invalid diagnostics configuration must never prevent the app from playing.
    pub fn from_env() -> Option<Self> {
        Self::try_from_env().unwrap_or_else(|error| {
            eprintln!("Latency tracing disabled: {error}");
            None
        })
    }
    fn try_from_env() -> Result<Option<Self>, String> {
        let Some(path) = std::env::var_os("SLITHER_LATENCY_TRACE") else {
            return Ok(None);
        };
        let revision = std::env::var("SLITHER_LATENCY_REVISION")
            .map_err(|_| "SLITHER_LATENCY_REVISION is required")?;
        if !(8..=64).contains(&revision.len()) || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid latency source revision".into());
        }
        if path.is_empty() {
            return Err("Latency output path is empty".into());
        }
        if Path::new(&path)
            .try_exists()
            .map_err(|e| format!("Invalid latency output: {e}"))?
        {
            return Err("Latency output already exists".into());
        }
        let parent = Path::new(&path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if !parent.is_dir() {
            return Err("Latency output parent is not a directory".into());
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
    pub fn finish(mut self, complete: bool) {
        self.thread.take();
        self.finished = true;
        if let Err(error) = self.recorder.write(&self.path, &self.revision, complete) {
            eprintln!("Latency trace finalization failed: {error}");
        }
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
    // Environment changes are confined to child processes; tests never mutate
    // the multi-threaded test runner's process environment.
    #[test]
    fn invalid_configuration_is_reported_once_and_startup_continues() {
        let path = std::env::temp_dir().join(format!(
            "latency-config-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let existing = path.with_extension("existing");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&existing)
            .unwrap()
            .write_all(b"preserve")
            .unwrap();
        let missing_parent = path.join("trace.jsonl");
        let oversized_revision = "a".repeat(65);
        for (output, revision, reports) in [
            (None, Some("invalid"), 0),
            (Some(path.as_os_str()), None, 1),
            (Some(path.as_os_str()), Some("invalid"), 1),
            (Some(path.as_os_str()), Some(oversized_revision.as_str()), 1),
            (Some(path.as_os_str()), Some(""), 1),
            (Some(std::ffi::OsStr::new("")), Some("2f34a000"), 1),
            (Some(existing.as_os_str()), Some("2f34a000"), 1),
            (Some(missing_parent.as_os_str()), Some("2f34a000"), 1),
        ] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "latency::tests::configuration_startup_child",
                    "--nocapture",
                ])
                .env("SLITHER_LATENCY_CONFIG_TEST", "1")
                .env_remove("SLITHER_LATENCY_TRACE")
                .env_remove("SLITHER_LATENCY_REVISION");
            if let Some(output) = output {
                command.env("SLITHER_LATENCY_TRACE", output);
            }
            if let Some(revision) = revision {
                command.env("SLITHER_LATENCY_REVISION", revision);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW for the startup probe.
            }
            let result = command.output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("startup continued"));
            assert_eq!(
                String::from_utf8_lossy(&result.stderr)
                    .matches("Latency tracing disabled:")
                    .count(),
                reports
            );
        }
        assert!(!path.exists());
        assert_eq!(std::fs::read(&existing).unwrap(), b"preserve");
        std::fs::remove_file(existing).unwrap();
    }
    #[test]
    fn configuration_startup_child() {
        if std::env::var_os("SLITHER_LATENCY_CONFIG_TEST").is_some() {
            assert!(Run::from_env().is_none());
            assert!(worker().is_none());
            crate::game::LocalGame::new(0.).unwrap();
            println!("startup continued");
        }
    }
    #[test]
    fn sparse_retired_buffers_keep_only_recorded_rows() {
        let recorder = Recorder::with_limit(4096, 512);
        for id in 0..128 {
            let _buffer = recorder.attach();
            event("test", id, 0, 0);
        }
        let buffers = recorder.shared.lock().unwrap();
        assert_eq!(buffers.retained, 256); // event + thread_end per worker lifetime
        assert_eq!(buffers.rows.len(), 128);
        assert!(buffers.rows.iter().all(|rows| rows.len() == 2));
        assert_eq!(buffers.active, 0);
        assert_eq!(buffers.dropped, 0);
    }
    #[test]
    fn run_budget_counts_loss_and_stops_registration_growth() {
        let recorder = Recorder::with_limit(8, 5);
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            for _ in 0..2 {
                scope.spawn(|| {
                    let _buffer = recorder.attach();
                    for id in 0..3 {
                        event("test", id, 0, 0);
                    }
                    // Both buffers are live before either claims the retirement budget.
                    barrier.wait();
                });
            }
        });
        assert_eq!(recorder.snapshot().0.len(), 5);
        assert_eq!(recorder.snapshot().1, 3); // eight attempted rows, five retained
        for _ in 0..100 {
            let _buffer = recorder.attach();
            event("test", 0, 0, 0);
        }
        {
            let buffers = recorder.shared.lock().unwrap();
            assert_eq!(buffers.rows.len(), 2);
            assert_eq!(buffers.retained, 5);
            assert_eq!(buffers.active, 0);
            assert_eq!(buffers.dropped, 203);
        }
        let path = std::env::temp_dir().join(format!(
            "latency-run-budget-{}-{}.jsonl",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        recorder.write(&path, "2f34a000", true).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let header: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(header["complete"], false);
        assert_eq!(header["dropped"], 203);
        std::fs::remove_file(path).unwrap();
    }
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
