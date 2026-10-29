"""Reuse #267's archive-bound canonical selector with the two new runtime pins."""
from pathlib import Path

prior = Path(__file__).resolve().parents[1] / "2026-10-29-battledome-live-recheck/select_results.py"
text = prior.read_text()
text = text.replace('cc1bb1e9935094a00cf513413d7922d858dd5c1e', '3a223676_PIN')
text = text.replace('539b37f3be969827b5eac67c2978e604f9e814e8', 'a1535902_PIN')
# The invocation supplies full resolved hashes in an adjacent public pins file.
import json
pins = json.loads(Path(__file__).with_name("runtimes.json").read_text())
text = text.replace('3a223676_PIN', pins['base']).replace('a1535902_PIN', pins['candidate'])
exec(compile(text, str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
