//! Own only a directory this invocation created. Failed output is moved aside,
//! not discarded: raw captures and failed attempts remain recoverable.
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);

pub fn with_new_directory<T>(
    output: &Path,
    run: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    fs::create_dir(output).map_err(|e| e.to_string())?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let marker = output.join(format!(".frame-perf-owner-{token}"));
    // This file is created only after exclusive directory creation succeeds.
    // A pre-existing output never reaches the callback or cleanup path.
    if let Err(error) = fs::write(&marker, token.as_bytes()) {
        let _ = fs::remove_dir(output); // Empty only; never recursively remove foreign contents.
        return Err(error.to_string());
    }
    match run() {
        Ok(value) => {
            fs::remove_file(marker).map_err(|e| e.to_string())?;
            Ok(value)
        }
        Err(error) => {
            if fs::read(&marker).ok().as_deref() != Some(token.as_bytes())
                || fs::symlink_metadata(output).map_or(true, |m| m.file_type().is_symlink())
            {
                return Err(format!(
                    "{error}; output ownership changed, cleanup refused"
                ));
            }
            let parent = output.parent().unwrap_or_else(|| Path::new("."));
            let failed = parent.join(format!("frame-perf-failed-{token}"));
            // Reserve a new container first. The destination inside it cannot
            // overwrite a pre-existing output or failed attempt.
            fs::create_dir(&failed).map_err(|e| format!("{error}; retaining output: {e}"))?;
            let retained = failed.join("output");
            fs::rename(output, &retained).map_err(|e| format!("{error}; retaining output: {e}"))?;
            Err(format!(
                "{error}; failed output retained at {}",
                retained.display()
            ))
        }
    }
}
