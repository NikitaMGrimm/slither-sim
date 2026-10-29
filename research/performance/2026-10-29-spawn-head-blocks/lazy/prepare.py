"""Apply the original portable/construction overlay unchanged to the lazy arms."""
from pathlib import Path

prior = Path(__file__).resolve().parents[1] / "prepare.py"
exec(compile(prior.read_text(), str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
