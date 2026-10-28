"""Reverse admission rejection in a task-owned check copy; require test failure."""
import argparse
import json
import os
from pathlib import Path
import subprocess

if not __debug__:
    raise SystemExit("Mutation checks require Python without optimization")

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("source", type=Path)
p.add_argument("target", type=Path)
p.add_argument("output", type=Path)
p.add_argument("--cpu", type=int, default=0)
a = p.parse_args()
os.sched_setaffinity(0, {a.cpu})
a.output.mkdir()
path = a.source / "src/simulator.rs"
original = path.read_bytes()
old = b"&& !bounds.near(query, width + 1.)"
assert original.count(old) == 1
env = dict(os.environ, CARGO_BUILD_JOBS="1", RAYON_NUM_THREADS="1", RUST_TEST_THREADS="1",
           CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0", CARGO_INCREMENTAL="0")
command = ["cargo", "test", "--offline", "--locked", "-j", "1", "--target-dir",
           str(a.target.resolve()), "--lib", "spawn_bounds", "--", "--nocapture"]
try:
    path.write_bytes(original.replace(old, b"&& bounds.near(query, width + 1.)"))
    with (a.output / "broken.log").open("x") as log:
        result = subprocess.run(command, cwd=a.source, env=env, stdout=log, stderr=subprocess.STDOUT)
    text = (a.output / "broken.log").read_text()
    assert result.returncode != 0 and "test result: FAILED" in text and "assertion" in text
finally:
    path.write_bytes(original)
with (a.output / "restored.log").open("x") as log:
    subprocess.run(command, cwd=a.source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
(a.output / "result.json").write_text(json.dumps({"mutation": "reverse box rejection",
    "brokenExit": result.returncode, "oracleFailed": True, "restoredPass": True}, indent=2) + "\n")
