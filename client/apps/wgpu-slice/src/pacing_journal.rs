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
// Covers 360 Hz at the maximum 180 s window, plus admission/warmup/collector
// guards. Remains bounded (4 MiB of scalar receipts); overflow still rejects.
const MAX_RECEIPTS: usize = 262_144;
impl Journal {
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os("SLITHER_FRAME_PACING_JOURNAL")?;
        if path.is_empty() {
            return None;
        }
        Self::new(path.into())
    }
    fn new(path: PathBuf) -> Option<Self> {
        Some(Self {
            path,
            start: Instant::now(),
            unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs_f64()
                * 1000.,
            rows: Vec::with_capacity(MAX_RECEIPTS),
            overflow: false,
        })
    }
    pub fn presented(&mut self, identity: u64) {
        if self.rows.len() == MAX_RECEIPTS {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_window_keeps_all_receipts_and_exports_overflow_explicitly() {
        let directory = std::env::temp_dir().join(format!(
            "pacing-journal-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("receipts.json");
        let mut journal = Journal::new(path.clone()).unwrap();
        for identity in 0..(180 + 5) * 360 {
            journal.presented(identity);
        }
        assert_eq!(journal.rows.len(), 66_600);
        assert!(!journal.overflow);
        for identity in journal.rows.len()..=MAX_RECEIPTS {
            journal.presented(identity as u64);
        }
        assert!(journal.overflow);
        assert_eq!(journal.rows.len(), MAX_RECEIPTS);
        drop(journal);
        let exported: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(exported["overflow"], true);
        assert_eq!(exported["rows"].as_array().unwrap().len(), MAX_RECEIPTS);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
