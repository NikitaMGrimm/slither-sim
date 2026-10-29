"""Reuse the prior fixed-work table calculation with this study's data."""
from pathlib import Path

prior = Path(__file__).resolve().parent.parent / "2026-10-29-food-diffs" / "tables.py"
exec(compile(prior.read_text(encoding="utf-8"), str(prior), "exec"), {"__file__": __file__})
