"""Recompute both socket count tables; earlier study data stays in its owner."""
import json
from pathlib import Path

r = Path(__file__).resolve().parent
prior = json.loads((r.parent / "2026-10-29-food-diffs" / "results.json").read_text(encoding="utf-8"))["live"]["fine"]
current = json.loads((r / "socket-results.json").read_text(encoding="utf-8"))
for phase in ["maintenance", "ordinary"]:
    row = prior[phase]
    print("500 historical", phase, row["ticks"], row["perRecipientMean"])
    row = current["phases"][phase]
    print("8 direct syscall witness", phase, row["ticks"], row["perRecipientTick"])
