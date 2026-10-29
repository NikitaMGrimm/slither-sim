"""Reuse the verified staged builder with the default portable release flags."""
from pathlib import Path

prior = Path(__file__).resolve().parents[1] / "2026-10-29-spawn-ring-prefilter/build_staged.py"
text = prior.read_text()
old = "RUSTFLAGS='-C target-cpu=native'"
assert text.count(old) == 1
text = text.replace(old, "RUSTFLAGS=''")
text = text.replace("'--example','perf_baseline']", "'--example','perf_baseline','--example','study_build']")
text = text.replace("['study_fixed','study_protocols','perf_baseline']", "['study_fixed','study_protocols','perf_baseline','study_build']")
exec(compile(text, str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
