"""Expose the retained private controllers' live-parity and pin-freezing steps.

Run on the remote builder: prepare_counting.py OUTPUT {parity,pins}.
Creates fresh receipts only; never replaces an archived receipt or manifest.
"""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
mode = sys.argv[2]
assert mode in ["parity", "pins"]
fixture = root.parent / "fixture"
for name in ["required-checks-pass.json", "parity-pass.json"]:
    assert json.loads((root / name).read_text())["passed"]

if mode == "parity":
    for row in ["refused", "successful"]:
        traces = []
        for arm in ["base", "candidate"]:
            path = root / arm / f"live-{row}-parity.json"
            with path.open("xb") as output:
                subprocess.run([str(root / arm / "bin/study_live"), "admission",
                                str(fixture), row, "3"], stdout=output,
                               env=dict(os.environ, RAYON_NUM_THREADS="1"), check=True)
            raw = path.read_bytes()
            value = json.loads(raw)
            assert value["stateUnchanged"] and len(value["records"]) == 3
            assert all(record == value["records"][0] for record in value["records"])
            traces.append(raw)
        assert traces[0] == traces[1], row
    with (root / "live-parity-pass.json").open("x") as output:
        json.dump({"passed": True, "rows": 2}, output)
        output.write("\n")
else:
    assert json.loads((root / "live-parity-pass.json").read_text())["passed"]

    def sha(path):
        with path.open("rb") as stream:
            return hashlib.file_digest(stream, "sha256").hexdigest()

    manifest = {
        "runnerSha256": sha(Path(__file__).with_name("run_pairs.py")),
        "fixtureSha256": sha(fixture / "world.json"),
        "fixtureMetadataSha256": sha(fixture / "fixture.json"),
        "arms": {arm: {"pinsFileSha256": sha(root / arm / "pins.json")}
                 for arm in ["base", "candidate"]},
    }
    with (root / "measurement-pins.json").open("x") as output:
        json.dump(manifest, output, indent=2)
