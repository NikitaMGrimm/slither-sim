"""Reuse the prior fixed-work table calculation with this study's data."""
from pathlib import Path

prior = Path(__file__).resolve().parent.parent / "2026-10-29-food-diffs" / "tables.py"
context = {"__file__": __file__}
exec(compile(prior.read_text(encoding="utf-8"), str(prior), "exec"), context)

# The earlier calculator covers the primary row. Keep rejected comparisons
# separate, including their different baseline and incomplete row matrix.
median = context["median"]
for prototype, data in context["data"]["rejected"].items():
    for case, row in data["rows"].items():
        for phase in ["maintenance", "ordinary", "all"]:
            pairs = row["pairs"]
            changes = [100 * (p["candidate"][phase]["instructionsMean"] /
                             p["base"][phase]["instructionsMean"] - 1) for p in pairs]
            print(prototype, case, phase, "instruction change %:", median(changes),
                  "range:", min(changes), max(changes), "CPU mean ms:",
                  *[median(p[a][phase]["cpuMsMean"] for p in pairs) for a in ["base", "candidate"]],
                  "CPU p99 ms:",
                  *[median(p[a][phase]["cpuMsP99"] for p in pairs) for a in ["base", "candidate"]])
