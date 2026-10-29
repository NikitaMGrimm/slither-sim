"""Combine the verified canonical prefix and native pins without private identifiers.

Usage: assemble_results.py PRIVATE_ROOT OUTPUT [INTERRUPTED_ROOT] [EARLIER_REPEAT_ROOT] [PHYSICAL_CORE_FAILURE_ROOT]
Run select_results.py for each rows/LABEL/raw into analysis/LABEL.json first.
"""
from pathlib import Path
import hashlib
import json
import sys
from select_results import bind_archive

ORDER = [("base", 160), ("candidate", 160), ("candidate", 320), ("base", 320), ("base", 480), ("candidate", 480)]


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def row_archive(row, folder, *, primary=False):
    receipt = json.loads((folder / "retrieval-verification.json").read_text())
    archive = folder / "results.tar.gz"
    assert archive.stat().st_size == receipt["archiveBytes"]
    assert row["sourceArchiveSha256"] == receipt["archiveSha256"] == sha(archive), "Selected row belongs to a different archive"
    assert (row["label"], row["arm"], row["primary"]["bots"]) == (receipt["label"], receipt["arm"], receipt["target"])
    verified = receipt["requiredSourcesVerified"] is True and receipt["status"] == "complete"
    if primary:
        assert verified, "Primary archive receipt is incomplete or source-unverified"
    row["evidenceStatus"] = "verified" if verified else "unverified"
    row["archive"] = {k: receipt[k] for k in ["archiveSha256", "archiveBytes", "requiredSourcesVerified", "status"]}
    return receipt


def generator_segment(state, types, rows):
    topology = next(v["VCpuInfo"] for v in types if v["InstanceType"] == state["type"])
    return {"rows": rows, "type": state["type"], "vcpus": topology["DefaultVCpus"],
            "physicalCores": topology["DefaultCores"], "threadsPerCore": topology["DefaultThreadsPerCore"]}


def supplementary(row):
    """Bound earlier rows to cited comparisons and their qualification receipts."""
    w = row["primary"]
    return {k: row[k] for k in ["label", "arm", "verification", "archive", "evidenceStatus", "totalDeferredTicks"]} | {
        "primary": {k: w[k] for k in ["bots", "ticks", "worldCpuMsPerTick", "deferredTicks", "boost", "clientValid"]} | {
            "spawns": {"respawns": w["spawns"]["respawns"]},
            "slowestCpu1Percent": {k: w["slowestCpu1Percent"][k] for k in ["n", "worldMeanMs", "eventTicks"]},
            "socket": {"errors": w["socket"]["errors"]},
        }
    }


def main():
    root, destination = map(Path, sys.argv[1:3])
    build = json.loads((root / "build-pins-private.json").read_text())
    state = json.loads((root / "state-private.json").read_text())
    recovered_generator = "generatorOld" in state
    split = state.get("replacementStartsAtRow", 3)
    types = json.loads((root / "types-private.json").read_text())["InstanceTypes"]
    if recovered_generator:
        assert build == json.loads((root / "build-pins-before-replacement-private.json").read_text())
        assert state["server"]["id"] == json.loads((root / "state-before-generator-replacement-private.json").read_text())["server"]["id"]
    accepted = len(state["verifiedRows"])
    assert accepted in (4, 6), "Only a complete 160/320 prefix or all three pairs may publish"
    not_measured = [480] if accepted == 4 else []
    rows, analysis = [], None
    for order, (arm, seats) in enumerate(ORDER[:accepted], 1):
        label = f"{order:02d}-{arm}-{seats}"
        row = json.loads((root / "analysis" / f"{label}.json").read_text())
        assert row["arm"] == arm and row["primary"]["bots"] == seats
        row_archive(row, root / "rows" / label, primary=True)
        if analysis is None:
            analysis = row["analysisSha256"]
        assert row.pop("analysisSha256") == analysis
        row["order"] = order
        row["generatorType"] = state["generatorOld"]["type"] if recovered_generator and order < split else state["generator"]["type"]
        rows.append(row)

    arms = {}
    for arm, pins in build["arms"].items():
        manifest = root / "bundle" / "arms" / arm / "source-manifest.json"
        assert sha(manifest) == pins["instrumentedManifestSha256"]
        source = manifest.parent / "server/source"
        assert json.loads(manifest.read_text()) == {p.relative_to(source).as_posix(): sha(p) for p in source.rglob("*") if p.is_file()}
        arms[arm] = {k: pins[k] for k in ["runtime", "archiveSha256", "instrumentedManifestSha256"]}
        arms[arm]["hostBinarySha256"] = pins["binaries"]["host"]
    shared = build["arms"]["candidate"]
    assert shared["sharedPolicySha256"] == build["arms"]["base"]["sharedPolicySha256"]
    output = {
        "server": {"type": "c7a.medium", "vcpus": 1},
        "generator": {"workers": 6, "primarySegments": [generator_segment(state["generatorOld"], types, list(range(1, split))), generator_segment(state["generator"], types, list(range(split, accepted + 1)))] if recovered_generator else [generator_segment(state["generator"], types, list(range(1, accepted + 1)))]},
        "sameTargetServerAcrossAllPrimaryRows": True,
        "pins": {
            "arms": arms,
            "sharedClientBinarySha256": build["sharedClientSha256"],
            "sharedClientSourceSha256": shared["clientSourceSha256"],
            "sharedPolicySha256": shared["sharedPolicySha256"],
            "toolchain": shared["toolchain"], "cpuTarget": shared["cpuTarget"],
            "prepareSha256": shared["studyPrepareSha256"],
            "collectorBundleSha256": sha(root / "bundle.tar.gz"),
            "analysisSha256": analysis | {"assembler": sha(Path(__file__))},
        },
        "rows": rows,
        "notMeasured": not_measured,
        "rawAccess": "Local-only private archives. Not publicly downloadable; full receipts, binaries, endpoints and cloud identifiers are excluded.",
    }
    if len(sys.argv) > 3:
        interrupted = Path(sys.argv[3])
        previous = json.loads((interrupted / "build-pins-private.json").read_text())
        for arm in arms:
            for key in ["runtime", "archiveSha256", "instrumentedManifestSha256", "clientSourceSha256", "sharedPolicySha256", "toolchain", "cpuTarget"]:
                assert previous["arms"][arm][key] == build["arms"][arm][key], key
        retained = []
        for order, arm in enumerate(["base", "candidate"], 1):
            label = f"{order:02d}-{arm}-160"
            row = json.loads((interrupted / "analysis" / f"{label}.json").read_text())
            assert row.pop("analysisSha256") == analysis
            row_archive(row, interrupted / "rows" / label)
            retained.append(row)
        salvage = json.loads((interrupted / "interruption-salvage.json").read_text())
        archive = interrupted / "rows/03-candidate-320/interrupted-server-results.tar.gz"
        assert archive.stat().st_size == salvage["archiveBytes"] and sha(archive) == salvage["archiveSha256"]
        output["interruptedAttempt"] = {
            "outcome": "Spot reclaimed the generator during candidate-320 warmup; no 320 primary completed",
            "scope": "Supplementary completed 160 rows, not pooled into the primary comparisons",
            "sourcePins": "Same pins as primary; source manifests are not duplicated",
            "binaryOverrides": {arm: p["binaries"] for arm, p in previous["arms"].items() if p["binaries"] != build["arms"][arm]["binaries"]},
            "rows": [supplementary(row) for row in retained],
            "survivingHostArchive": {k: salvage[k] for k in ["archiveBytes", "archiveSha256", "localRequiredSourcesVerified"]},
            "missingEvidence": "Incomplete third-row generator receipts were deleted by Spot with its root volume; unavailable",
        }
    if recovered_generator:
        salvage = json.loads((root / "interruption-salvage.json").read_text())
        archive = root / "rows/interrupted-04-base-320/interrupted-server-results.tar.gz"
        assert archive.stat().st_size == salvage["archiveBytes"] and sha(archive) == salvage["archiveSha256"]
        assert state["generatorOld"]["type"] == state["generator"]["type"] or (state.get("ondemandApproved") and state["generator"]["type"] == "c8a.2xlarge" and state["generator"].get("market") == "on-demand")
        output["primarySessionInterruption"] = {
            "scope": "Same target retained; two Spot generator interruptions during row 4, then approved single On-Demand c8a.2xlarge",
            "completedRowsRetained": list(range(1, split)), "unfinishedRowRestarted": split,
            "generatorInstances": 3, "primaryGeneratorInstances": 2,
            "marketExceptionApproved": state.get("ondemandApproved", False),
            "survivingHostArchive": {k: salvage[k] for k in ["archiveBytes", "archiveSha256", "localRequiredSourcesVerified"]},
            "missingEvidence": "Spot-deleted row 4 warmup and finalization client receipts unavailable; neither attempt supplies a primary",
        }
    if state.get("ondemandApproved"):
        salvage = json.loads((root / "interruption-finalization-salvage.json").read_text())
        archive = root / "rows/interrupted-finalization-04-base-320/interrupted-server-results.tar.gz"
        assert archive.stat().st_size == salvage["archiveBytes"] and sha(archive) == salvage["archiveSha256"]
        output["primarySessionInterruption"]["survivingFinalizationHostArchive"] = {k: salvage[k] for k in ["archiveBytes", "archiveSha256", "localRequiredSourcesVerified"]}
    if len(sys.argv) > 4:
        earlier = Path(sys.argv[4])
        assert build == json.loads((earlier / "build-pins-private.json").read_text())
        retained = []
        for label in ["01-base-160", "02-candidate-160", "supplementary-03-candidate-320"]:
            row = json.loads((earlier / "analysis" / f"{label}.json").read_text())
            assert row.pop("analysisSha256") == analysis
            assert row["verification"]["allGeneratorAndQueueGatesPass"]
            row_archive(row, earlier / "rows" / label)
            retained.append(supplementary(row))
        bad = earlier / "rows/03-candidate-320"
        summary = json.loads((bad / "raw/run-summary.json").read_text())
        window = next(w for w in summary["windows"] if w["bots"] == 320)
        v = window["result"]
        assert not v["clientValid"] and v["client"]["worstInputP99Ms"] >= 42
        receipt = bind_archive(bad / "raw")
        assert receipt["requiredSourcesVerified"] is True and receipt["status"] == "complete"
        salvage = json.loads((earlier / "interruption-salvage.json").read_text())
        archive = earlier / "rows/interrupted-04-base-320/interrupted-server-results.tar.gz"
        assert archive.stat().st_size == salvage["archiveBytes"] and sha(archive) == salvage["archiveSha256"]
        output["earlierRepeatAttempt"] = {
            "scope": "Qualified supplementary rows from the earlier repeated pair; none supplies a final primary",
            "sourcePins": "Same source and native binary pins as final primary; not duplicated",
            "rows": retained,
            "survivingHostArchive": {k: salvage[k] for k in ["archiveBytes", "archiveSha256", "localRequiredSourcesVerified"]},
            "missingEvidence": "Spot-deleted fourth-row generator receipts unavailable; no primary 320 had completed there",
            "invalidGeneratorRow": {
                "scope": "Unqualified online diagnostic only; excluded from live-tail comparison",
                "generatorType": "c7i.2xlarge", "bots": 320, "arm": "candidate",
                "tickRange": v["tickRange"], "clientValid": False,
                "worstInputP99Ms": v["client"]["worstInputP99Ms"], "inputGateMs": 42,
                "maximumWorkerCpuP99Fraction": max(w["p99"] for w in v["client"]["workerCpuFraction"].values()),
                "socketErrors": v["socket"]["errors"], "queues": v["socket"]["queues"],
                "boostShare": v["boost"]["aliveTickShare"],
                "archive": {k: receipt[k] for k in ["archiveSha256", "archiveBytes", "requiredSourcesVerified"]},
            },
        }
    if len(sys.argv) > 5:
        failed = Path(sys.argv[5])
        assert build == json.loads((failed / "build-pins-private.json").read_text())
        folder = failed / "rows/01-base-160"
        summary = json.loads((folder / "raw/run-summary.json").read_text())
        value = next(w["result"] for w in summary["windows"] if w["bots"] == 160)
        assert not value["clientValid"] and value["client"]["worstInputP99Ms"] >= 42
        receipt = bind_archive(folder / "raw")
        assert receipt["requiredSourcesVerified"] is True and receipt["status"] == "complete"
        output["physicalCoreGeneratorFailure"] = {
            "scope": "Unqualified online diagnostic only; no primary comparison",
            "generatorType": "c7a.2xlarge", "bots": 160, "arm": "base", "clientValid": False,
            "tickRange": value["tickRange"], "worstInputP99Ms": value["client"]["worstInputP99Ms"], "inputGateMs": 42,
            "maximumWorkerCpuP99Fraction": max(w["p99"] for w in value["client"]["workerCpuFraction"].values()),
            "socketErrors": value["socket"]["errors"], "queues": value["socket"]["queues"],
            "boostShare": value["boost"]["aliveTickShare"],
            "archive": {k: receipt[k] for k in ["archiveSha256", "archiveBytes", "requiredSourcesVerified"]},
        }
    destination.write_text(json.dumps(output, indent=2) + "\n", newline="\n")


if __name__ == "__main__":
    main()
