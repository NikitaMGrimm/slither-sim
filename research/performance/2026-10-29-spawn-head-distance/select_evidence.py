"""Select bounded proof/timing data remotely; source maps and raw traces stay private.

Usage: select_evidence.py PRIVATE_ROOT DESTINATION
"""
from pathlib import Path
import hashlib
import json
import sys

root, destination = map(Path, sys.argv[1:3])
proof = root / "proof"
assert (root / "status").read_text().strip() == "PROOFS_PASSED"
assert (root / "measure-status").read_text().strip() == "MEASUREMENTS_PASSED"
destination.mkdir(exist_ok=True)


def read(path):
    return json.loads(path.read_text())


def write(name, value):
    (destination / name).write_text(json.dumps(value, indent=2) + "\n", newline="\n")


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


artifacts = {}
for arm in ["base", "candidate", "mutation"]:
    pins = read(proof / arm / "pins.json")
    for name, digest in pins["binaries"].items():
        assert sha(proof / arm / "bin" / name) == digest
    artifacts[arm] = {key: pins[key] for key in ["commit", "archiveSha256", "instrumentedManifestSha256", "binaries"]}
pins = read(proof / "candidate/pins.json")
controls = read(root / "focused-controls.json")
assert controls["focused"]["exit"] == controls["restored"]["exit"] == 0
assert controls["boundaryMutation"]["exit"] == 101
assert "test result: FAILED" in (root / "focused-boundary-mutation.log").read_text()
checks = read(root / "rust-group.json")["fullRustCommands"]
assert len(checks) == 15 and all(v["exit"] == 0 for v in checks)
catalog = read(proof / "catalog-parity.json")
host = read(proof / "host-parity.json")
fixed = read(proof / "fixed-parity.json")
assert [len(catalog), len(host), len(fixed)] == [23, 6, 2]
for rows in [catalog, host, fixed]:
    assert all(row["base"] == row["candidate"] for row in rows)
assert all(row["wireMutationDetected"] for row in fixed)
write("correctness.json", {
    "sourceAndBinaryPins": artifacts,
    "build": {key: pins[key] for key in ["toolchain", "rustflags", "studyPrepareSha256", "priorPrepareSha256"]},
    "wireControlPreparationSha256": read(proof / "mutation/pins.json")["studyPrepareSha256"],
    "harness": {name: sha(root / "repo/research/performance/2026-10-29-spawn-head-distance" / name)
                for name in ["run_pairs.py", "fixed_parity.py"]},
    "focused": {"headVerdicts": 1156612, "standaloneSegmentVerdicts": 409996,
                "independentSweptComparisonVerdicts": 19992,
                "boundaryMutationExit": 101, "restoredPass": True},
    "fullRustCommands": [{"command": v["command"], "exit": v["exit"]} for v in checks],
    "catalog": [{"case": row["case"], **row["base"]} for row in catalog],
    "host": [{"case": row["case"], **row["base"]} for row in host],
    "queued": [{"protocol": row["protocol"], "identicalBaseAndCandidate": row["base"],
                "mutation": row["mutation"], "wireMutationDetected": True} for row in fixed],
    "access": "Full sources, logs, binaries and traces local-only; pins identify but do not grant access.",
    "failedInitialWireControl": read(root / "failed-wire-control.json"),
    "failedInitialControlSetup": read(root / "unprimed-setup-failure.json"),
})

measurements = {}
qualification = []
for row in ["spawn", "seats80", "seats160"]:
    measurements[row] = read(proof / f"selected-fixed-{row}.json")
    for protocol in [19, 13]:
        assert read(root / f"measure-{row}-{protocol}.json")["exit"] == 0
        pairs = read(proof / f"pairs-{row}-{protocol}.json")
        assert len(pairs) == 6 and pairs[0]["warmup"]
        scheduling_all = []
        summary = None
        for pair in pairs:
            values = {arm: read(proof / pair["paths"][arm]) for arm in ["base", "candidate"]}
            assert values["base"]["summary"] == values["candidate"]["summary"]
            assert sha((proof / pair["paths"]["base"]).with_suffix(".trace")) == sha((proof / pair["paths"]["candidate"]).with_suffix(".trace"))
            scheduling = [v["sample"]["runningNs"] / v["sample"]["enabledNs"]
                          for value in values.values() for v in value["ticks"] if v["sample"]["enabledNs"]]
            assert min(scheduling) >= .99
            scheduling_all.extend(scheduling)
            if summary is None:
                summary = values["base"]["summary"]
            assert summary == values["base"]["summary"]
        qualification.append({"row": row, "protocol": protocol, "summary": summary,
                              "minimumPmuScheduledFraction": min(scheduling_all)})
write("measurements.json", measurements)
write("verification.json", {
    "allRequiredProofsPass": True, "catalogTraces": 23, "hostTraces": 6, "queuedTraces": 2,
    "protocols": [19, 13], "wireMutationControls": 2, "focusedBoundaryMutationControls": 1,
    "fullRustCommandsPassed": 15, "measurementWindows": 60, "discardedWarmupWindows": 12,
    "alternatingPairsPerRowAndProtocol": 5, "repeatedForkAndDirectParentProofs": True,
    "qualifiedRows": qualification,
    "timingContext": read(root / "timing-context.json"),
    "retainedRustSourceBinding": read(root / "retained-rust-binding.json"),
    "nativeByteProofArchive": {key: read(root / "stage-archives/proofs-ready.json")[key]
                               for key in ["archiveSha256", "archiveBytes", "manifestSha256", "allManifestMembersVerified"]},
    "pmu": "Hardware instructions:u; root-owned task process access; no kernel/security settings changed; >=99% scheduled time required each tick.",
    "localNativeWork": False, "sshGuestUsed": False, "independentReview": False,
    "studySelectionSha256": sha(Path(__file__)),
})
print("Bounded proof and measurement selection complete", flush=True)
