"""Recompute the report's summaries from its selected pair/window aggregates."""
import json
from pathlib import Path
from statistics import median

data = json.loads(Path(__file__).with_name("results.json").read_text(encoding="utf-8"))
for case, row in data["fixed"]["rows"].items():
    print(case)
    for phase in ["all", "maintenance", "ordinary"]:
        pairs = row["pairs"]
        reductions = [
            100 * (1 - p["candidate"][phase]["instructionsMean"] / p["base"][phase]["instructionsMean"])
            for p in pairs
        ]
        print(
            phase,
            "instructions/tick:",
            *[median(p[a][phase]["instructionsMean"] for p in pairs) for a in ["base", "candidate"]],
            "reduction %:", median(reductions), "range:", min(reductions), max(reductions),
            "CPU ms/tick:",
            *[median(p[a][phase]["cpuMsMean"] for p in pairs) for a in ["base", "candidate"]],
            "CPU p99 ms:",
            *[median(p[a][phase]["cpuMsP99"] for p in pairs) for a in ["base", "candidate"]],
        )
if "live" in data:
    for enabled in [False, True]:
        windows = [w for w in data["live"]["windows"] if w["cacheEnabled"] == enabled and not w["fine"]]
        print("live cache", enabled)
        for phase, metric in [("maintenance", "worldCpuMsMean"), ("ordinary", "worldCpuMsMean"), ("all", "worldCpuMsP99")]:
            values = [w[phase][metric] for w in windows]
            print(phase, metric, "median", median(values), "range", min(values), max(values))
    print("remaining fine diagnostics:", json.dumps(data["live"]["fine"], indent=2))
