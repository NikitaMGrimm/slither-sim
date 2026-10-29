"""Deduplicate build pins while retaining each valid attempt and every pair.

Usage: assemble.py INITIAL SECOND FINAL DESTINATION
Inputs are select_results.py outputs from the retained archive directories.
The procedural counting exclusion is recorded separately, never selected.
"""
from pathlib import Path
import hashlib
import json
import sys

sources = list(map(Path, sys.argv[1:4]))
destination = Path(sys.argv[4])
builds = {}
output = {"builds": builds}
for key, path in zip(["initialRejected", "secondRejected", "selected"], sources):
    value = json.loads(path.read_text(encoding="utf8"))
    refs = {}
    for arm, pins in value.pop("builds").items():
        digest = hashlib.sha256(json.dumps(pins, sort_keys=True).encode()).hexdigest()
        builds[digest] = pins
        refs[arm] = digest
    value["buildRefs"] = refs
    value["selectorOutputSha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
    output[key] = value
output["excludedAttempt"] = {
    "runtime": "7d34ef8c",
    "reason": "Counting began before explicit proof-download completion; excluded in full",
    "retainedDirectory": "round2",
}
destination.write_text(json.dumps(output, indent=2) + "\n", encoding="utf8", newline="\n")
