"""Require complete JSONL and exact ordered actions/worlds in the bot traces."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path

if not __debug__:
    raise SystemExit("run without -O: trace assertions are required")

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("output", type=Path)
p.add_argument("--population", type=int, default=500)
p.add_argument("--cpu", type=int, default=1)
a = p.parse_args()
os.sched_setaffinity(0, {a.cpu})
prefix = a.output / f"verified-trace-{a.population}"
decisions = actions = worlds = 0
ended = reported = False
digest = hashlib.sha256()
with gzip.open(str(prefix) + "-base.jsonl.gz", "rb") as left, gzip.open(str(prefix) + "-candidate.jsonl.gz", "rb") as right:
    for line in left:
        if line != right.readline():
            raise RuntimeError("trace bytes differ")
        digest.update(line)
        row = json.loads(line)
        if "decision" in row:
            assert not ended and row["decision"] == decisions
            assert set(row) == {"decision", "actions", "frame", "world"}
            decisions += 1
            actions += len(row["actions"])
            worlds += row["world"] is not None
        elif row.get("end") is True:
            assert not ended and row["decisions"] == decisions == 1200
            ended = True
        else:
            assert ended and not reported and row["decisions"] == 200
            reported = True
    assert not right.read(1)
assert decisions == 1200 and worlds == 202 and ended and reported
result = {"population": a.population, "decisions": decisions, "backgroundActions": actions,
          "worldSnapshots": worlds, "controlledFrames": decisions,
          "complete": True, "byteIdentical": True, "sha256": digest.hexdigest()}
(a.output / f"trace-validation-{a.population}.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
