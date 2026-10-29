"""Reuse #275's archive/pin-bound assembler; retain its source pin explicitly."""
from pathlib import Path
import hashlib
import json
import sys

prior = Path(__file__).resolve().parents[1] / "2026-10-29-battledome-post-spawn/assemble_results.py"
exec(compile(prior.read_text(), str(prior), "exec"),
     {"__file__": str(Path(__file__)), "__name__": "__main__"})
path = Path(sys.argv[2])
value = json.loads(path.read_text())
value["pins"]["analysisSha256"]["inheritedAssembler"] = hashlib.sha256(prior.read_bytes()).hexdigest()
value["pins"]["analysisSha256"]["inheritedRuntimeWrapper"] = hashlib.sha256(
    prior.with_name("select_results.py").read_bytes()).hexdigest()
root = Path(sys.argv[1])
value["holds"] = json.loads((root / "hold-gates.json").read_text())
path.write_text(json.dumps(value, indent=2) + "\n", newline="\n")
