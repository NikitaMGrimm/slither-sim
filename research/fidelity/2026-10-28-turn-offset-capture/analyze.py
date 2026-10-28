"""Offline archive audit, pinned native replay and frozen first-arc scoring."""
import argparse
import base64
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
PREVIOUS = HERE.parent / "2026-10-28-turn-count-phase"
spec = importlib.util.spec_from_file_location("count_phase", PREVIOUS / "analyze.py")
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


def audit(path):
    digest = hashlib.sha256()
    times, events, identities = {}, [], {}
    connection, incoming, outgoing, receipts, final = 0, 0, 0, 0, None
    counts = {"i": {"frames": 0, "bytes": 0}, "o": {"frames": 0, "bytes": 0}}
    payload_bytes = 0
    with gzip.open(path, "rb") as stream:
        for line, raw in enumerate(stream, 1):
            if final:
                raise ValueError("data after final integrity receipt")
            row = json.loads(raw)
            if isinstance(row, list):
                direction, t, encoded = row
                data = base64.b64decode(encoded, validate=True)
                counts[direction]["frames"] += 1
                counts[direction]["bytes"] += len(data)
                if direction == "i":
                    incoming += 1
                    times[line] = t
                else:
                    outgoing += 1
                    if data in (bytes([253]), bytes([254])):
                        raise ValueError("boost request in cruise-only capture")
            else:
                if row.get("type") == "header":
                    header = row
                if row.get("event") == "connect":
                    connection += 1
                if row.get("event") == "turn-receipt":
                    identities[row["life"]] = row["id"]
                    receipts += 1
                if row.get("event") in ("turn-control", "turn-receipt", "turn-hold-interrupted"):
                    events.append(dict(row, kind="meta", line=line))
                if row.get("event") in ("capture-end", "capture-interrupted"):
                    final = row
                    integrity = row["integrity"]
                    if (integrity["sha256"] != digest.hexdigest() or integrity["lines"] != line - 1
                            or integrity["payloadBytes"] != payload_bytes or integrity["counts"] != counts):
                        raise ValueError("archive prefix integrity mismatch")
                    if not row["collectorSourcesUnchanged"]:
                        raise ValueError("collector source drift")
                    continue
            digest.update(raw)
            payload_bytes += len(raw)
    if not final:
        raise ValueError("archive missing final integrity receipt")
    return dict(sha256=native.sha(path), incoming=incoming, outgoing=outgoing, lives=connection,
        ownStateReceipts=receipts, integrityVerified=True, completed=final["event"] == "capture-end",
        stopReason=final.get("stopReason")), times, events, identities, header


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--archive-root", type=Path, required=True)
    p.add_argument("--binary", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    a = p.parse_args()
    native.limit_cpu()
    a.output.mkdir(parents=True, exist_ok=False)
    ledger_path = a.archive_root / "invocations.jsonl"
    ledger = [json.loads(s) for s in ledger_path.read_text().splitlines()] if ledger_path.exists() else []
    all_events, summaries = [], []
    for attempt in ledger:
        ordinal = attempt["ordinal"]
        folder = a.archive_root / f"invocation-{ordinal}"
        result_path = folder / "result-private.json"
        acquisition = json.loads(result_path.read_text()) if result_path.exists() else dict(
            reason="launch-or-process-failure", elapsedSeconds=None, socketsClosed=None,
            maxCounts=None, controlsDone=[], interruptionCounts={})
        summaries.append(dict(ordinal=ordinal, members=[], acquisition=acquisition))
        members, headers = [], []
        for member in (0, 1):
            source = folder / f"bot{member}.jsonl.gz"
            try:
                summary, times, events, identities, header = audit(source)
            except (OSError, EOFError, ValueError, KeyError) as error:
                (a.output / f"invocation-{ordinal}-member-{member}-audit-failure-private.txt").write_text(repr(error))
                summaries[-1]["members"].append(dict(completed=False, integrityVerified=False,
                    sha256=native.sha(source) if source.exists() else None, failure="missing-or-invalid-archive"))
                members.append(([], [], []))
                headers.append(None)
                continue
            headers.append(header)
            prefix = a.output / f"invocation-{ordinal}-member-{member}"
            exported = prefix.with_suffix(".input.jsonl")
            rows = prefix.with_suffix(".rows.jsonl")
            native.export(source, exported)
            with exported.open("rb") as inp, rows.open("wb") as out, prefix.with_suffix(".stderr.txt").open("wb") as err:
                process = subprocess.run([str(a.binary.resolve())], stdin=inp, stdout=out, stderr=err)
            if process.returncode:
                summary.update(completed=False, failure="native-replay-rejected", nativeExitCode=process.returncode)
                summaries[-1]["members"].append(summary)
                members.append(([], [], []))
                continue
            anchors, peer_rows, audits = [], [], []
            with rows.open(encoding="utf-8") as stream:
                for text in stream:
                    r = json.loads(text)
                    if r.get("kind") == "audit":
                        audits.append(r)
                    elif r["id"] == identities.get(r["connection"]):
                        anchors.append(dict(r, received=times[r["line"]], kind="anchor"))
                    else:
                        peer_rows.append(dict(r, received=times[r["line"]]))
            summary.update(nativeAudit=audits, ownAnchors=len(anchors))
            summaries[-1]["members"].append(summary)
            members.append((events, anchors, peer_rows))
            if native.sha(source) != summary["sha256"]:
                raise ValueError("source changed during replay")
            exported.unlink()
        provenance = [h["captureProvenance"] if h else None for h in headers]
        if all(provenance) and (provenance[0]["captureBatchId"] != provenance[1]["captureBatchId"] or provenance[0]["timeOrigin"] != provenance[1]["timeOrigin"]):
            for summary in summaries[-1]["members"]:
                summary.update(completed=False, failure="paired-source-clock-or-batch-mismatch")
        events, anchors, _ = members[0]
        peer_rows = members[1][2]
        # Match only positively received corresponding anchors. No absence claim,
        # clock-epoch fitting or inference from a missing peer cache is permitted.
        by_id = {}
        for row in peer_rows:
            by_id.setdefault(row["id"], []).append(row)
        for row in anchors:
            candidates = [r for r in by_id.get(row["id"], []) if abs(r["received"] - row["received"]) <= 300
                and sum((r["anchor"][i] - row["anchor"][i]) ** 2 for i in (0, 1)) <= 8.000001]
            row["observerAgreement"] = len(candidates) == 1 and candidates[0]["wire_count"] == row["wire_count"]
        combined = sorted(events + anchors, key=lambda r: (r["line"], r.get("packet", -1)))
        all_events.append(dict(ordinal=ordinal, events=combined))
    selected_input = a.output / "selection-input.json"
    native.dump(selected_input, all_events)
    result = json.loads(subprocess.check_output(["node", str(HERE / "score.mjs"), str(selected_input)], text=True))
    for invocation, summary in zip(result["invocations"], summaries, strict=True):
        invocation["archiveQualified"] = all(m["completed"] and m["integrityVerified"] for m in summary["members"])
        invocation["qualified"] &= invocation["archiveQualified"]
    # Apply archive qualification before the final predeclared decision.
    decision_input = a.output / "decision-input.json"
    native.dump(decision_input, result["invocations"])
    result["decision"] = subprocess.check_output(["node", "--input-type=module", "-e",
        "import fs from 'node:fs'; import {decision} from './selection.mjs'; "
        "console.log(decision(JSON.parse(fs.readFileSync(process.argv[1]))));", str(decision_input.resolve())], cwd=HERE, text=True).strip()
    for s in summaries:
        # Error strings can include private operational details; retain privately.
        acquisition = s["acquisition"]
        s["acquisition"] = {k: acquisition[k] for k in ("reason", "elapsedSeconds", "socketsClosed", "maxCounts", "controlsDone", "interruptionCounts")}
    result.update(schemaVersion=1, freezeCommit="fd8962419cd762952160e86942f694ac2390de7b",
        clientRevision="8d0772c015d6b9b73b80253db8c7349aa600b201", summaries=summaries,
        nativeBinarySha256=native.sha(a.binary),
        sourceSha256={str(f.relative_to(HERE)): native.sha(f) for f in HERE.iterdir() if f.suffix in (".py", ".mjs", ".ps1")})
    native.dump(a.output / "selected-results.json", result)
    print(json.dumps(dict(decision=result["decision"], cells=[list(r["cells"]) for r in result["invocations"]])))


if __name__ == "__main__":
    main()
