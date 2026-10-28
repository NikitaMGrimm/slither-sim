"""Supplement the deterministic matrix with the two catalog loopback host rows.

build OUTPUT; count OUTPUT. Diagnostics count simulation-thread user instructions
only: complete apply/advance/handover tick and nested spawn search. Socket work,
the separate load driver and kernel instructions are outside these PMU brackets.
These wall-clock-driven samples have no cross-arm deterministic parity claim.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess

if not __debug__:
    raise SystemExit("Run without Python optimization")

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("mode", choices=["build", "count"])
p.add_argument("output", type=Path)
p.add_argument("--cpu", type=int, default=0)
p.add_argument("--driver-cpu", type=int, default=2)
a = p.parse_args()
root = a.output.resolve()
os.sched_setaffinity(0, {a.cpu})
spec = importlib.util.spec_from_file_location("study", Path(__file__).with_name("reproduce.py"))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
env = dict(os.environ, CARGO_BUILD_JOBS="1", RAYON_NUM_THREADS="1", RUST_TEST_THREADS="1",
           CARGO_PROFILE_RELEASE_DEBUG="1", RUSTFLAGS="", CARGO_ENCODED_RUSTFLAGS="",
           SLITHER_LOAD_HOST_CPU=str(a.cpu), SLITHER_LOAD_DRIVER_CPU=str(a.driver_cpu))
for key, tool in [("RUSTC", "rustc"), ("RUSTDOC", "rustdoc")]:
    env[key] = subprocess.check_output(["rustup", "which", tool], text=True).strip()

if a.mode == "build":
    pins = {}
    for arm in m.ARMS:
        source = root / f"socket-{arm}"
        shutil.copytree(root / arm, source)
        counter = source / "src/study_counter.rs"
        with counter.open("a") as stream:
            stream.write('''
pub struct Guard(usize, Option<[u64;3]>);
impl Guard {pub fn new(stage:usize)->Self {Self(stage,start())}}
impl Drop for Guard {fn drop(&mut self) {add(self.0,self.1);}}
''')
        path = source / "src/client/simulation.rs"
        text = path.read_text()
        start = text.index("    fn spawn_pose_waited(")
        brace = text.index("{", start)
        path.write_text(text[:brace + 1] + "\n        let _search = crate::study_counter::Guard::new(2);" + text[brace + 1:])
        path = source / "src/client/server.rs"
        m.swap(path, "            for _ in 0..due {", "            for _ in 0..due {\n                crate::study_counter::reset(true);\n                let study_start = crate::study_counter::read_counter();")
        m.swap(path, "                self.hand_over()?;", '''                self.hand_over()?;
                let study_end = crate::study_counter::read_counter();
                let study_search = crate::study_counter::result()["instructions"][2].as_u64().unwrap();
                eprintln!("SPAWN_TICK {}", serde_json::json!([self.ticks,
                    study_end[0]-study_start[0], study_search,
                    study_end[1]-study_start[1], study_end[2]-study_start[2]]));''')
        m.swap(source / "examples/host_load.rs", '"actualTicks": actual,',
               '"actualTicks": actual, "firstTick": before, "lastTick": before + actual,')
        target = root / f"target-{arm}"
        with (root / f"socket-build-{arm}.log").open("x") as log:
            subprocess.run(["cargo", "clean", "--release", "-p", "slither-sim", "--target-dir", str(target)],
                           cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
            subprocess.run(["cargo", "build", "--offline", "--locked", "--release", "-j", "1",
                            "--target-dir", str(target), "--example", "perf_baseline"],
                           cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        dest = root / "bin" / arm / "socket_diagnostic"
        shutil.copyfile(target / "release/examples/perf_baseline", dest)
        dest.chmod(0o755)
        pins[arm] = m.sha(dest)
    (root / "socket-pins.json").write_text(json.dumps(pins, indent=2) + "\n")
else:
    pins = json.loads((root / "socket-pins.json").read_text())
    out = root / "socket-counts"
    out.mkdir()
    selected = {}
    for case in ["host-official-500", "host-battledome-80"]:
        pairs = []
        for pair in range(-1, 5):
            item = {"pair": pair, "warmup": pair < 0}
            for arm in (m.ARMS if pair % 2 == 0 or pair < 0 else m.ARMS[::-1]):
                binary = root / "bin" / arm / "socket_diagnostic"
                assert m.sha(binary) == pins[arm]
                log = out / f"{case}-{pair}-{arm}.log"
                with log.open("x") as stream:
                    subprocess.run([str(binary), case, "286"], env=env,
                                   stdout=stream, stderr=subprocess.STDOUT, check=True)
                lines = log.read_text().splitlines()
                report = json.loads(next(line for line in lines if line.startswith('{"case":')))
                details = report["details"]
                rows = [json.loads(line.removeprefix("SPAWN_TICK ")) for line in lines if line.startswith("SPAWN_TICK ")]
                rows = [r for r in rows if details["firstTick"] < r[0] <= details["lastTick"]]
                assert len(rows) == details["actualTicks"] and len(rows) >= 286
                assert all(r[3] > 0 and r[4] / r[3] >= .99 for r in rows)
                item[arm] = {"ticks": len(rows), "instructionsPerTick": sum(r[1] for r in rows) / len(rows),
                             "searchPerTick": sum(r[2] for r in rows) / len(rows), "details": details}
            pairs.append(item)
            selected[case] = pairs
            (out / "selected.json").write_text(json.dumps(selected, indent=2) + "\n")
            print("SOCKET", case, pair, flush=True)
