"""Focused mutation controls and the repository Rust check group, one CPU."""
import argparse
import os
from pathlib import Path
import subprocess

if not __debug__:
    raise SystemExit("run without -O: mutation assertions are required")

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("source", type=Path, help="clean candidate archive, without diagnostic overlays")
p.add_argument("output", type=Path)
p.add_argument("--cpu", type=int, default=1)
p.add_argument("--target", type=Path, help="optional task-owned reusable target")
a = p.parse_args()
os.sched_setaffinity(0, {a.cpu})
source = a.source.resolve()
out = a.output.resolve()
out.mkdir(exist_ok=False)
env = dict(os.environ, CARGO_BUILD_JOBS="1", RAYON_NUM_THREADS="1", RUST_TEST_THREADS="1",
           CARGO_INCREMENTAL="0",
           CARGO_NET_OFFLINE="true", CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0",
           CARGO_TARGET_DIR=str((a.target or out / "target").resolve()))


def check(command, name, fails=False):
    with (out / (name + ".log")).open("x") as log:
        result = subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
    if fails:
        text = (out / (name + ".log")).read_text()
        if result.returncode == 0 or "test result: FAILED" not in text:
            raise RuntimeError("mutation was not rejected by the regression")
    elif result.returncode:
        raise RuntimeError(f"{name} failed: exit {result.returncode}")


path = source / "src/client/protocol.rs"
original = path.read_bytes()
try:
    text = original.decode()
    anchor = "        self.session.food_frame.take();"
    assert text.count(anchor) == 1
    path.write_text(text.replace(anchor, "        // Mutation: retain stale food snapshot."), newline="\n")
    check(["cargo", "test", "--locked", "--test", "client_protocol",
           "warmed_food_frames_match_fresh_replay_after_every_edit"], "stale-food-control", fails=True)
    anchor = "let dt = (server - self.base_server).max(0.);"
    assert text.count(anchor) == 1
    path.write_text(text.replace(anchor, anchor.replace(";", " + 0.000_001;")), newline="\n")
    check(["cargo", "test", "--locked", "--lib", "completed_motion_matches_bounded_replay_bits"],
          "motion-control", fails=True)
finally:
    path.write_bytes(original)
check(["cargo", "test", "--locked", "--lib", "completed_motion_matches_bounded_replay_bits"], "motion-restored")
check(["cargo", "test", "--locked", "--test", "client_protocol"], "protocol-restored")
check(["python3", "tools/check.py", "rust"], "rust-group")
check(["python3", "-B", "-m", "unittest", "discover", "-s", "tools", "-p", "test_documentation.py"], "documentation")
