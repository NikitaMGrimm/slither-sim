"""Select P3's decisive failed-slot pilot record; never fit a food model.

Run with the retained study and the pinned source export. Endpoint-bearing
inputs are read locally; only explicitly selected fields enter the output.
"""
import argparse
from collections import Counter
from datetime import datetime
import gzip
import hashlib
import importlib
import json
from pathlib import Path
import sys


SOURCE_REVISION = "7580d53baffa35d776af4fc22a9926220d33c672"
TEMPLATE_SHA256 = "50bc4ad5203114e016aeadb598dea5705e6fe1c5bc7c566f997ffcf1d9a5d954"
SLOTS = [("A", "positive"), ("A", "negative"), ("B", "positive"),
         ("B", "negative"), ("B", "positive"), ("B", "negative"),
         ("A", "positive"), ("A", "negative")]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def failed_slot_gate(slots):
    """Only a decisive failure is supported; passing needs the other P3 gates."""
    if len(slots) != 8:
        raise ValueError("expected all eight scheduled slots, including missing slots")
    for ordinal, (row, (lane, direction)) in enumerate(zip(slots, SLOTS), 1):
        if (row["scheduledSlot"], row["lane"], row["direction"], row["split"]) != (
                ordinal, lane, direction, "calibration" if ordinal <= 4 else "holdout"):
            raise ValueError("slot schedule differs from frozen P3")
        if row["status"] not in ("completed", "aborted", "missing"):
            raise ValueError("unfinished or unknown slot status")
    completed = [row for row in slots if row["status"] == "completed"]
    if len(completed) >= 4:
        raise ValueError("slot count does not decisively fail; evaluate the full frozen gate")
    return {
        "verdict": "fail", "completedSlots": len(completed), "requiredCompletedSlots": 4,
        "observedCompletedLaneDirections": [f"{row['lane']}/{row['direction']}" for row in completed],
        "knownSectorListingClearGate": {"observedQualified": None, "required": 10,
                                        "status": "not evaluated after decisive slot failure"},
        "clearPositiveReintroductionGate": {"observedQualified": None, "required": 5,
                                            "status": "not evaluated after decisive slot failure"},
        "queryGate": {"eligible200msQueries": None, "required": 20,
                      "status": "not evaluated after decisive slot failure"},
        "pilot2": "not attempted; frozen stop rule", "development": "not attempted",
        "heldout": "not attempted", "candidateCount": 32, "candidatesScored": 0,
        "selectedModel": None, "candidateErrors": None, "replacementContrast": None,
        "simulatorChangeRecommended": False,
    }


def analyze(study, source_root):
    pins = json.loads((Path(__file__).parent / "source-pins.json").read_text(encoding="utf-8"))
    for name, expected in pins["files"].items():
        if digest(source_root / name) != expected:
            raise ValueError(f"pinned analysis dependency changed: {name}")
    sys.path.insert(0, str(source_root / "tools/fidelity"))
    evidence = importlib.import_module("evidence")
    quality = importlib.import_module("capture_quality")
    protocol = importlib.import_module("slither_protocol")
    plan = evidence.read_plan(study)
    publication = Path(__file__).parent / "03-food-lanes.json"
    if digest(publication) != publication.with_suffix(".sha256").read_text().strip():
        raise ValueError("published plan digest mismatch")
    if publication.with_suffix(".source.sha256").read_text().strip() != TEMPLATE_SHA256:
        raise ValueError("unexpected frozen template digest")
    del plan["capture"]["server"]
    plan["method"] = plan["method"].replace("192.0.2.1", "[documentation address withheld]")
    if plan != json.loads(publication.read_text(encoding="utf-8")):
        raise ValueError("bound plan scientific fields differ from the published frozen plan")
    starts, ends = evidence.read_attempts(study)
    if len(starts) != 1 or len(ends) != 1:
        raise ValueError("this selector requires exactly one retained pilot attempt")
    end = next(iter(ends.values()))
    start = next(iter(starts.values()))
    if end["status"] != "completed" or end["exitCode"] != 0 or len(end["captures"]) != 1:
        raise ValueError("pilot acquisition did not complete intact")
    archive = study / end["captures"][0]
    audit = quality.audit_capture(archive)
    if audit["status"] != "verified" or audit["connections"] != 1 or audit["joins"] != 1:
        raise ValueError("pilot archive integrity or single-connection assumption failed")
    if not audit["integrityVerified"] or not audit["provenanceValid"]:
        raise ValueError("pilot integrity/provenance failed")
    records = [json.loads(line) for line in gzip.open(archive, "rt", encoding="utf-8")]
    metadata = [(line, row) for line, row in enumerate(records, 1) if isinstance(row, dict)]
    routes = [(line, row) for line, row in metadata if row.get("event") == "food-lane-route-end"]
    if len(routes) != 1:
        raise ValueError("expected one terminal route event")
    route_line, route = routes[0]
    if route["completedPasses"] != sum(row["status"] == "completed" for row in route["slots"]):
        raise ValueError("terminal pass counter disagrees with slot ledger")
    phases = []
    for line, row in metadata:
        if row.get("event") == "food-lane-phase-end":
            phases.append({"jsonlLine": line, **{key: row[key] for key in (
                "kind", "scheduledSlot", "lane", "direction", "split", "reason", "completed",
                "elapsedMs", "endpointDistance", "maxCrossTrack")}})
    if [row["scheduledSlot"] for row in phases if row["kind"] == "pass" and row["completed"]] != [
            row["scheduledSlot"] for row in route["slots"] if row["status"] == "completed"]:
        raise ValueError("phase completion events disagree with terminal slot ledger")
    packet_counts, food_entries = Counter(), 0
    setup_times = []
    for frame in protocol.read(archive):
        if isinstance(frame, protocol.Frame):
            for packet in frame.packets:
                command = chr(packet[0])
                packet_counts[command] += 1
                if command == "a":
                    setup_times.append(frame.t)
                elif command == "F":
                    food_entries += (len(packet) - 3) // 4
    connect = next(row for _, row in metadata if row.get("event") == "connect")
    terminal = records[-1]
    if terminal.get("event") != "capture-end" or len(setup_times) != 1:
        raise ValueError("missing terminal receipt or ambiguous setup")
    study_audit = evidence.audit_study(study)
    if study_audit != {"status": "incomplete", "issues": ["planned_attempts_missing"],
                       "attempts": 1, "captures": 1}:
        raise ValueError("unexpected study audit")
    return {
        "studyId": "fg-20261026-03-food-lanes", "scientificStatus": "exploratory, unreviewed",
        "sourceRevision": SOURCE_REVISION, "templateSha256": TEMPLATE_SHA256,
        "boundPlanSha256": digest(study / "plan.json"), "studyAudit": study_audit,
        "batches": {"planned": 6, "started": 1, "acquisitionCompleted": 1,
                    "quarantined": 0, "notAttempted": 5, "eligibleForMainAnalysis": 0},
        "pilotGate": failed_slot_gate(route["slots"]),
        "route": {"status": route["status"], "reason": route["reason"],
                  "terminalJsonlLine": route_line, "slots": [
                      {key: row[key] for key in ("scheduledSlot", "lane", "direction", "split", "status")}
                      for row in route["slots"]], "phases": phases},
        "archive": {"alias": "p3-pilot-1-member-1", "sha256": digest(archive),
                    **{key: audit[key] for key in ("status", "issues", "warnings", "integrityVerified",
                        "completeGzip", "connections", "joins", "counts")},
                    "jsonlLines": len(records), "bytes": archive.stat().st_size},
        "diagnosticCommandOutcomes": json.loads((study / "analysis/diagnostic-outcomes.json").read_text()),
        "descriptiveOnly": {
            "foodPacketCounts": {key: packet_counts[key] for key in ("F", "f", "b", "w", "c", "C", "<")},
            "foodEntriesInF": food_entries,
            "limits": "All phases included. These are delivery packets, not qualified exposures, physical births, or refill evidence.",
            "joinedUntilRouteStopSeconds": round((route["t"] - setup_times[0]) / 1000, 6),
            "connectToRouteStopSeconds": round((route["t"] - connect["t"]) / 1000, 6),
            "recordedAcquisitionSeconds": round((terminal["t"] - records[0]["t"]) / 1000, 6),
            "connectionTimeUpperBoundSeconds": (datetime.fromisoformat(end["time"]) -
                                                datetime.fromisoformat(start["time"])).total_seconds(),
            "connectionTimeLimit": "Whole durable invocation from before child spawn through child exit bounds its single socket, including connecting/closing. Actual socket-close time is unrecorded.",
        },
        "limits": ["Pilot data never enter fitting or validation.",
                   "The completed-slot gate fails independently of the remaining coverage gates; their qualified denominators are unassessed, not zero.",
                   "No phase estimate, candidate-boundary/uncertainty scoring, food-identity matching or local-replacement inference was performed.",
                   "One single-endpoint pilot is not an independent-server sample. Raw archives and endpoint provenance are local-only."],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--study", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", type=Path)
    args = parser.parse_args()
    result = analyze(args.study, args.source_root)
    if args.check and result != json.loads(args.check.read_text(encoding="utf-8")):
        raise ValueError("selected result differs from retained evidence")
    if args.output:
        with args.output.open("x", encoding="utf-8", newline="\n") as stream:
            stream.write(json.dumps(result, indent=2, ensure_ascii=False, allow_nan=False) + "\n")
    print(json.dumps({"pilotGate": result["pilotGate"]["verdict"],
                      "completedSlots": result["pilotGate"]["completedSlots"],
                      "archiveStatus": result["archive"]["status"], "selectedResultMatches": bool(args.check)}))


if __name__ == "__main__":
    main()
