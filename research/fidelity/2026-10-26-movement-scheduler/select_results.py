"""Select bounded publication data from retained local evidence, without sockets.

Supply the original task root with --source-root. --check compares parsed JSON
and plan digests without writing. This does not rerun the scientific analysis.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
VISIBILITY = HERE.parent / "2026-10-26-visibility-pilot"


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def select(root):
    outputs = {}
    manifest = read(root / "freeze-manifest.json")
    for name, directory in (("01-snake-x", VISIBILITY),
                            ("01-snake-diagonal", VISIBILITY),
                            ("02-scheduler-movement", HERE)):
        source = root / "plans" / (name + ".json")
        digest = sha(source)
        assert digest == (source.with_suffix(".sha256")).read_text().strip()
        assert digest == manifest["files"]["plans/" + name + ".json"]
        plan = read(source)
        placeholder = plan["capture"].pop("server")["ip"]
        plan["method"] = plan["method"].replace(placeholder, "[unbound placeholder removed]")
        outputs[directory / (name + ".json")] = plan
        outputs[directory / (name + ".source.sha256")] = digest + "\n"
        published = json.dumps(plan, indent=2, ensure_ascii=True, allow_nan=False) + "\n"
        outputs[directory / (name + ".sha256")] = hashlib.sha256(published.encode()).hexdigest() + "\n"

    study = root / "execution" / "01-snake-x-study"
    result = read(root / "execution" / "RESULT.json")
    route = read(study / "analysis" / "route-inventory.json")
    grid = read(study / "analysis" / "causal-descriptive-grid.json")
    diagnostics = read(study / "analysis" / "descriptive-diagnostics.json")
    endpoints = route["members"][1]["endpoints"]
    legs = [e for e in endpoints if e["crossingEndpoint"]["start"] is not None]
    assert len(legs) == route["endpointToEndpointLegs"]
    sequences = sum(a["crossingTrial"] == b["crossingTrial"] and
                    a["crossingEndpoint"]["start"]["offset"] == 800 and
                    a["crossingEndpoint"]["end"]["offset"] == 2300 and
                    b["crossingEndpoint"]["end"]["offset"] == 800
                    for a, b in zip(legs, legs[1:]))
    members = []
    for n, member in enumerate(diagnostics["members"]):
        members.append(dict(archiveAlias=f"p1-member-{n + 1}", **{
            k: member[k] for k in ("archiveSha256", "status", "issues", "joins",
                                   "connections", "counts", "commands")}))
    gate = dict(result["gate"])
    gate["recommendation"] = "switch to P2 under its separate authorization"
    outputs[VISIBILITY / "selected-results.json"] = dict(
        studyAudit=result["studyAudit"], gate=gate, members=members,
        descriptiveCounters=grid["counters"],
        route=dict(endpointLegs=len(legs), nearFarNearSequences=sequences,
                   initialApproaches=route["initialEndpointArrivals"],
                   legs=[dict(trial=e["crossingTrial"], jsonlLine=e["jsonlLine"],
                              startOffset=e["crossingEndpoint"]["start"]["offset"],
                              endOffset=e["crossingEndpoint"]["end"]["offset"],
                              **{k: e["crossingEndpoint"][k] for k in
                                 ("interrupted", "pauseInterruptionCount", "avoidanceInterruptionCount")})
                         for e in legs]),
        sourceRevision=manifest["revision"],
        sourceHashes={name: sha(path) for name, path in (
            ("pilot-result", root / "execution" / "RESULT.json"),
            ("route-inventory", study / "analysis" / "route-inventory.json"),
            ("descriptive-grid", study / "analysis" / "causal-descriptive-grid.json"),
            ("diagnostics", study / "analysis" / "descriptive-diagnostics.json"))})

    study = root / "execution" / "p2" / "02-scheduler-movement-study"
    tooling = study / "analysis" / "tooling"
    final = read(tooling / "final-verification.json")
    summaries = []
    source_hashes = {}
    for ordinal in range(1, 7):
        name = f"ordinal-{ordinal:02d}-summary.json"
        source = study / "analysis" / name
        original = read(source)
        source_hashes[name] = sha(source)
        # Explicit allowlist: omit operational fields and complete witness traces.
        selected = {k: original[k] for k in (
            "ordinal", "split", "captureAudit", "integrityAbortGatePassed",
            "unexpectedLifecycleEvents", "joins", "connections", "archiveSha256",
            "coverage", "counts", "headingStatuses", "gaps", "fieldGaps",
            "motionGapByFieldMask", "exceptionContexts", "qualifiedInputStrata",
            "boundedQualifiedCommands", "reflectionDelayEndpointSummariesMs",
            "transport", "diagnosticCommandStatus", "candidates", "analysisSha256", "crossCheck")}
        selected["archiveAlias"] = f"p2-ordinal-{ordinal:02d}"
        selected["earliestWitnesses"] = {
            k: [{field: value for field, value in witness.items()
                 if field not in ("packetHex", "packetsHex")} for witness in witnesses]
            for k, witnesses in original["earliestWitnesses"].items()
            if k in ("motion-under35", "h-under100", "reverse-same-frame-mass-motion",
                     "unchanged-tuple/direction-unchanged")}
        member = final["members"][ordinal - 1]
        for key in ("archiveSha256", "coverage", "transport", "crossCheck"):
            assert original[key] == member[key]
        assert original["analysisSha256"] == sha(tooling / "analyze_p2.py")
        summaries.append(selected)
        outputs[HERE / name] = selected
    split_gates = {}
    totals = Counter()
    for split in ("development", "heldout"):
        group = [s for s in summaries if s["split"] == split]
        eligible = sum(s["coverage"]["batchEligible"] for s in group)
        counts = Counter()
        for s in group:
            counts.update(s["counts"])
        totals.update(counts)
        split_gates[split] = dict(batches=len(group), eligible=eligible,
                                 requiredEligible=2, passed=eligible >= 2,
                                 motionUnder35=counts["motion-under35"], hUnder100=counts["h-under100"])
    outputs[HERE / "selected-results.json"] = dict(
        studyAudit=final["studyAudit"], splitGates=split_gates,
        motionUnder35=totals["motion-under35"], hUnder100=totals["h-under100"],
        transportGatesPassed=sum(t["result"] == "PASS" for s in summaries for t in s["transport"]),
        selectedCandidate=final["selectedCandidate"],
        verification={k: final[k] for k in ("status", "frozenInputsUnchanged", "scientificCodeUnchanged",
                       "developmentSealUnchanged", "developmentSealPrecedesHeldout",
                       "sourceCopiesAndSyntheticBytesVerified", "startIntervalsSeconds")})
    outputs[HERE / "source-pins.json"] = dict(
        sourceRevision=manifest["revision"],
        frozenProtocolSha256=sha(root / "PROTOCOLS.md"),
        freezeManifestSha256=sha(root / "freeze-manifest.json"),
        originalSummaries=source_hashes,
        originalTooling={p.name: sha(p) for p in (
            tooling / "analyze_p2.py", tooling / "test_analyze_p2.py", tooling / "check_analysis.py",
            tooling / "make_summary.py", tooling / "final-verification.json")},
        decoderAndTools={k.removeprefix("source-origin-main-7580d53/"): v
                         for k, v in manifest["files"].items()
                         if k in ("source-origin-main-7580d53/tools/fidelity/slither_protocol.py",
                                  "source-origin-main-7580d53/tools/fidelity/packet_census.py",
                                  "source-origin-main-7580d53/tools/fidelity/transport_parity.py",
                                  "source-origin-main-7580d53/tests/fixtures/transport-official-near-server.json")})
    for name in ("analysis-code-freeze.json", "development-seal.json", "development-selection.json",
                 "development-predictions.json", "analysis-revision-preservation.json"):
        outputs[HERE / name] = read(tooling / name)
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    outputs = select(args.source_root)
    for path, value in outputs.items():
        if args.check:
            actual = read(path) if path.suffix == ".json" else path.read_text()
            assert actual == value, path.name
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            text = json.dumps(value, indent=2, ensure_ascii=True, allow_nan=False) + "\n" if isinstance(value, dict) else (
                json.dumps(value, indent=2) + "\n" if isinstance(value, list) else value)
            path.write_text(text, encoding="utf-8", newline="\n")
    print(f"{'Checked' if args.check else 'Selected'} {len(outputs)} publication files")


if __name__ == "__main__":
    main()
