"""Reuse #275's renderer with repeat labels and the frozen hold/three-way audit."""
from pathlib import Path
import json
import sys

prior = Path(__file__).resolve().parents[1] / "2026-10-29-battledome-live-recheck/tables.py"
text = prior.read_text().replace("{row['arm']} |", "{row['arm']} ({row['order']}) |")
text = text.replace('r["arm"] != "base"', 'r["order"]')
text = text.replace("| Seats | Arm |", "| Seats | Arm (order) |")
text = text.replace("| both arms | Not measured |", "| candidate only | Not measured |")
text = text.replace("Outside all marked windows", "Outside counted interiors")
exec(compile(text, str(prior), "exec"),
     {"__file__": str(prior), "__name__": "__main__"})

data = json.loads(Path(sys.argv[1]).read_text())
print("\n| Order | Arm / seats | Original online deferred | Canonical deferred | Enclosing census deferred | Holds |")
print("| ---: | --- | ---: | ---: | ---: | --- |")
holds = {hold["label"]: hold for hold in data["holds"]}
for row in data["rows"]:
    hold = holds[row["label"]]
    print(f"| {row['order']} | {row['arm']} / {hold['seats']} | "
          f"{hold['originalOnlineDeferredTicks']} | {hold['canonicalDeferredTicks']} | "
          f"{hold['enclosingCensusDeferredTicks']} | {hold['holds']} |")
print("\n| Order | Arm / seats | Slow-tail spawn % | Sockets+broadcast % | Other including minimap % | Socket CPU ms | Broadcast excluding minimap ms |")
print("| ---: | --- | ---: | ---: | ---: | ---: | ---: |")
for row in data["rows"]:
    t = row["primary"]["slowestCpu1Percent"]
    p = t["exclusivePercent"]
    print(f"| {row['order']} | {row['arm']} / {row['primary']['bots']} | "
          f"{p['spawnSearch']:.2f} | {p['socketsBroadcast']:.2f} | {p['other']+p['minimap']:.2f} | "
          f"{t['phasesMs']['socket']:.3f} | {t['phasesMs']['broadcast']-t['nestedMs']['minimap']:.3f} |")
print("\n| Order | Arm / seats | Busy wall p99 / max ms | Simulation CPU at largest wall tick ms | Census spans with deferrals | Largest span deferred |")
print("| ---: | --- | ---: | ---: | ---: | ---: |")
for row in data["rows"]:
    d = row["timingDiagnostics"]
    batches = d["deferralBatches"]
    largest = batches["largestThree"][0]["deferredTicks"] if batches["n"] else 0
    print(f"| {row['order']} | {row['arm']} / {row['primary']['bots']} | "
          f"{d['busyWallMs']['p99']:.3f} / {d['busyWallMs']['max']:.3f} | "
          f"{d['largestWallTick']['simulationCpuMs']:.3f} | {batches['n']} | {largest} |")
