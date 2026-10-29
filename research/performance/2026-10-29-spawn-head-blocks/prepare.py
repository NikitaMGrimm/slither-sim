"""Reuse #272's fixture; add a separate whole-Safety construction diagnostic."""
from pathlib import Path
import hashlib
import json
import sys

prior = Path(__file__).resolve().parents[1] / "2026-10-29-spawn-head-distance/prepare.py"
exec(compile(prior.read_text(), str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
root = Path(sys.argv[2]).resolve()
arm = sys.argv[3]
source = root / arm / "source"
p = source / "src/client/simulation/study_spawn_fixture.rs"
p.write_text(p.read_text() + '''
impl RawSimulator {
    pub(crate) fn study_block_cost(&self) -> Vec<serde_json::Value> {
        let safety=crate::spawn::Safety::new_excluding(&self.backend.world,
            Some(8),26,crate::spawn::CIRCULAR_CLEARANCE,crate::spawn::HEAD_PATH_SECONDS);
        safety.study_block_cost()
    }
    pub(crate) fn study_build_safety(&self) {
        std::hint::black_box(crate::spawn::Safety::new_excluding(
            &self.backend.world, Some(8), 26, crate::spawn::CIRCULAR_CLEARANCE,
            crate::spawn::HEAD_PATH_SECONDS));
        std::hint::black_box(crate::spawn::Safety::departure_floor(
            &self.backend.world, 8, 26));
    }
}
''', newline="\n")
p = source / "src/client/server/study_fixed.rs"
p.write_text(p.read_text() + '''
pub fn build_cost(legacy: bool, row: &str) {
    let mut fixture=Fixture::new(legacy,row);
    for _ in 0..300 { fixture.tick(true,None); }
    if std::env::var_os("STUDY_BLOCK_ONLY").is_some() {
        println!("{}",serde_json::json!({"samples":fixture.sim.world.study_block_cost(),
            "safetiesPerSearch":1,"searchesPerSample":16,"row":row,"legacy":legacy}));
        return;
    }
    let before=fixture.state();
    for _ in 0..16 { fixture.sim.world.study_build_safety(); }
    crate::study_tick::enable();
    let mut samples=Vec::new();
    for _ in 0..64 {
        let start=crate::study_tick::begin();
        for _ in 0..16 { fixture.sim.world.study_build_safety(); }
        samples.push(crate::study_tick::end(start));
    }
    assert_eq!(before,fixture.state());
    println!("{}",serde_json::json!({"samples":samples,"searchesPerSample":16,
        "safetiesPerSearch":2,"stateUnchanged":true,"row":row,"legacy":legacy}));
}
''', newline="\n")
p = source / "src/spawn/safety.rs"
body = "Vec::new()" if arm == "base" else '''
        crate::study_tick::enable();
        let mut samples=Vec::new();
        for _ in 0..64 {
            let start=crate::study_tick::begin();
            for _ in 0..16 {
                for obstacle in &self.obstacles {
                    std::hint::black_box(head_blocks(&obstacle.edges));
                }
            }
            samples.push(crate::study_tick::end(start));
        }
        samples'''
p.write_text(p.read_text() + "\nimpl Safety { pub(crate) fn study_block_cost(&self) -> Vec<serde_json::Value> {\n" + body + "\n} }\n", newline="\n")
(source / "examples/study_build.rs").write_text('''fn main() {
    let a:Vec<_>=std::env::args().collect();
    slither_sim::client::server::study_fixed::build_cost(a[1]=="13",&a[2]);
}
''', newline="\n")

def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

files = {p.relative_to(source).as_posix(): sha(p) for p in source.rglob("*") if p.is_file()}
manifest = root / arm / "source-manifest.json"
manifest.write_text(json.dumps(files, sort_keys=True, indent=2) + "\n", newline="\n")
p = root / arm / "source-stage.json"
pins = json.loads(p.read_text())
pins.update(files=files, sources={k: v for k, v in files.items() if k.endswith(".rs")},
            instrumentedManifestSha256=sha(manifest), studyPrepareSha256=sha(Path(__file__)),
            headDistancePrepareSha256=sha(prior))
p.write_text(json.dumps(pins, indent=2) + "\n", newline="\n")
