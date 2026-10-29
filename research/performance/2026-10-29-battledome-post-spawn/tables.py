"""Reuse #267's table renderer unchanged for the new rows."""
from pathlib import Path

prior = Path(__file__).resolve().parents[1] / "2026-10-29-battledome-live-recheck/tables.py"
exec(compile(prior.read_text(), str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
