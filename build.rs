fn main() {
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
