//! Opt-in bounded successful-present identities. No per-frame I/O or scene scan.
use std::{
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub struct Journal {
    path: PathBuf,
    start: Instant,
    unix_ms: f64,
    rows: Vec<(u64, u64)>,
    overflow: bool,
}
impl Journal {
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os("SLITHER_FRAME_PACING_JOURNAL")?;
        if path.is_empty() {
            return None;
        }
        Some(Self {
            path: path.into(),
            start: Instant::now(),
            unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs_f64()
                * 1000.,
            rows: Vec::with_capacity(65_536),
            overflow: false,
        })
    }
    pub fn presented(&mut self, identity: u64) {
        if self.rows.len() == self.rows.capacity() {
            self.overflow = true;
            return;
        }
        self.rows.push((
            self.start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
            identity,
        ));
    }
}
impl Drop for Journal {
    fn drop(&mut self) {
        let data = serde_json::json!({"schema":1,"unixStartMs":self.unix_ms,"overflow":self.overflow,"rows":self.rows,
            "measure":"successful native present receipts with prepared-picture identity; not pixel uniqueness"});
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
        {
            Ok(file) => {
                if let Err(error) = serde_json::to_writer(file, &data) {
                    eprintln!("Pacing journal export failed: {error}");
                }
            }
            Err(error) => eprintln!("Pacing journal export failed: {error}"),
        }
    }
}
