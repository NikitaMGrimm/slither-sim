//! Filesystem-only diagnostic regression, no GPU or external input.
#[path = "../examples/frame_perf/output.rs"]
mod output;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[test]
fn owned_failure_frees_retry_path_without_discarding_data_or_touching_existing_output() {
    let root = std::env::temp_dir().join(format!(
        "frame-perf-output-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let target = root.join("run");
    let error = output::with_new_directory(&target, || {
        fs::write(target.join("raw.txt"), b"failed raw bytes").unwrap();
        Err::<(), _>("setup failed".into())
    })
    .unwrap_err();
    assert!(error.contains("setup failed"));
    assert!(!target.exists());
    let failed = fs::read_dir(&root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
        .join("output/raw.txt");
    assert_eq!(fs::read(failed).unwrap(), b"failed raw bytes");
    output::with_new_directory(&target, || {
        fs::write(target.join("success.txt"), b"retained").unwrap();
        Ok(())
    })
    .unwrap();
    let error: Result<(), String> =
        output::with_new_directory(&target, || panic!("existing directory must not run"));
    assert!(error.is_err());
    assert_eq!(fs::read(target.join("success.txt")).unwrap(), b"retained");
    assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
    let replacement = root.join("replacement");
    let error: Result<(), String> = output::with_new_directory(&replacement, || {
        fs::rename(&replacement, root.join("retired-owned")).unwrap();
        fs::create_dir(&replacement).unwrap();
        fs::write(replacement.join("sentinel"), b"pre-existing replacement").unwrap();
        Err("output was replaced".into())
    });
    assert!(error.unwrap_err().contains("ownership changed"));
    assert_eq!(
        fs::read(replacement.join("sentinel")).unwrap(),
        b"pre-existing replacement"
    );
    let absolute = root.canonicalize().unwrap();
    assert!(absolute.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    fs::remove_dir_all(absolute).unwrap();
}
