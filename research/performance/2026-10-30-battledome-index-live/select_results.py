"""Reuse the archive-bound #275 selector with this study's adjacent runtimes."""
from pathlib import Path

prior = Path(__file__).resolve().parents[1] / "2026-10-29-battledome-post-spawn/select_results.py"
exec(compile(prior.read_text(), str(prior), "exec"),
     {"__file__": str(Path(__file__)), "__name__": "__main__"})
