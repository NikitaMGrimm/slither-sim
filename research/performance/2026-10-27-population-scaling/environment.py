"""Capture descriptive Linux metadata without changing kernel settings."""
import json
import os
from pathlib import Path
import platform
import sys
import time
import argparse

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("output", type=Path)
parser.add_argument("--cpu", type=int, default=6)
args = parser.parse_args()
os.sched_setaffinity(0, {args.cpu})
workers = []
for path in Path("/proc").iterdir():
    if not path.name.isdigit():
        continue
    try:
        if (path / "comm").read_text().strip() != "study_scaling":
            continue
        status = (path / "status").read_text().splitlines()
        workers.append({"pid": int(path.name), "threads": len(list((path / "task").iterdir())),
                        "affinity": next(line for line in status if line.startswith("Cpus_allowed_list:"))})
    except FileNotFoundError:
        pass
result = {"utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "kernel": platform.release(),
          "cpu": next(line for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")),
          "logicalCpus": os.cpu_count(), "loadAverage": os.getloadavg(),
          "perfEventParanoid": Path("/proc/sys/kernel/perf_event_paranoid").read_text().strip(), "workers": workers}
with args.output.open("x") as stream:
    json.dump(result, stream, indent=2)
    stream.write("\n")
