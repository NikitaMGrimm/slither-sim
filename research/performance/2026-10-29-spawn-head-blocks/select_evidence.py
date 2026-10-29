"""Select bounded evidence on the builder after all proofs and pairs finish."""
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


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write(name, value):
    (destination / name).write_text(json.dumps(value, indent=2) + "\n", newline="\n")


pins = {}
for arm in ["base", "candidate", "mutation"]:
    value = read(proof / arm / "pins.json")
    assert value["rustflags"] == "" and value["releaseBuild"]
    for name, digest in value["binaries"].items():
        assert sha(proof / arm / "bin" / name) == digest
    pins[arm] = {k: value[k] for k in ["commit", "archiveSha256", "instrumentedManifestSha256", "binaries"]}
controls = read(root / "focused-controls.json")
assert controls["focused"]["exit"] == controls["restored"]["exit"] == 0
assert controls["wrongSkipMutation"]["exit"] == 101
assert "test result: FAILED" in (root / "wrong-skip.log").read_text()
checks = read(root / "rust-group.json")["fullRustCommands"]
assert len(checks) == 15 and all(v["exit"] == 0 for v in checks)
catalog, host, queued = [read(proof / name) for name in ["catalog-parity.json", "host-parity.json", "fixed-parity.json"]]
assert [len(catalog), len(host), len(queued)] == [23, 6, 2]
for rows in [catalog, host, queued]:
    assert all(v["base"] == v["candidate"] for v in rows)
assert all(v["wireMutationDetected"] for v in queued)
write("correctness.json", {
    "sourceAndBinaryPins": pins,
    "build": {k: value[k] for k in ["toolchain", "rustflags", "studyPrepareSha256", "headDistancePrepareSha256", "priorPrepareSha256"]},
    "portableBuildWrapperSha256": sha(root / "repo/research/performance/2026-10-29-spawn-head-blocks/build_staged.py"),
    "focused": {"headVerdicts": 1223404, "directBlockVerdicts": 50159,
                "standaloneSegmentVerdicts": 409996, "independentSweptComparisonVerdicts": 19992,
                "wrongSkipMutationExit": 101, "exactRestorationPass": True},
    "fullRustCommands": [{"command": v["command"], "exit": v["exit"]} for v in checks],
    "catalog": [{"case": v["case"], **v["base"]} for v in catalog],
    "host": [{"case": v["case"], **v["base"]} for v in host],
    "queued": [{"protocol": v["protocol"], "identicalBaseAndCandidate": v["base"], "mutation": v["mutation"], "wireMutationDetected": True} for v in queued],
    "access": "Full sources, logs, binaries and traces local-only; digests identify but do not grant access.",
})
measurements, qualification = {}, []
for row in ["spawn", "seats80", "seats160"]:
    measurements[row] = read(proof / f"selected-fixed-{row}.json")
    for protocol in [19, 13]:
        assert read(root / f"measure-{row}-{protocol}.json")["exit"] == 0
        pairs = read(proof / f"pairs-{row}-{protocol}.json")
        assert len(pairs) == 6 and pairs[0]["warmup"]
        minimum = 1.
        summaries = []
        for pair in pairs:
            values = {a: read(proof / pair["paths"][a]) for a in ["base", "candidate"]}
            assert values["base"]["summary"] == values["candidate"]["summary"]
            assert sha((proof / pair["paths"]["base"]).with_suffix(".trace")) == sha((proof / pair["paths"]["candidate"]).with_suffix(".trace"))
            summaries.append(values["base"]["summary"])
            minimum = min(minimum, *(v["sample"]["runningNs"] / v["sample"]["enabledNs"] for d in values.values() for v in d["ticks"] if v["sample"]["enabledNs"]))
        assert minimum >= .99 and all(s == summaries[0] for s in summaries)
        qualification.append({"row": row, "protocol": protocol, "summary": summaries[0], "minimumPmuScheduledFraction": minimum})
write("measurements.json", measurements)
write("construction.json", read(proof / "selected-builds.json"))
profile = read(root / "profile-exhausted/selected-profile.json")
profile.pop("controlAndPerfMessages")
assert profile["qualification"]["blockedExhausted"] == 1800 and profile["qualification"]["waiting"] == 72
write("profile.json", profile)
write("verification.json", {
    "allRequiredProofsPass": True, "catalogTraces": 23, "hostTraces": 6, "queuedTraces": 2,
    "protocols": [19, 13], "wireMutationControls": 2, "focusedWrongSkipMutationControls": 1,
    "fullRustCommandsPassed": 15, "measurementWindows": 60, "discardedWarmupWindows": 12,
    "alternatingPairsPerRowAndProtocol": 5, "repeatedForkAndDirectParentProofs": True,
    "qualifiedRows": qualification,
    "nativeByteProofArchive": {k: read(root / "stage-archives/proofs-ready.json")[k] for k in ["archiveSha256", "archiveBytes", "manifestSha256", "allManifestMembersVerified"]},
    "timingContext": {"instanceType": "c7i.xlarge", "region": "Frankfurt", "market": "Spot", "physicalCoresAssigned": 2, "threadsPerCore": 1, "workerCpu": 0, "controllerCpu": 1,
                      "scope": "AWS virtualized Intel; portable release; one pinned core; no concurrent build/test work; no dedicated physical-host or live-capacity claim"},
    "pmu": "Hardware instructions:u, >=99% scheduled time per tick; root task access without changing kernel/security settings.",
    "localNativeWork": False, "sshGuestUsed": False, "independentReview": False,
    "studySelectionSha256": sha(Path(__file__)),
})
print("Bounded evidence selected", flush=True)
