fn main() {
    perf_client();
    println!("cargo:rustc-check-cfg=cfg(slither_world_rules)");
    println!("cargo:rerun-if-changed=src/transport/frame.rs");
    if std::fs::read_to_string("src/transport/frame.rs")
        .is_ok_and(|source| source.contains("pub rules: WorldRules"))
    {
        println!("cargo:rustc-cfg=slither_world_rules");
    }
    println!("cargo:rustc-check-cfg=cfg(slither_runtime_radius)");
    println!("cargo:rerun-if-changed=src/simulator.rs");
    let source = std::fs::read_to_string("src/simulator.rs").expect("simulator source");
    if source
        .split("pub struct Simulator {")
        .nth(1)
        .is_some_and(|fields| {
            fields
                .split('}')
                .next()
                .unwrap()
                .contains("\n    radius: f64,")
        })
    {
        println!("cargo:rustc-cfg=slither_runtime_radius");
    }
    // The performance overlay copies this build script and one shared adapter
    // into both revisions. Select the runtime API at compile time, without a
    // compatibility layer in the simulator or two copies of workload logic.
    println!("cargo:rustc-check-cfg=cfg(slither_private_state)");
    println!("cargo:rerun-if-changed=src/simulator/initialization.rs");
    if std::path::Path::new("src/simulator/initialization.rs").is_file() {
        println!("cargo:rustc-cfg=slither_private_state");
    }
    println!("cargo:rustc-check-cfg=cfg(slither_checkpoint)");
    println!("cargo:rerun-if-changed=src/simulator/checkpoint.rs");
    if std::path::Path::new("src/simulator/checkpoint.rs").is_file() {
        println!("cargo:rustc-cfg=slither_checkpoint");
    }
}

// The catalog needs private raw-host operations, but adds no runtime API. Compile
// an example-only source mirror of this revision, with the fixture as a child
// of simulation. Historical overlays replace helpers, never the measured engine.
fn perf_client() {
    use std::path::Path;
    fn copy(source: &Path, target: &Path) {
        std::fs::create_dir_all(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let destination = target.join(entry.file_name());
            if path.is_dir() {
                copy(&path, &destination);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                println!("cargo:rerun-if-changed={}", path.display());
                let text = std::fs::read_to_string(&path).unwrap();
                // The library already owns its unit tests. Do not register a
                // second copy or resolve their fixture paths in OUT_DIR.
                let text = text.replace("#[cfg(test)]", "#[cfg(any())]");
                std::fs::write(destination, text).unwrap();
            }
        }
    }
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let helper = root.join("examples/support/catalog_backend.rs");
    println!("cargo:rerun-if-changed={}", helper.display());
    if !helper.is_file() {
        return;
    }
    let mirror = out.join("perf-engine");
    if mirror.exists() {
        std::fs::remove_dir_all(&mirror).unwrap();
    }
    copy(&root.join("src"), &mirror);
    let path = mirror.join("client/simulation/broadcast.rs");
    if helper.is_file() {
        use std::io::Write;
        writeln!(
            std::fs::OpenOptions::new().append(true).open(path).unwrap(),
            "\n#[path = {:?}] pub(crate) mod catalog_backend;",
            helper
        )
        .unwrap();
        writeln!(
            std::fs::OpenOptions::new()
                .append(true)
                .open(mirror.join("client/simulation.rs"))
                .unwrap(),
            "\npub(crate) use broadcast::catalog_backend;"
        )
        .unwrap();
    }
    let lib = std::fs::read_to_string(mirror.join("lib.rs")).unwrap();
    let lib = lib
        .lines()
        .map(|line| {
            if line.starts_with("//!") {
                line.replacen("//!", "//", 1)
            } else if line.starts_with("mod ") {
                format!("pub(crate) {line}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(mirror.join("lib.rs"), lib).unwrap();
}
