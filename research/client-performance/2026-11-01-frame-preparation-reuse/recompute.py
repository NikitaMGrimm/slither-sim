"""Recompute bounded per-run aggregate tables; Python standard library only."""
import argparse
import csv
from pathlib import Path
from statistics import median


def excluded_baseline_table():
    rows = list(csv.DictReader(Path(__file__).with_name("excluded-diagnostics.csv").open(encoding="utf-8")))
    assert len(rows) == 4
    cells = {(r["workload"], r["zoom"]): r for r in rows}
    assert len(cells) == 4
    lines = [
        "| Workload / view | CPU prepare median / p95 ms | GPU passes median / p95 ms | Peak prepared quads |",
        "| --- | ---: | ---: | ---: |",
    ]
    for workload, label in (("replay", "Restricted replay"), ("synthetic", "Authored 30,000 food, 40 snakes")):
        for zoom, view in (("0", "default"), ("-120", "whole map")):
            row = cells[workload, zoom]
            assert row["exit"] == "0" and row["side"] == "before"
            cpu = [float(row[f"redraw_ns_{q}_ms"]) for q in ("median", "p95")]
            gpu = [float(row[f"gpu_ns_{q}_ms"]) for q in ("median", "p95")]
            lines.append(f"| {label} / {view} | {cpu[0]:.3f} / {cpu[1]:.3f} | {gpu[0]:.3f} / {gpu[1]:.3f} | {int(row['max_draws']):,} |")
    return "\n".join(lines)


def check_excluded_table(report, expected):
    start = "<!-- BEGIN EXCLUDED DIAGNOSTIC BASELINE -->\n"
    end = "\n<!-- END EXCLUDED DIAGNOSTIC BASELINE -->"
    assert report.count(start) == report.count(end) == 1, "missing/duplicate diagnostic table markers"
    actual = report.split(start, 1)[1].split(end, 1)[0]
    assert actual == expected, "excluded diagnostic baseline table differs from retained CSV"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify retained-data controls and the excluded diagnostic report table")
    args = parser.parse_args()
    diagnostic = excluded_baseline_table()
    print("Excluded diagnostic baseline (original retained runs):")
    print(diagnostic)
    if args.check:
        check_excluded_table(Path(__file__).with_name("REPORT.md").read_text(encoding="utf-8"), diagnostic)
    stages = list(csv.DictReader(Path(__file__).with_name("stages.csv").open(encoding="utf-8")))
    assert len(stages) == 96
    print("Diagnostic stages (observer runs, excluded from primary timing):")
    for workload in ("replay", "synthetic"):
        for zoom in ("0", "-120"):
            before = sorted((r for r in stages if r["workload"] == workload and r["zoom"] == zoom and r["side"] == "before"), key=lambda r: float(r["median_ms"]), reverse=True)
            after = {r["stage"]: r for r in stages if r["workload"] == workload and r["zoom"] == zoom and r["side"] == "after"}
            print(f"  {workload}/{zoom}: before median/p95 -> after median/p95 ms")
            for row in before:
                other = after[row["stage"]]
                print(f"    {row['stage']}: {float(row['median_ms']):.4f}/{float(row['p95_ms']):.4f} -> {float(other['median_ms']):.4f}/{float(other['p95_ms']):.4f}")
    rows = list(csv.DictReader(Path(__file__).with_name("runs.csv").open(encoding="utf-8")))
    print("All retained primary runs are included, including observed compilation.")
    for workload in ("replay", "synthetic"):
        for zoom in ("0", "-120"):
            cell = [r for r in rows if r["workload"] == workload and r["zoom"] == zoom]
            if not cell:
                continue
            print(f"\n{workload} / {'default' if zoom == '0' else 'whole-map'}")
            for side in ("before", "after"):
                runs = [r for r in cell if r["side"] == side]
                assert len(runs) == 5, (workload, zoom, side, len(runs))
                assert all(r["exit"] == "0" and r["complete"] == "True" and r["skips"] == "0" for r in runs)
                print(f"{side}: n={len(runs)}, observed compilation={sum(r['compileOverlap'] == 'True' for r in runs)}")
                for key in ("redraw_ns", "cpu_supply_ns", "wall_ns", "sync_ns", "submit_ns", "gpu_ns"):
                    p50 = [float(r[key + "_median_ms"]) for r in runs]
                    p95 = [float(r[key + "_p95_ms"]) for r in runs]
                    print(f"  {key}: median/p95 {median(p50):.4f}/{median(p95):.4f} ms; run median range {min(p50):.4f}..{max(p50):.4f}")
                print(f"  requested allocation bytes: {median(int(r['alloc_bytes']) for r in runs):.0f}")
            deltas = []
            for pair in range(1, 6):
                both = [r for r in cell if r["pair"] == str(pair)]
                assert len(both) == 2
                for key in ("messages", "max_foods", "max_snakes", "max_draws", "frames", "command_digest"):
                    assert both[0][key] == both[1][key], (workload, zoom, pair, key)
                a = next(r for r in both if r["side"] == "before")
                b = next(r for r in both if r["side"] == "after")
                deltas.append(float(a["redraw_ns_median_ms"]) - float(b["redraw_ns_median_ms"]))
            before = median(float(r["redraw_ns_median_ms"]) for r in cell if r["side"] == "before")
            after = median(float(r["redraw_ns_median_ms"]) for r in cell if r["side"] == "after")
            print(f"  preparation reduction: {(1-after/before)*100:.2f}%")
            print(f"  paired median savings: {median(deltas):.4f} ms, range {min(deltas):.4f}..{max(deltas):.4f}; lower in {sum(d > 0 for d in deltas)}/5 pairs")
    loads = [float(r[k]) for r in rows for k in ("loadMin", "loadMax")]
    print(f"\nCPU-load snapshot range: {min(loads):.0f}..{max(loads):.0f}%")
    if args.check:
        print("PASS: retained-data controls and excluded diagnostic table match.")


if __name__ == "__main__":
    main()
