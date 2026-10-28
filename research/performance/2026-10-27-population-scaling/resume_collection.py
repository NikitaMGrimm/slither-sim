"""Serialize already-paused warmups after the owner's one-CPU amendment.

python3 resume_collection.py REPOSITORY OUTPUT --cpu 2 \
  --inflight 5000-uncapped=PID 10000-capped=PID
Only exact task-owned warmup commands can be resumed. Completed evidence is
never overwritten. Use collect.py for an entirely fresh reproduction.
"""
import argparse
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("repository")
parser.add_argument("output", type=Path)
parser.add_argument("--cpu", type=int, required=True)
parser.add_argument("--inflight", nargs="+", required=True)
args = parser.parse_args()
root = args.output.resolve()
os.sched_setaffinity(0, {args.cpu})
os.environ.update(RAYON_NUM_THREADS="1", CARGO_BUILD_JOBS="1", RUST_TEST_THREADS="1")
study = Path(__file__).resolve().parent
prepared = []
for item in args.inflight:
    case, pid = item.split("=")
    if case not in {"5000-uncapped", "10000-capped"}:
        raise RuntimeError("not an owner-paused primary case")
    process = Path("/proc") / str(int(pid))
    summary = root / case / "summary.json"
    if not summary.exists():
        command = (process / "cmdline").read_bytes().split(b"\0")
        expected = [str(root / "plain/study_scaling"), "warm", str(root / case)]
        if command[:3] != [s.encode() for s in expected]:
            raise RuntimeError("refusing to resume a different process")
        os.sched_setaffinity(int(pid), {args.cpu})
        os.kill(int(pid), signal.SIGCONT)
        while True:
            try:
                state = (process / "stat").read_text()
            except FileNotFoundError:
                break
            if ") Z " in state:
                break
            time.sleep(5)
    if not summary.is_file():
        raise RuntimeError("warmup ended without its complete checkpoint")
    json.loads(summary.read_text())
    print(f"completed retained warmup: {case}", flush=True)
    prepared.append(case)
# Release both retained worlds before checkpoint-loading measurement children.
for case in prepared:
    subprocess.run([sys.executable, study / "collect.py", args.repository, root,
                    "--cpu", str(args.cpu), "--cases", case, "--prepared", case,
                    "--skip-bots"], check=True)
subprocess.run([sys.executable, study / "collect.py", args.repository, root,
                "--cpu", str(args.cpu), "--cases", "5000-capped", "2000-common",
                "1000-common", "--skip-bots"], check=True)
with (root / "sequential-resource.json").open("x") as stream:
    json.dump({"affinity": sorted(os.sched_getaffinity(0)), "cargoJobs": 1,
               "rayonThreads": 1, "maximumIndividualChildRssKiB": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
               "note": "Retained warmups were resumed sequentially; no paused world remains during counted checkpoint loads."}, stream, indent=2)
    stream.write("\n")
