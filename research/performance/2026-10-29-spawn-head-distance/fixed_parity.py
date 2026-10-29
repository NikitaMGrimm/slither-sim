"""Reuse queued full-byte checks; the wrong refusal control may admit nobody."""
from pathlib import Path

prior = Path(__file__).resolve().parents[1] / "2026-10-29-spawn-ring-prefilter/fixed_parity.py"
text = prior.read_text()
old = "assert summary['releaseSearches']>0 and summary['waiting']<72"
assert text.count(old) == 1
text = text.replace(old, "assert summary['releaseSearches']>0 and (arm=='mutation' or summary['waiting']<72)")
exec(compile(text, str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
