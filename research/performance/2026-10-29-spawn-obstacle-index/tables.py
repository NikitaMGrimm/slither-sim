"""Recompute report tables from bounded selected measurements (standard library)."""
from pathlib import Path
import json
import statistics as stats
import sys

root = Path(__file__).resolve().parent
data = json.loads((root / "measurements.json").read_text(encoding="utf8"))
lines = ["| Row | Protocol label | Mean CPU, base → candidate (ms) | Paired CPU ratio median [range] | Instructions ratio | Median tick delta (µs) | Gate |",
         "| --- | --- | --- | --- | --- | --- | --- |"]
for row in data["selected"]["rows"]:
    means = [stats.median(p[a]["all"]["cpuNs"]["mean"] for p in row["pairs"]) / 1e6
             for a in ["base", "candidate"]]
    ratio = row["cpuRatio"]
    lines.append(f'| {row["row"]} | {row["protocolLabel"]} | {means[0]:.4f} → {means[1]:.4f} | '
                 f'{ratio["median"]:.4f} [{ratio["min"]:.4f}, {ratio["max"]:.4f}] | '
                 f'{row["instructionsRatio"]["median"]:.4f} | {row["p50DeltaNs"]/1000:+.3f} | '
                 f'{"pass" if row["gatePass"] else "FAIL"} |')
lines += ["", "| Row | Protocol label | All-tick p50 (ms) | All-tick p99 (ms) | Slowest 1% mean CPU (ms) | Search-tick mean CPU (ms) |",
          "| --- | --- | --- | --- | --- | --- |"]
for row in data["selected"]["rows"]:
    def field(group, key):
        values = []
        for arm in ["base", "candidate"]:
            groups = [p[arm].get(group, {}) for p in row["pairs"]]
            if not all(g.get("n", 0) for g in groups):
                return "—"
            values.append(stats.median(g["cpuNs"][key] for g in groups) / 1e6)
        return f"{values[0]:.4f} → {values[1]:.4f}"
    lines.append(f'| {row["row"]} | {row["protocolLabel"]} | {field("all", "p50")} | '
                 f'{field("all", "p99")} | {field("slowestCpu1Percent", "mean")} | {field("search", "mean")} |')
text = "\n".join(lines) + "\n"
if len(sys.argv) > 1 and sys.argv[1] == "--check":
    assert (root / "TABLES.md").read_text(encoding="utf8") == text
else:
    (root / "TABLES.md").write_text(text, encoding="utf8", newline="\n")
