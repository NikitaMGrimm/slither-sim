"""Print bounded fixed-work comparisons from measurements.json."""
from pathlib import Path
import json
import statistics
import sys


def med(values):
    return statistics.median(values)


def main():
    data = json.loads(Path(sys.argv[1]).read_text())
    print("| Row | Protocol | Instructions change median [range] | CPU p50 base → candidate ms | CPU p99 base → candidate ms | CPU max base → candidate ms |")
    print("| --- | --- | ---: | ---: | ---: | ---: |")
    for name, protocols in data.items():
        for protocol, selected in protocols.items():
            pairs = selected["pairs"][1:]
            ratio = selected["pairedInstructions"]["all"]
            cells = []
            for q in ["p50", "p99", "max"]:
                cells.append(" → ".join(f"{med([p[a]['all']['cpuMs'][q] for p in pairs]):.3f}" for a in ["base", "candidate"]))
            print(f"| {name} | {protocol} | {(ratio['ratioMedian']-1)*100:+.2f}% [{(ratio['ratioMin']-1)*100:+.2f}, {(ratio['ratioMax']-1)*100:+.2f}] | " + " | ".join(cells) + " |")
    print("\n| Row / protocol | Group | Instructions change median [range] | CPU p99 base → candidate ms |")
    print("| --- | --- | ---: | ---: |")
    for name, protocols in data.items():
        for protocol, selected in protocols.items():
            pairs = selected["pairs"][1:]
            for group in ["search", "exhausted", "other", "slowestCpu1Percent"]:
                ratio = selected["pairedInstructions"].get(group)
                if ratio is None:
                    continue
                cpu = " → ".join(f"{med([p[a][group]['cpuMs']['p99'] for p in pairs]):.3f}" for a in ["base", "candidate"])
                print(f"| {name} / {protocol} | {group} | {(ratio['ratioMedian']-1)*100:+.2f}% [{(ratio['ratioMin']-1)*100:+.2f}, {(ratio['ratioMax']-1)*100:+.2f}] | {cpu} |")
    print("\n| Row / protocol | CPU p50 range base / candidate ms | CPU p99 range base / candidate ms |")
    print("| --- | ---: | ---: |")
    for name, protocols in data.items():
        for protocol, selected in protocols.items():
            pairs = selected["pairs"][1:]
            cells = []
            for q in ["p50", "p99"]:
                cells.append(" / ".join(
                    f"{min(p[a]['all']['cpuMs'][q] for p in pairs):.3f}–{max(p[a]['all']['cpuMs'][q] for p in pairs):.3f}"
                    for a in ["base", "candidate"]))
            print(f"| {name} / {protocol} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main()
