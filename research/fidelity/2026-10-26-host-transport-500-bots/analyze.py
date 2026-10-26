"""Recompute selected transport results from the six retained raw captures.

Usage: python analyze.py EXTERNAL_RESULTS NEW_OUTPUT_DIRECTORY
Does not contact servers, fit parameters, mask samples or change tool gates.
"""

import argparse
from contextlib import redirect_stdout
import copy
from datetime import datetime, timezone
import hashlib
import io
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/fidelity"))
import capture_quality as cq
import slither_protocol as sp
import transport_parity as tp


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8", newline="\n")


def gates(report, reference, tolerances):
    rows = []
    for name, (path, unit) in tp.GATES.items():
        npath = path.rsplit(".", 1)[0] + ".n"
        values = [tp.lookup(c["summary"], path) for c in reference["captures"]
                  if tp.lookup(c["summary"], npath) >= tolerances["minimumSamples"]]
        low, high = min(values), max(values)
        config = tolerances["metrics"][name]
        limit = high + config["spreadAllowance"] * max(high - low, config.get("spreadFloor", 0))
        value, n = tp.lookup(report["summary"], path), tp.lookup(report["summary"], npath)
        rows.append(dict(metric=name, unit=unit, n=n, value=value, upperLimit=limit,
                         passed=n >= tolerances["minimumSamples"] and value is not None and value <= limit))
    buffer = io.StringIO()
    with redirect_stdout(buffer):
        code = tp.compare(reference, report, tolerances)
    assert code == int(not all(row["passed"] for row in rows))
    return dict(passed=code == 0, gates=rows)


def portable(report):
    result = copy.deepcopy(report)
    for capture in result["captures"]:
        # The native reports contain numeric endpoint addresses. Selected
        # summaries identify archives by portable relative path and digest.
        capture.pop("servers", None)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    reference = json.loads(tp.DEFAULT_REFERENCE.read_text())
    tolerances = json.loads(tp.DEFAULT_TOLERANCES.read_text())
    complete = json.loads((args.raw / "complete.json").read_text())
    first_heldout = min(row["startedUnix"] for row in complete["attempts"] if row["split"] == "heldout")
    seal_time = (args.raw / "development-seal.json").stat().st_mtime
    assert seal_time <= first_heldout, "development seal postdates heldout acquisition"
    study = Path(__file__).resolve().parent
    assert complete["planSha256"] == digest(study / "plan.json")
    assert complete["continuationPlanSha256"] == digest(study / "second-continuation-plan.json")
    pins = json.loads((study / "source-pins.json").read_text())
    for pin_file in ("source-pins.json", "continuation-source-pins.json", "second-continuation-source-pins.json"):
        for name, expected in json.loads((study / pin_file).read_text())["files"].items():
            assert digest(ROOT / name) == expected, name
    results = dict(dataOrigin="local-simulator", scientificStatus="unreviewed",
                   planSha256=complete["planSha256"], baseCommit=pins["baseCommit"],
                   continuationPlanSha256=complete["continuationPlanSha256"],
                   referenceSha256=digest(tp.DEFAULT_REFERENCE),
                   tolerancesSha256=digest(tp.DEFAULT_TOLERANCES), arms=[],
                   attempts=[dict(attempt=row["attempt"], split=row["split"], exitCode=row["exitCode"],
                             elapsedSeconds=row["finishedUnix"] - row["startedUnix"])
                             for row in complete["attempts"]],
                   acquisitionFailure=dict(attempt="bots-500-r1", exitCode=2,
                       error="[Errno 98] Address already in use", rawCaptureProduced=False,
                       handling="Initial sequence stopped; retained continuation plans describe port and launcher corrections."),
                   launcherFailure=dict(attempt="bots-500-r1-retry", exitCode=2,
                       error="[Errno 2] No such file or directory: 'cargo'", rawCaptureProduced=False),
                   heldoutSeal=dict(sealSha256=digest(args.raw / "development-seal.json"),
                       sealFileTimeUtc=datetime.fromtimestamp(seal_time, timezone.utc).isoformat(),
                       firstHeldoutStartedUtc=datetime.fromtimestamp(first_heldout, timezone.utc).isoformat(),
                       limit="Filesystem time corroborates runner order; it is not a trusted external timestamp."))
    inventory = []
    for bots in (0, 500):
        names = [f"bots-{bots}-r{r}" + ("-retry2" if bots == 500 and r == 1 else "") for r in (1, 2, 3)]
        paths = [args.raw / name / "raw/bot0.jsonl.gz" for name in names]
        report = tp.measure(paths, summary_only=True, relative_to=args.raw)
        original = json.loads((args.raw / f"pooled-{bots}.json").read_text())
        assert report == original, "independent offline reproduction differs"
        per_capture = []
        for repetition, (name, path, capture) in enumerate(zip(names, paths, report["captures"]), 1):
            audit = cq.audit_capture(path)
            populations = []
            for frame in sp.read(path):
                if isinstance(frame, sp.Frame):
                    for packet in frame.packets:
                        if packet[:1] == b"l":
                            if len(packet) < 6:
                                raise ValueError("short leaderboard")
                            populations.append(int.from_bytes(packet[4:6], "big"))
            pin = json.loads((args.raw / f"{name}-pins.json").read_text())
            assert len({tuple(cpu) for cpu in pin.values()}) == len(pin)
            config = args.raw / f"{name}/host-config.json"
            row = dict(attempt=name,
                       split="heldout" if repetition == 3 else "development",
                       **gates(dict(report, summary=capture["summary"]), reference, tolerances),
                       summary=capture["summary"], collectorTiming=capture["collectorTiming"],
                       connectionSetupEpochs=capture["connectionSetupEpochs"], affinity=pin,
                       leaderboardPopulation=dict(tp.quantiles(populations),
                           minimum=min(populations, default=None), maximum=max(populations, default=None)),
                       archive=dict(path=capture["path"], sha256=capture["sha256"],
                                    bytes=path.stat().st_size, access="local-only"),
                       config=json.loads(config.read_text()), configSha256=digest(config),
                       audit={k: audit[k] for k in ("status", "issues", "warnings", "integrityVerified",
                              "completeGzip", "provenanceValid", "payloadSha256", "connections", "joins")})
            assert row["audit"]["status"] == "verified", row["audit"]
            per_capture.append(row)
            inventory.append(row["archive"])
        development = json.loads((args.raw / f"development-{bots}.json").read_text())
        seal = json.loads((args.raw / "development-seal.json").read_text())
        assert seal[f"development-{bots}.json"] == digest(args.raw / f"development-{bots}.json")
        results["arms"].append(dict(backgroundBots=bots, captures=per_capture,
            development=gates(development, reference, tolerances),
            pooled=gates(report, reference, tolerances), pooledSummary=report["summary"]))
        write(args.out / f"transport-{bots}.json", portable(report))
    write(args.out / "results.json", results)
    write(args.out / "raw-manifest.json", dict(dataOrigin="local-simulator", access="local-only",
          scientificStatus="unreviewed", planSha256=complete["planSha256"], files=inventory))
    # Runtime commands can contain private absolute paths, so publish the
    # schema-compatible ledger with portable reproduction commands instead.
    for source, target in (("attempts.jsonl", "initial-attempts.jsonl"),
                           ("continuation-attempts.jsonl", "continuation-attempts.jsonl"),
                           ("second-continuation-attempts.jsonl", "second-continuation-attempts.jsonl")):
        ledger = []
        for line in (args.raw / source).read_text().splitlines():
            row = json.loads(line)
            if row["event"] == "started":
                name = row["attemptId"]
                row["command"] = ["python", "tools/fidelity/local_capture.py", "--config", f"{name}.json",
                                  "--seconds", "300", "--out", name]
            ledger.append(json.dumps(row))
        (args.out / target).write_text("\n".join(ledger) + "\n", encoding="utf-8", newline="\n")
    write(args.out / "development-seal.json", json.loads((args.raw / "development-seal.json").read_text()))
    print("Reproduced both pooled summaries exactly; audited all six archives and preserved every gate.")


if __name__ == "__main__":
    main()
