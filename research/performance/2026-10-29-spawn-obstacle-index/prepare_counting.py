"""Expose the retained private controllers' gate, live-parity and pin-freezing steps.

Run on the remote builder:
  prepare_counting.py OUTPUT gates CORRECTNESS_JSON RUST_RECEIPTS_JSON
  prepare_counting.py OUTPUT {parity,pins}
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
assert mode in ["gates", "parity", "pins"]
fixture = root.parent / "fixture"


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def tested():
    # The exact files the gate and parity checks ran; later steps refuse any change.
    return {
        "fixture": {"worldSha256": sha(fixture / "world.json"),
                    "metadataSha256": sha(fixture / "fixture.json")},
        "arms": {arm: {"studyLiveSha256": sha(root / arm / "bin/study_live"),
                       "pinsFileSha256": sha(root / arm / "pins.json")}
                 for arm in ["base", "candidate"]},
    }


if mode == "gates":
    # The inherited write_gates.py records only pass flags, so bind its receipts
    # here to the arms it checked; later steps refuse a rebuilt arm.
    receipts = ["required-checks-pass.json", "parity-pass.json", "gates-bound.json"]
    assert not any((root / name).exists() for name in receipts), "receipts exist"
    files = tested()
    subprocess.run([sys.executable, str(Path(__file__).parent.parent
                    / "2026-10-29-spawn-ring-prefilter/write_gates.py"),
                    str(root), *sys.argv[3:5]], check=True)
    assert tested() == files, "artifacts changed during gate checks"
    with (root / "gates-bound.json").open("x") as output:
        json.dump(files, output, indent=2)
        output.write("\n")
    sys.exit()

for name in ["required-checks-pass.json", "parity-pass.json"]:
    assert json.loads((root / name).read_text())["passed"]
assert json.loads((root / "gates-bound.json").read_text()) == tested(), \
    "binary, pins or fixture changed after the gate checks"

if mode == "parity":
    files = tested()
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
    assert tested() == files, "artifacts changed during parity"
    with (root / "live-parity-pass.json").open("x") as output:
        json.dump({"passed": True, "rows": 2, **files}, output, indent=2)
        output.write("\n")
else:
    receipt = json.loads((root / "live-parity-pass.json").read_text())
    assert receipt["passed"]
    current = tested()
    assert receipt["fixture"] == current["fixture"], "fixture changed after parity"
    assert receipt["arms"] == current["arms"], "binary or pins changed after parity"

    manifest = {
        "runnerSha256": sha(Path(__file__).with_name("run_pairs.py")),
        "fixtureSha256": sha(fixture / "world.json"),
        "fixtureMetadataSha256": sha(fixture / "fixture.json"),
        "arms": {arm: {"pinsFileSha256": sha(root / arm / "pins.json")}
                 for arm in ["base", "candidate"]},
    }
    with (root / "measurement-pins.json").open("x") as output:
        json.dump(manifest, output, indent=2)
