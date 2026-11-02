"""Recompute all matched tables from bounded per-run evidence (stdlib only)."""
import argparse
import csv
import json
from pathlib import Path
from statistics import median

BEGIN = "<!-- BEGIN RESULTS -->"
END = "<!-- END RESULTS -->"


def render(folder):
    with (folder / "runs.csv").open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    identities = json.loads((folder / "identities.json").read_text(encoding="utf-8"))
    seen = set()
    for row in rows:
        key = (row["pair"], row["view"], row["arm"])
        if key in seen:
            raise ValueError("duplicate run")
        seen.add(key)
        if row["view"] not in ("default", "whole_map") or row["arm"] not in ("control", "candidate"):
            raise ValueError("unknown arm/view")
        if row["binary_sha256"] != identities["parity"][row["view"]][row["arm"]]["binary_sha256"]:
            raise ValueError("binary assignment mismatch")
        if row["numeric_sha256"] != identities["parity"][row["view"]][row["arm"]]["numeric_sha256"]:
            raise ValueError("output mismatch")
        if (row["callbacks"], row["messages"], row["skips"]) != ("600", "29044", "0"):
            raise ValueError("incomplete run")
        expected = ("excluded", "observed_rustc") if int(row["rustc_samples"]) else ("accepted", "none")
        if (row["status"], row["reason"]) != expected:
            raise ValueError("unsupported exclusion")
        for key, value in row.items():
            if key.endswith(("_ns", "_p50", "_p95", "_max", "_total", "_s")) or key == "alloc_bytes":
                if value and float(value) < 0:
                    raise ValueError("negative measurement")
    result = []
    metrics = [
        ("Ten-second processing elapsed", "elapsed_s", 1, "s"),
        ("Process CPU", "cpu_s", 1, "s"),
        ("Requested allocation", "alloc_bytes", 1e9, "GB"),
        ("All callbacks p50", "wall_ns_p50", 1e6, "ms"),
        ("All callbacks p95", "wall_ns_p95", 1e6, "ms"),
        ("Worst callback", "wall_ns_max", 1e6, "ms"),
        ("Sector-refresh callback", "refresh_wall_ns", 1e6, "ms"),
        ("Sector-refresh ingress", "refresh_ingress_ns", 1e6, "ms"),
        ("First two seconds: largest callback", "first_two_max_ns", 1e6, "ms"),
        ("Art interval: callback p50", "art_p50_ns", 1e6, "ms"),
        ("Art interval: callback p95", "art_p95_ns", 1e6, "ms"),
        ("Recovered interval: callback p50", "recovered_p50_ns", 1e6, "ms"),
        ("Art interval: callbacks over 16.7 ms (of 300)", "art_over_budget_callbacks", 1, "callbacks"),
        ("Recovered: callbacks over 16.7 ms (of 120)", "recovered_over_budget_callbacks", 1, "callbacks"),
        ("Longest consecutive over-budget virtual interval", "longest_over_budget_virtual_ms", 1, "ms"),
        ("Complete-history ingress", "ingress_ns_total", 1e9, "s"),
        ("Complete-history redraw", "redraw_ns_total", 1e9, "s"),
        ("Complete-history CPU submit", "submit_ns_total", 1e9, "s"),
        ("Complete-history GPU passes", "gpu_ns_total", 1e9, "s"),
    ]
    for view in ("default", "whole_map"):
        selected = [r for r in rows if r["view"] == view]
        arms = {arm: [r for r in selected if r["arm"] == arm and r["status"] == "accepted"] for arm in ("control", "candidate")}
        if not all(arms.values()):
            raise ValueError("no accepted arm")
        result += [f"**{view}**: accepted control/candidate {len(arms['control'])}/{len(arms['candidate'])}; "
                   f"excluded {sum(r['status'] == 'excluded' for r in selected)}. "
                   f"Observed load {min(float(r['load_min']) for r in selected):.1f}–{max(float(r['load_max']) for r in selected):.1f}%.", "",
                   "| Median of accepted per-run summaries (range) | Before | After |", "| --- | ---: | ---: |"]
        for label, field, scale, unit in metrics:
            cells = []
            for arm in ("control", "candidate"):
                values = [float(r[field]) / scale for r in arms[arm]]
                cells.append(f"{median(values):.3f} ({min(values):.3f}–{max(values):.3f}) {unit}")
            result.append(f"| {label} | " + " | ".join(cells) + " |")
        result += ["", "| Complete pair | Before / after refresh callback | Reduction |", "| --- | ---: | ---: |"]
        changes = []
        for pair in sorted({int(r["pair"]) for r in selected}):
            found = {r["arm"]: r for r in selected if int(r["pair"]) == pair and r["status"] == "accepted"}
            if len(found) == 2:
                before, after = (float(found[a]["refresh_wall_ns"]) / 1e6 for a in ("control", "candidate"))
                change = 100 * (1 - after / before)
                changes.append(change)
                result.append(f"| {pair} | {before:.3f} / {after:.3f} ms | {change:.1f}% |")
        if changes:
            result += ["", f"Paired refresh reduction: median {median(changes):.1f}%, range {min(changes):.1f}–{max(changes):.1f}%."]
        result += [""]
    return "\n".join(result)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    folder = Path(__file__).resolve().parent
    generated = render(folder)
    print(generated)
    if args.check:
        report = (folder / "REPORT.md").read_text(encoding="utf-8")
        if report.split(BEGIN + "\n", 1)[1].split(END, 1)[0] != generated + "\n":
            raise ValueError("report differs from run summaries")
        print("All aggregates match retained summaries.")
