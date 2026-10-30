"""Reuse #275's archive/pin-bound assembler; retain its source pin explicitly."""
from pathlib import Path
import hashlib
import json
import sys
import tarfile
import importlib.util

diagnostics_path = Path(__file__).with_name("timing_diagnostics.py")
spec = importlib.util.spec_from_file_location("timing_diagnostics", diagnostics_path)
diagnostics = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostics)

prior = Path(__file__).resolve().parents[1] / "2026-10-29-battledome-post-spawn/assemble_results.py"
exec(compile(prior.read_text(), str(prior), "exec"),
     {"__file__": str(Path(__file__)), "__name__": "__main__"})
path = Path(sys.argv[2])
value = json.loads(path.read_text())
value["pins"]["analysisSha256"]["inheritedAssembler"] = hashlib.sha256(prior.read_bytes()).hexdigest()
value["pins"]["analysisSha256"]["inheritedRuntimeWrapper"] = hashlib.sha256(
    prior.with_name("select_results.py").read_bytes()).hexdigest()
value["pins"]["analysisSha256"]["timingDiagnostics"] = hashlib.sha256(diagnostics_path.read_bytes()).hexdigest()
root = Path(sys.argv[1])
value["holds"] = json.loads((root / "hold-gates.json").read_text())
assert len(value["holds"]) == len(value["rows"])
by_label = {hold["label"]: hold for hold in value["holds"]}
assert len(by_label) == len(value["holds"])
for row in value["rows"]:
    hold = by_label[row["label"]]
    assert (hold["arm"], hold["seats"]) == (row["arm"], row["primary"]["bots"])
    assert hold["sourceArchiveSha256"] == row["sourceArchiveSha256"]
    assert hold["canonicalDeferredTicks"] == row["primary"]["deferredTicks"]
    with tarfile.open(root / "rows" / row["label"] / "results.tar.gz") as archive:
        with archive.extractfile("run-summary.json") as stream:
            summary = json.load(stream)
        with archive.extractfile("host-run/host.log") as stream:
            census = [json.loads(line[7:]) for line in stream
                      if line.startswith(b"CENSUS ") and line.endswith(b"\n")]
        with archive.extractfile("host-run/host.log") as stream:
            row["timingDiagnostics"] = diagnostics.analyze(stream, row["primary"]["tickRange"])
        assert row["timingDiagnostics"]["deferralBatches"]["totalDeferredTicks"] == row["primary"]["deferredTicks"]
    marked = [w for w in summary["windows"] if w["bots"] == hold["seats"]][-1]
    assert hold["originalOnlineDeferredTicks"] == marked["result"]["deferredTicks"]
    before = [v for v in census if v["unix"] <= marked["start"]["unix"]]
    after = [v for v in census if v["unix"] >= marked["end"]["unix"]]
    covered = bool(before and after)
    enclosing = after[0]["deferred"] - before[-1]["deferred"] if covered else None
    assert hold["boundaryCoverageComplete"] == covered
    assert hold["enclosingCensusDeferredTicks"] == enclosing
    assert hold["holds"] == (covered and enclosing == 0
                             and hold["originalOnlineDeferredTicks"] == 0
                             and hold["canonicalDeferredTicks"] == 0)
path.write_text(json.dumps(value, indent=2) + "\n", newline="\n")
