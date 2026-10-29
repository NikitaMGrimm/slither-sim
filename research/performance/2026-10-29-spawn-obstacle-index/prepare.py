"""Extend the earlier admission/trace overlay by reference; remote use only."""
from pathlib import Path
import hashlib
import json
import runpy
import sys

repo, root = map(lambda value: Path(value).resolve(), sys.argv[1:3])
arm = sys.argv[3]
runpy.run_path(str(repo / "research/performance/2026-10-29-spawn-head-distance/prepare.py"), run_name="__main__")
source = root / arm / "source"
study = Path(__file__).resolve().parent
(source / "src/client/simulation/study_live.rs").write_text((study / "live_fixture.rs").read_text(), newline="\n")
path = source / "src/client/simulation.rs"
path.write_text(path.read_text() + "\npub mod study_live;\n", newline="\n")
(source / "examples/study_live.rs").write_text('''fn main() {
 let a:Vec<_>=std::env::args().collect();
 if a[1]=="warmup" { slither_sim::client::simulation::study_live::warmup(&a[2]); }
 else { slither_sim::client::simulation::study_live::admission(&a[2],&a[3],a[4].parse().unwrap(),a.get(5).is_some_and(|s|s=="--measure")); }
}
''', newline="\n")

def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

files = {p.relative_to(source).as_posix(): sha(p) for p in source.rglob("*") if p.is_file()}
manifest = root / arm / "source-manifest.json"
manifest.write_text(json.dumps(files, sort_keys=True, indent=2) + "\n", newline="\n")
path = root / arm / "source-stage.json"
value = json.loads(path.read_text())
value.update(files=files, sources={k:v for k,v in files.items() if k.endswith(".rs")},
             instrumentedManifestSha256=sha(manifest), livePrepareSha256=sha(Path(__file__)))
path.write_text(json.dumps(value, indent=2) + "\n", newline="\n")
