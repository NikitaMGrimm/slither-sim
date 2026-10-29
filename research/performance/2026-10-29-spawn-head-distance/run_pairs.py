"""Reuse the frozen-host pair runner; add 80/160-seat reconnect controls.

Usage: run_pairs.py OUTPUT PRIOR_INVENTORY PROTOCOL ROW
Run only after write_gates.py has verified the complete proof receipts.
"""
from pathlib import Path
import sys

prior = Path(__file__).resolve().parents[1] / "2026-10-29-spawn-ring-prefilter/run_pairs.py"
text = prior.read_text()
old = "row in ['spawn','minimap','markers']"
assert text.count(old) == 1
text = text.replace(old, "row in ['spawn','seats80','seats160']")
old = "env=dict(os.environ,RAYON_NUM_THREADS='1'))"
assert text.count(old) == 1
text = text.replace(old, "env=dict(os.environ,RAYON_NUM_THREADS='1',STUDY_SEATS='160' if row=='seats160' else '80'))")
old = "            assert s['enabledNs']==0 or s['runningNs']/s['enabledNs']>=.99"
assert text.count(old) == 1
# An actual queued reconnect is inserted outside the counted interval, then the
# original admission work runs inside it. It must finish within that same tick.
text = text.replace("assert tick_row['waitingBefore']==tick_row['waitingAfter']==expected",
                    "assert (tick_row['waitingBefore']==tick_row['waitingAfter']==expected) if row=='spawn' else (tick_row['waitingBefore']<=1 and tick_row['waitingAfter']==0)")
exec(compile(text, str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
