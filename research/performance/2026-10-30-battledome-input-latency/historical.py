"""Post-hoc #281 diagnostics from its verified, unchanged canonical intervals.

Run after native counting: python3 historical.py HISTORICAL_DIRECTORY OUTPUT.
Histograms cannot recover unrecorded per-frame packet or kernel timestamps.
"""
from pathlib import Path
import hashlib
import json
import re
import sys
import tarfile

from reduce import stats


def main():
    root, output = Path(sys.argv[1]), Path(sys.argv[2])
    selected = json.loads((root / "selected-results.json").read_text())
    result = []
    for label in ["01-base-320", "02-candidate-320", "05-candidate-320", "06-base-320"]:
        row = next(row for row in selected["rows"] if row["label"] == label)
        path = root / f"{label}.tar.gz"
        receipt = json.loads((root / f"{label}-receipt.json").read_text())
        assert receipt["status"] == "complete" and receipt["requiredSourcesVerified"]
        assert path.stat().st_size == receipt["archiveBytes"]
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        assert digest == receipt["archiveSha256"] == row["sourceArchiveSha256"]
        with tarfile.open(path) as archive:
            names = archive.getnames()
            client_name = "load-run/client.log"
            assert client_name in names, names[:20]
            with archive.extractfile("host-run/host.log") as stream:
                census = [json.loads(line[7:]) for line in stream if line.startswith(b"CENSUS ")]
            first, last = row["primary"]["tickRange"]
            cs = [value for value in census if first <= value["tick"] <= last]
            assert cs[0]["tick"] == first and cs[-1]["tick"] == last
            start, end = cs[0]["unix"], cs[-1]["unix"]
            clients, before_lines, opened = [], {}, set()
            with archive.extractfile(client_name) as stream:
                for line in stream:
                    if not line.startswith(b"CLIENT ") or not line.endswith(b"\n"):
                        continue
                    at = float(re.search(rb'"unix"\s*:\s*([-+0-9.eE]+)', line)[1])
                    worker = int(re.search(rb'"worker"\s*:\s*(\d+)', line)[1])
                    if at < start:
                        before_lines[worker] = line
                    elif at <= end:
                        if worker not in opened:
                            if worker in before_lines:
                                clients.append(json.loads(before_lines[worker][7:]))
                            opened.add(worker)
                        clients.append(json.loads(line[7:]))
        previous = {}
        reports = []
        for value in clients:
            worker = value["worker"]
            before = previous.get(worker)
            previous[worker] = value
            if before is None or before["unix"] < start or value["unix"] > end:
                continue
            hist = value.get("gapResidualMicros", [])
            excess = value.get("frameExcessMicros", [])
            reports.append({"secondsFromStart": value["unix"] - start, "worker": worker,
                            "maxPassMs": value["maxPassNanos"] / 1e6,
                            "lateP99Ms": value["lateP99Nanos"] / 1e6,
                            "lateMaxMs": value["lateMaxNanos"] / 1e6,
                            "cpuFraction": value["cpuFraction"],
                            "residualMaxMs": max((key for key, count in hist if count), default=0) / 1000,
                            "excessMaxMs": max((key for key, count in excess if count), default=0) / 1000,
                            "residualStalls20": sum(count for key, count in hist if key > 20000),
                            "joins": value["joins"] - before["joins"],
                            "deaths": value["deaths"] - before["deaths"]})
        host_lag = [(value["unix"] - start) * 1000 - (value["tick"] - first) * 7 for value in cs]
        minimum = min(host_lag)
        current_lag = [lag - (value["deferred"] - cs[0]["deferred"]) * 7
                       for lag, value in zip(host_lag, cs)]
        current_minimum = min(current_lag)
        high = [value for value in reports if value["residualStalls20"]]
        result.append({
            "label": label, "sourceArchiveSha256": digest, "canonicalTickRange": [first, last],
            "originalDeferredTicks": row["primary"]["deferredTicks"],
            "originalFrameExcessMs": row["primary"]["client"]["frameExcessMs"],
            "originalResidualStalls": row["primary"]["client"]["residualStalls"],
            "originalBusyWallMs": row["timingDiagnostics"]["busyWallMs"],
            "censusWallMinusTickRelativeMs": stats([value - minimum for value in host_lag]),
            "censusWallMinusTickAndDeferredRelativeMs": stats([value - current_minimum for value in current_lag]),
            "workerMaxPassMs": stats([value["maxPassMs"] for value in reports]),
            "reportsWithResidualStalls20": len(high),
            "stallReportsWithPassOver20Ms": sum(value["maxPassMs"] > 20 for value in high),
            "stallReportsWithLifecycleActivity": sum(value["joins"] > 0 or value["deaths"] > 0 for value in high),
            "largestTwelveWorkerPasses": sorted(reports, key=lambda value: value["maxPassMs"], reverse=True)[:12],
        })
    output.write_text(json.dumps({"rows": result}, indent=2) + "\n")
    print("All four historical 320 canonical intervals audited; no windows reselected")


if __name__ == "__main__":
    main()
