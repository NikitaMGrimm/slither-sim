"""Recompute final main/lazy tables, successful-search acceptance and eager arm."""
from pathlib import Path
import json
import statistics as stats
import sys

prior = Path(__file__).resolve().parents[1] / "tables.py"
exec(compile(prior.read_text(), str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
root = Path(sys.argv[1]).parent
data = json.loads((root / "measurements.json").read_text())
eager = json.loads((root / "eager-measurements.json").read_text())
construction = json.loads((root / "construction.json").read_text())
print("\n| Successful row / protocol | Main / eager / lazy mean CPU/search µs, median [window ranges] | Lazy/main CPU ratio median [range] | Lazy/main search instructions change |")
print("| --- | ---: | ---: | ---: |")
for row in ["seats80", "seats160"]:
    for protocol, selected in data[row].items():
        pairs = selected["pairs"][1:]
        values = {a: [p[a]["search"]["cpuMeanMs"]*1000 for p in pairs] for a in ["base", "candidate"]}
        values["eager"] = [p["base"]["search"]["cpuMeanMs"]*1000 for p in eager[row][protocol]["pairs"][1:]]
        cells = " / ".join(f"{stats.median(v):.3f} [{min(v):.3f}, {max(v):.3f}]" for v in [values[a] for a in ["base", "eager", "candidate"]])
        ratios = [p["candidate"]["search"]["cpuMeanMs"]/p["base"]["search"]["cpuMeanMs"] for p in pairs]
        instruction = selected["pairedInstructions"]["search"]
        print(f"| {row} / {protocol} | {cells} | {stats.median(ratios):.4f} [{min(ratios):.4f}, {max(ratios):.4f}] | {(instruction['ratioMedian']-1)*100:+.3f}% |")
print("\n| Row / protocol | Eager → lazy total instructions | Eager → lazy CPU p99 ms | Whole two-Safety build CPU main / eager / lazy µs |")
print("| --- | ---: | ---: | ---: |")
for row, protocols in eager.items():
    for protocol, selected in protocols.items():
        pairs = selected["pairs"][1:]
        ratio = selected["pairedInstructions"]["all"]["ratioMedian"]
        cpu = " → ".join(f"{stats.median(p[a]['all']['cpuMs']['p99'] for p in pairs):.3f}" for a in ["base", "candidate"])
        builds = construction[row][protocol]["pairs"][1:]
        build = " / ".join(f"{stats.median(p[a]['cpuNsPerTwoSafetiesMedian'] for p in builds)/1000:.3f}" for a in ["base", "eager", "candidate"])
        print(f"| {row} / {protocol} | {(ratio-1)*100:+.3f}% | {cpu} | {build} |")
