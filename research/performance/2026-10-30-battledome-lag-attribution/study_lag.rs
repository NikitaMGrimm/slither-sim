//! Linux-only observer staged outside src/ in the disposable source tree.
//! Sixteen little-endian u64 words per record; see prepare.py and reduce.py.
use std::cell::RefCell;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[repr(C)]
struct Timespec { sec: i64, ns: i64 }
unsafe extern "C" {
    fn clock_gettime(clock: i32, value: *mut Timespec) -> i32;
    fn gettid() -> i32;
}
fn clock(clock: i32) -> u64 {
    let mut value = Timespec { sec: 0, ns: 0 };
    assert_eq!(unsafe { clock_gettime(clock, &mut value) }, 0);
    value.sec as u64 * 1_000_000_000 + value.ns as u64
}
static SOCKET: AtomicU64 = AtomicU64::new(0);
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

struct State {
    tid: u64,
    socket: u64,
    own_stat: File,
    socket_stat: Option<File>,
    output: BufWriter<File>,
    gate: PathBuf,
    enabled: bool,
    next_check: u64,
    epoch: u64,
    tick: u64,
    deferred: u64,
    seq: u64,
}
fn proc_stat(file: &File) -> [u64; 3] {
    let mut bytes = [0; 160];
    let n = file.read_at(&mut bytes, 0).expect("schedstat read");
    let mut values = std::str::from_utf8(&bytes[..n]).unwrap().split_whitespace();
    std::array::from_fn(|_| values.next().expect("schedstat field").parse().unwrap())
}
impl State {
    fn emit(&mut self, kind: u64, scheduled: u64, requested: u64) {
        if !self.enabled { return; }
        let mono = clock(1);
        let cpu = clock(3);
        let own = proc_stat(&self.own_stat);
        let socket_tid = SOCKET.load(Ordering::Relaxed);
        if self.socket != socket_tid {
            self.socket = socket_tid;
            self.socket_stat = (socket_tid != 0).then(||
                File::open(format!("/proc/self/task/{socket_tid}/schedstat")).unwrap());
        }
        let socket = self.socket_stat.as_ref().map(proc_stat).unwrap_or([0; 3]);
        self.seq += 1;
        let words = [kind, self.seq, mono, scheduled, self.tick, self.deferred,
                     cpu, own[0], own[1], socket[0], socket[1], own[2], socket[2],
                     self.tid, self.socket, requested];
        let mut raw = [0; 128];
        for (i, value) in words.into_iter().enumerate() {
            raw[i*8..i*8+8].copy_from_slice(&value.to_le_bytes());
        }
        self.output.write_all(&raw).expect("observer write");
    }
}
pub fn register_socket() { SOCKET.store(unsafe { gettid() } as u64, Ordering::Relaxed); }
pub fn register_sim(epoch: Instant) {
    let Some(path) = std::env::var_os("STUDY_LOG") else { return; };
    let path = PathBuf::from(path);
    let gate = path.with_file_name("RECORD");
    let mono = clock(1);
    let epoch_ns = mono - epoch.elapsed().as_nanos() as u64;
    let tid = unsafe { gettid() } as u64;
    let metadata = serde_json::json!({"recordWords":16,"wordBytes":8,
        "epochMonotonicNs":epoch_ns,"anchorMonotonicNs":mono,
        "anchorUnixNs":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64,
        "simulationTid":tid,"dtNs":7_000_000});
    std::fs::write(path.with_extension("meta.json"), metadata.to_string()).unwrap();
    STATE.with_borrow_mut(|state| *state = Some(State {
        tid, socket:0, own_stat:File::open(format!("/proc/self/task/{tid}/schedstat")).unwrap(),
        socket_stat:None, output:BufWriter::with_capacity(65536, File::create(path).unwrap()),
        enabled:gate.exists(), gate, next_check:mono, epoch:epoch_ns,
        tick:0, deferred:0, seq:0,
    }));
}
pub fn tick_begin(tick: u64, deferred: u64, dt: f64) {
    STATE.with_borrow_mut(|state| {
        let Some(s) = state else { return; };
        let now = clock(1);
        if now >= s.next_check {
            s.enabled = s.gate.exists();
            s.next_check = now + 200_000_000;
            if !s.enabled { s.output.flush().expect("observer flush"); }
        }
        s.tick = tick + 1;
        s.deferred = deferred;
        s.emit(0, s.epoch + ((tick+deferred+1) as f64 * dt * 1e9).round() as u64, 0);
    });
}
pub fn tick_end(dt: f64) {
    STATE.with_borrow_mut(|state| if let Some(s) = state {
        s.emit(1, s.epoch + ((s.tick+s.deferred) as f64 * dt * 1e9).round() as u64, 0);
    });
}
pub fn sleep_begin(wait: f64) {
    STATE.with_borrow_mut(|state| if let Some(s) = state {
        // The timestamp is before the observer itself: its cost is disclosed.
        s.emit(2, clock(1) + (wait*1e9).round() as u64, (wait*1e9).round() as u64);
    });
}
pub fn sleep_end() {
    STATE.with_borrow_mut(|state| if let Some(s) = state { s.emit(3, 0, 0); });
}
pub fn flush() {
    STATE.with_borrow_mut(|state| if let Some(s) = state { s.output.flush().unwrap(); });
}
pub fn microcalibration() {
    register_sim(Instant::now());
    for enabled in [false,true,true,false] {
        let cpu = clock(3); let wall=clock(1);
        STATE.with_borrow_mut(|state| {
            let s=state.as_mut().unwrap();s.enabled=enabled;
            for _ in 0..20_000 { s.emit(0,0,0); }
            s.output.flush().unwrap();
        });
        println!("{}",serde_json::json!({"enabled":enabled,"records":20000,
            "cpuNs":clock(3)-cpu,"wallNs":clock(1)-wall}));
    }
}
