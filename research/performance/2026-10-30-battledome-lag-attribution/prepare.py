"""Remote overlay only; leave the checkout's src/ unchanged.

python3 prepare.py PRISTINE_SOURCE STUDY_DIRECTORY
The exact changed source archive/manifest, not a moving script path, pins a build.
"""
from pathlib import Path
import difflib
import hashlib
import json
import shutil
import sys

source, study = map(Path, sys.argv[1:])
path = source / "src/client/server.rs"
original = path.read_text()
text = original

def swap(old, new):
    global text
    assert text.count(old) == 1, (old, text.count(old))
    text = text.replace(old, new)

swap('use super::{', '#[path="../../study_lag.rs"]\nmod study_lag;\nuse super::{')
swap('thread::sleep(Duration::from_secs_f64(wait));',
     'study_lag::sleep_begin(wait);\n            thread::sleep(Duration::from_secs_f64(wait));\n            study_lag::sleep_end();')
swap('''impl Simulation {
    fn run(&mut self) -> Result<(), String> {
        let dt''', '''impl Simulation {
    fn run(&mut self) -> Result<(), String> {
        study_lag::register_sim(self.shared.clock.epoch);
        let dt''')
swap('''            for _ in 0..due {
                // A command''', '''            for _ in 0..due {
                study_lag::tick_begin(self.ticks, self.deferred, dt);
                // A command''')
swap('''                self.hand_over()?;
            }
            let busy''', '''                self.hand_over()?;
                study_lag::tick_end(dt);
            }
            let busy''')
swap('''impl Gateway {
    fn run(&mut self) -> Result<(), String> {
        while''', '''impl Gateway {
    fn run(&mut self) -> Result<(), String> {
        study_lag::register_socket();
        while''')
# All emitted records must be durable on normal host completion. For forced
# shutdown a final incomplete buffered tail is disclosed, never silently parsed.
swap('''        Ok(())
    }

    fn due_ticks''', '''        study_lag::flush();
        Ok(())
    }

    fn due_ticks''')
path.write_text(text, newline="\n")
shutil.copyfile(study / "study_lag.rs", source / "study_lag.rs")
(source / "examples/lag_micro.rs").write_text(
    '#[path="../study_lag.rs"] mod study_lag;\nfn main(){study_lag::microcalibration();}\n')
(source.parent / "instrumentation.diff").write_text("".join(difflib.unified_diff(
    original.splitlines(True), text.splitlines(True), fromfile="a/src/client/server.rs",
    tofile="b/src/client/server.rs")), newline="\n")
manifest = {p.relative_to(source).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(source.rglob("*.rs"))}
(source.parent / "instrumented-manifest.json").write_text(json.dumps(manifest, indent=2)+"\n")
print("Observation overlay staged; production checkout unchanged")
