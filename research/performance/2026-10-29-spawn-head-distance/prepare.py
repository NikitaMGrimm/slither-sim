"""Stage the earlier fixed-host/trace overlay by reference; no local builds.

Usage: prepare.py REPO OUTPUT ARM REF (run on the AWS builder).
"""
from pathlib import Path
import hashlib
import json
import os
import sys

repo, root = map(lambda p: Path(p).resolve(), sys.argv[1:3])
arm, ref = sys.argv[3:]
prior = repo / "research/performance/2026-10-29-spawn-ring-prefilter/prepare.py"
trace_prepare = repo / "research/performance/2026-10-29-pit-minimap-cache/prepare_traces.py"
text = prior.read_text()
old = "if arm == 'mutation':\n    if os.environ.get('STUDY_MUTATION')=='map':"
assert text.count(old) == 1
text = text.replace(old, "if False:\n    if os.environ.get('STUDY_MUTATION')=='map':")
argv = sys.argv[:]
sys.argv = [str(prior), str(trace_prepare), str(repo), str(root), arm, ref]
os.environ["STUDY_STAGE_ONLY"] = "1"
try:
    exec(compile(text, str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
except SystemExit as error:
    assert error.code == 0
finally:
    sys.argv = argv

source = root / arm / "source"
if arm == "mutation":
    path = source / "src/spawn/safety.rs"
    text = path.read_text()
    old = "    pub(crate) fn head_is_clear(&self, head: Point) -> bool {"
    assert text.count(old) == 1
    path.write_text(text.replace(old, old + "\n        if !self.obstacles.is_empty() { return false; }"), newline="\n")

path = source / "src/client/server/study_fixed.rs"
text = path.read_text()
text = text.replace("const SEATS: usize = 80;", "fn seats() -> usize { std::env::var(\"STUDY_SEATS\").map_or(80, |v| v.parse().unwrap()) }")
text = text.replace("SEATS", "seats()")
# The replacement above must not change the environment variable's name.
text = text.replace("STUDY_seats()", "STUDY_SEATS")
text = text.replace('["spawn","minimap","markers"]', '["spawn","minimap","markers","seats80","seats160"]')
old = "            sim.world.study_viewers(legacy,true);"
assert text.count(old) == 1
text = text.replace(old, "            if row.starts_with(\"seats\") { sim.world.study_grow_for_controls(seats()); }\n" + old)
old = "        let flags = self.sim.world.study_flags();"
assert text.count(old) == 1
text = text.replace(old, '''        if self.row.starts_with("seats") && self.sim.ticks.is_multiple_of(72) {
            let slot = (self.sim.ticks as usize / 72) % seats();
            self.sim.world.disconnect(slot).unwrap();
            self.sim.generations[slot] += 1;
            self.sim.waiting.push_back(PendingJoin {
                slot, generation: self.sim.generations[slot],
                appearance: PlayerAppearance::default(), session_id: false,
                physical_food: false, protocol13: self.legacy, full_minimap: false,
                requested_tick: self.sim.ticks, reset_kills: true,
            });
        }
'''+old)
path.write_text(text, newline="\n")
path = source / "src/client/simulation/study_spawn_fixture.rs"
path.write_text(path.read_text() + '''
impl RawSimulator {
    pub(crate) fn study_grow_for_controls(&mut self, seats: usize) {
        assert!(self.backend.world.grow_for_seats(seats, true));
    }
}
''', newline="\n")

def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

files = {p.relative_to(source).as_posix(): sha(p) for p in source.rglob("*") if p.is_file()}
manifest = root / arm / "source-manifest.json"
manifest.write_text(json.dumps(files, sort_keys=True, indent=2) + "\n", newline="\n")
old = json.loads((root / arm / "source-stage.json").read_text())
old.update(files=files, sources={k: v for k, v in files.items() if k.endswith(".rs")},
           instrumentedManifestSha256=sha(manifest), priorPrepareSha256=sha(prior), studyPrepareSha256=sha(Path(__file__)))
(root / arm / "source-stage.json").write_text(json.dumps(old, indent=2) + "\n", newline="\n")
print("HEAD DISTANCE STAGED", arm, flush=True)
