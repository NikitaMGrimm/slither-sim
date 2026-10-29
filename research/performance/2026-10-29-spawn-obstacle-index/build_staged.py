"""Extend the earlier builder by reference with the portable live-admission binary."""
from pathlib import Path
import hashlib
import json
import sys

prior = Path(__file__).resolve().parents[1] / "2026-10-29-spawn-ring-prefilter/build_staged.py"
text = prior.read_text()
text = text.replace("RUSTFLAGS='-C target-cpu=native'", "RUSTFLAGS=''")
text = text.replace("CARGO_BUILD_JOBS='1'", "CARGO_BUILD_JOBS='4'")
text = text.replace("'--offline','-j','1'", "'--offline','-j','4'")
text = text.replace("'--example','perf_baseline']", "'--example','perf_baseline','--example','study_live']")
text = text.replace("['study_fixed','study_protocols','perf_baseline']", "['study_fixed','study_protocols','perf_baseline','study_live']")
exec(compile(text, str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
pins = Path(sys.argv[1]) / sys.argv[2] / "pins.json"
value = json.loads(pins.read_text())
value["liveBuilderSha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
pins.write_text(json.dumps(value, indent=2) + "\n", newline="\n")
