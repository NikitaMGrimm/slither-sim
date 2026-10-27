"""Reuse the pinned recipient-work fixtures for ordered-food-compaction build/trace/count.

Usage: python3 reproduce.py {build,trace,count} REPOSITORY NEW_OUTPUT [OPTIONS]
Linux x86-64, Python 3.12+, Rust 1.99.0, patch and user PMU access required.
Count additionally needs --perf PATH. Use --cpu to select one logical CPU.
PICKUP_TARGET_ROOT optionally reuses task-owned native-{base,candidate}/target caches.
"""

import os
import runpy
import subprocess
import sys
from pathlib import Path

if not __debug__:
    raise SystemExit("Study checks require Python without -O/PYTHONOPTIMIZE")

FIXTURE_REF = "8014ad0da6ad8d67f3d305ca3c43ed53841d66bd"
FIXTURE_PATH = "research/performance/2026-10-27-host-snake-candidates"
BASE = "32d4cbcf0d3f248db0298c3b4e201c01567823f8"
CANDIDATE = "30040c8ada2ee3d632333ffd22eb21f5214e9bda"
GUARD = 'if not __debug__: raise SystemExit("Study checks require Python without optimization")\n'

if len(sys.argv) < 4 or sys.argv[1] not in {"build", "trace", "count"}:
    raise SystemExit(__doc__)

mode, repository, output = sys.argv[1:4]
repository = Path(repository).resolve()
output = Path(output).resolve()
fixture = output.with_name(output.name + "-fixture")
fixture.mkdir(parents=True, exist_ok=True)
for name in ["reproduce.py", "protocol_fixture.rs", "tick_counter.rs"]:
    data = subprocess.check_output(
        ["git", "-C", str(repository), "show", f"{FIXTURE_REF}:{FIXTURE_PATH}/{name}"]
    )
    if name.endswith(".py"):
        data = GUARD.encode() + data
        # Optional task-owned Cargo caches; executable copies and hashes remain
        # under the new output. This changes build storage, not runtime sources.
        if os.environ.get("PICKUP_TARGET_ROOT"):
            old = b"target=out/'target';"
            if data.count(old) != 1:
                raise SystemExit("Pinned harness target declaration changed")
            data = data.replace(
                old,
                b"target=Path(os.environ['PICKUP_TARGET_ROOT']).resolve()/('native-'+arm)/'target';",
            )
    (fixture / name).write_bytes(data)

# The reused harness keeps full raw traces, checks terminal records, rehashes
# binaries and alternates five pairs after one warmup. Only runtime refs change.
sys.argv.extend(["--base", BASE] if "--base" not in sys.argv else [])
sys.argv.extend(["--candidate", CANDIDATE] if "--candidate" not in sys.argv else [])
runpy.run_path(str(fixture / "reproduce.py"), run_name="__main__")

if mode == "build":
    # This helper is a separate Python process during historical trace checking.
    for arm in ["base", "candidate"]:
        helper = output / f"native-{arm}" / "harness/trace_compare.py"
        helper.write_bytes(GUARD.encode() + helper.read_bytes())
