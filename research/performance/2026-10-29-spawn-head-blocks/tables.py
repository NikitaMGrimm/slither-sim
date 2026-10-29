"""Recompute fixed-work and separate construction tables from selected JSON."""
from pathlib import Path
import json
import statistics
import sys

prior = Path(__file__).resolve().parents[1] / "2026-10-29-spawn-head-distance/tables.py"
exec(compile(prior.read_text(), str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
data = json.loads(Path(sys.argv[1]).with_name("construction.json").read_text())
print("\n| Row / protocol | Whole two-Safety CPU median base → candidate µs | Paired CPU ratio median [range] | Candidate blocks alone / one Safety µs |")
print("| --- | ---: | ---: | ---: |")
for row, protocols in data.items():
    for protocol, selected in protocols.items():
        pairs = selected["pairs"][1:]
        key = "cpuNsPerTwoSafetiesMedian"
        cpu = " → ".join(f"{statistics.median(p[a][key] for p in pairs)/1000:.3f}" for a in ["base", "candidate"])
        ratios = [p["candidate"][key]/p["base"][key] for p in pairs]
        blocks = selected["candidateBlocksOnly"]["cpuNsPerSafetyMedian"]/1000
        print(f"| {row} / {protocol} | {cpu} | {statistics.median(ratios):.3f} [{min(ratios):.3f}, {max(ratios):.3f}] | {blocks:.3f} |")
