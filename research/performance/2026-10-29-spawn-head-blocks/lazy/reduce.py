"""Reduce both comparisons from the same three-arm windows on the builder."""
from pathlib import Path
import sys

prior = Path(__file__).resolve().parents[2] / "2026-10-29-spawn-ring-prefilter/tables.py"
root, row = Path(sys.argv[1]), sys.argv[2]
text = prior.read_text()
text = text.replace("'cpuMs':{", "'cpuMeanMs':statistics.mean(cpu),'cpuMs':{")
exec(compile(text, str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
selected = root / f"selected-fixed-{row}.json"
main = selected.read_bytes()
assert text.count("for arm in ['base','candidate']:") == 1
text = text.replace("for arm in ['base','candidate']:", "for arm in ['eager','candidate']:")
text = text.replace("result[arm]={}", "result['base' if arm=='eager' else arm]={}")
text = text.replace("result[arm][name]", "result['base' if arm=='eager' else arm][name]")
exec(compile(text, str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
(root / f"selected-eager-fixed-{row}.json").write_bytes(selected.read_bytes())
selected.write_bytes(main)
