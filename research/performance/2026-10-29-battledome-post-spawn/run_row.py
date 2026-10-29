"""Reuse #267's fresh ladder and unchanged qualification, allowing 640 conditionally."""
from pathlib import Path

r = Path(__file__).resolve().parent
prior = r / "shared267/run_row.py"
text = prior.read_text()
assert text.count('target in [160,320,480]') == 1
text = text.replace('target in [160,320,480]', 'target in [160,320,480,640]')
text = text.replace('[x for x in [80,160,320,480] if x<=target]', '[x for x in [80,160,320,480,640] if x<=target]')
exec(compile(text, str(prior), "exec"), {"__file__": str(Path(__file__)), "__name__": "__main__"})
