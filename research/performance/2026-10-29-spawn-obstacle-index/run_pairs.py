"""Five portable alternating pairs; reuse complete-host controls by reference."""
from pathlib import Path
import hashlib
import importlib.util
import json
import os
import subprocess
import sys

root, inventory = map(Path, sys.argv[1:3])
protocol, row = int(sys.argv[3]), sys.argv[4]
assert protocol in [19, 13]
fixture = root.parent / "fixture"
manifest_path = root / "measurement-pins.json"
manifest_bytes = manifest_path.read_bytes()
manifest = json.loads(manifest_bytes)
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == manifest["runnerSha256"]


def verify_pins():
    assert manifest_path.read_bytes() == manifest_bytes
    for name, key in [("world.json", "fixtureSha256"), ("fixture.json", "fixtureMetadataSha256")]:
        with (fixture / name).open("rb") as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == manifest[key]
    for arm in ["base", "candidate"]:
        pins_path = root / arm / "pins.json"
        assert hashlib.sha256(pins_path.read_bytes()).hexdigest() == manifest["arms"][arm]["pinsFileSha256"]
        pins = json.loads(pins_path.read_text())
        for binary, digest in pins["binaries"].items():
            with (root / arm / "bin" / binary).open("rb") as stream:
                assert hashlib.file_digest(stream, "sha256").hexdigest() == digest


verify_pins()
binding_path = root / f"{row}-{protocol}-measurement-binding.json"
binding = {"measurementManifestSha256": hashlib.sha256(manifest_bytes).hexdigest(), "status": "started"}
with binding_path.open("x") as stream:
    json.dump(binding, stream)


def completed():
    verify_pins()
    binding["status"] = "complete"
    binding_path.write_text(json.dumps(binding) + "\n")


if not row.startswith("live-"):
    prior = Path(__file__).resolve().parents[1] / "2026-10-29-spawn-head-distance/run_pairs.py"
    exec(compile(prior.read_text(), str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
    completed()
    raise SystemExit(0)

assert row in ["live-refused", "live-successful"]
assert all(json.loads((root / name).read_text())["passed"]
           for name in ["required-checks-pass.json", "parity-pass.json"])
assert json.loads((root / "live-parity-pass.json").read_text())["passed"]
os.sched_setaffinity(0, {1})
spec = importlib.util.spec_from_file_location("inventory", inventory)
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
prefix = f"{row}-{protocol}"
expected = [1, 4096, 1, 0] if row == "live-refused" else [1, 1638, 0, 0]


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


pairs = []
reference = None
for pair in range(-1, 5):
    shared.inventory(root, f"{prefix}-pair-{pair}-before")
    paths = {}
    for arm in (["base", "candidate"] if pair < 0 or pair % 2 == 0 else ["candidate", "base"]):
        binary = root / arm / "bin/study_live"
        pins = json.loads((root / arm / "pins.json").read_text())
        assert pins["rustflags"] == "" and pins["releaseBuild"]
        assert sha(binary) == pins["binaries"]["study_live"]
        path = root / f"{prefix}-pair-{pair}-{arm}.json"
        with path.open("x") as output, path.with_suffix(".log").open("x") as log:
            subprocess.run(["taskset", "-c", "0", str(binary), "admission", str(fixture),
                            row.removeprefix("live-"), "512", "--measure"],
                           stdout=output, stderr=log, env=dict(os.environ, RAYON_NUM_THREADS="1"),
                           check=True, timeout=120)
        value = json.loads(path.read_text())
        assert value["stateUnchanged"] and len(value["records"]) == 512
        trace = []
        for record in value["records"]:
            sample = record["sample"]
            assert sample["events"] == expected
            assert sample["enabledNs"] > 0 and sample["runningNs"] / sample["enabledNs"] >= .99
            trace.append({k: v for k, v in record.items() if k != "sample"})
        assert all(record == trace[0] for record in trace)
        if reference is None:
            reference = trace
        assert trace == reference
        assert sha(binary) == pins["binaries"]["study_live"]
        paths[arm] = path.name
    shared.inventory(root, f"{prefix}-pair-{pair}-after")
    pairs.append({"pair": pair, "warmup": pair < 0, "paths": paths})
    (root / f"pairs-{prefix}.json").write_text(json.dumps(pairs, indent=2) + "\n")
print("Five live-like admission pairs complete", protocol, row)
completed()
