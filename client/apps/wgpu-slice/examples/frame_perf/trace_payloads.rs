//! Lossless bounded storage for API-trace byte payloads; validation runs only.
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
pub fn archive(root: &Path) -> Result<(), String> {
    let absolute = root.canonicalize().map_err(|e| e.to_string())?;
    for directory in fs::read_dir(&absolute).map_err(|e| e.to_string())? {
        let directory = directory.map_err(|e| e.to_string())?;
        if !directory.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        if !directory.path().join(".frame-perf-gpu-trace").is_file() {
            continue;
        }
        for entry in fs::read_dir(directory.path()).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|e| e != "bin") {
                continue;
            }
            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");
            if name.strip_prefix("data").is_none_or(|digits| {
                digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit())
            }) {
                continue;
            }
            let source = path.canonicalize().map_err(|e| e.to_string())?;
            if !source.starts_with(&absolute) || !source.is_file() {
                return Err("trace payload outside owned root".into());
            }
            let bytes = fs::read(&source).map_err(|e| e.to_string())?;
            let compressed = source.with_extension("bin.gz");
            let file = fs::File::create_new(&compressed).map_err(|e| e.to_string())?;
            let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
            encoder.write_all(&bytes).map_err(|e| e.to_string())?;
            encoder
                .finish()
                .map_err(|e| e.to_string())?
                .sync_all()
                .map_err(|e| e.to_string())?;
            let mut restored = Vec::new();
            flate2::read::GzDecoder::new(fs::File::open(&compressed).map_err(|e| e.to_string())?)
                .read_to_end(&mut restored)
                .map_err(|e| e.to_string())?;
            if restored != bytes {
                return Err("trace archive failed exact-byte verification".into());
            }
            // The raw bytes remain in the durable verified archive. RON data
            // references resolve to .bin or losslessly restored .bin.gz offline.
            fs::remove_file(source).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
