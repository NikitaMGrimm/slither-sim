"""Assemble only archive-bound, source-verified canonical rows, remotely."""
from pathlib import Path
import hashlib
import json
import sys

root, destination = map(Path, sys.argv[1:3])


def read(path):
    return json.loads(path.read_text())


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


order = read(root / "order.json")
build = read(root / "pins-master.json")
rows = []
analysis = None
for number, (arm, seats) in enumerate(order, 1):
    label = f"{number:02d}-{arm}-{seats}"
    selected = read(root / "analysis" / (label + ".json"))
    receipt = read(root / "rows" / label / "retrieval-verification.json")
    archive = root / "rows" / label / "results.tar.gz"
    assert receipt["status"] == "complete" and receipt["requiredSourcesVerified"] is True
    assert (receipt["label"], receipt["arm"], receipt["target"]) == (label, arm, seats)
    assert archive.stat().st_size == receipt["archiveBytes"]
    assert selected["sourceArchiveSha256"] == receipt["archiveSha256"] == sha(archive)
    assert selected["label"] == label
    assert selected["primary"]["bots"] == seats and selected["arm"] == arm
    assert selected["verification"]["allGeneratorAndQueueGatesPass"]
    assert selected["verification"]["minimumBoostShareIncludingCalibration"] >= .99
    current = selected.pop("analysisSha256")
    if analysis is None:
        analysis = current
    assert current == analysis
    selected.update(order=number, evidenceStatus="verified",
                    archive={k: receipt[k] for k in ["archiveSha256", "archiveBytes", "requiredSourcesVerified", "status"]})
    rows.append(selected)
arms = {}
for arm, value in build["arms"].items():
    folder = root / "arms" / arm
    assert sha(folder / "source-manifest.json") == value["instrumentedManifestSha256"]
    assert read(folder / "source-manifest.json") == {p.relative_to(folder / "server/source").as_posix(): sha(p)
                                                    for p in (folder / "server/source").rglob("*") if p.is_file()}
    arms[arm] = {k: value[k] for k in ["runtime", "archiveSha256", "instrumentedManifestSha256"]}
    arms[arm]["hostBinarySha256"] = value["binaries"]["host"]
    assert sha(root / "bin" / arm / "host") == value["binaries"]["host"]
    assert sha(root / "bin" / arm / "clients") == value["binaries"]["clients"]
assert build["arms"]["base"]["sharedPolicySha256"] == build["arms"]["candidate"]["sharedPolicySha256"]
shared = build["arms"]["candidate"]
assert sha(root / "bin/candidate/clients") == build["sharedClientSha256"]
output = {
    "server": {"type": "c7a.medium", "vcpus": 1},
    "generator": {"workers": 6, "primarySegments": read(root / "generator-segments.json")},
    "sameTargetServerAcrossAllPrimaryRows": True,
    "pins": {"arms": arms, "sharedClientBinarySha256": build["sharedClientSha256"],
             "sharedClientSourceSha256": shared["clientSourceSha256"],
             "sharedPolicyManifestSha256": hashlib.sha256(json.dumps(shared["sharedPolicySha256"], sort_keys=True).encode()).hexdigest(),
             "toolchain": shared["toolchain"], "cpuTarget": shared["cpuTarget"],
             "prepareSha256": shared["studyPrepareSha256"],
             "analysisSha256": analysis | {"assembler": sha(Path(__file__)),
                                           "runtimeWrapper": sha(Path(__file__).with_name("select_results.py")),
                                           "runtimes": sha(Path(__file__).with_name("runtimes.json"))}},
    "rows": rows,
    "notMeasured": read(root / "not-measured.json")["seats"],
    "rawAccess": "Local-only verified archives; hashes identify but do not grant access. No host/cloud identifiers are published.",
}
destination.write_text(json.dumps(output, indent=2) + "\n", newline="\n")
