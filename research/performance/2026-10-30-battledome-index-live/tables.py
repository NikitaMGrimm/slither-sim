"""Use #275's table renderer unchanged, keeping every repeat as its own row."""
from pathlib import Path

prior = Path(__file__).resolve().parents[1] / "2026-10-29-battledome-post-spawn/tables.py"
exec(compile(prior.read_text(), str(prior), "exec"),
     {"__file__": str(prior), "__name__": "__main__"})
