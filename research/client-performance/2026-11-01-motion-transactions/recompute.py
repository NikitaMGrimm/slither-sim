"""Recompute the study's timing tables from bounded per-run summaries (stdlib)."""
import argparse
import csv
import re
import sys
from decimal import Decimal
from pathlib import Path
from statistics import median

ARMS = {
    "cow_control": (
        "0b919c09e3c622c3b206c7067ad5cda379222e0710dbcb87138de63d54aab0fd",
        "3209d873d318b102c19521acaea6f8799a1bee4b",
    ),
    "motion_cow": (
        "e817bd39bf7754876a65fefbe66f12be187440b5f34ba237083187970a058105",
        "ada0f7b2d11d0bb3dc1e5157f8c0dd87cd3b533c",
    ),
}
BEGIN = "<!-- BEGIN RECOMPUTED RUN AGGREGATES -->"
END = "<!-- END RECOMPUTED RUN AGGREGATES -->"


def read_runs(path):
    with path.open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream))
    pairs = {}
    for row in rows:
        pair, arm = int(row["pair"]), row["arm"]
        if not 1 <= pair <= 28 or arm not in ARMS:
            raise ValueError("unexpected pair or arm")
        if (pair, arm) in pairs:
            raise ValueError("duplicate pair/arm")
        if (row["binary_sha256"], row["source_base_commit"]) != ARMS[arm]:
            raise ValueError("binary/source identity does not match arm")
        snapshots, rustc = int(row["process_snapshots"]), int(row["rustc_snapshots"])
        if not 0 <= rustc <= snapshots or snapshots < 2:
            raise ValueError("invalid compiler observation count")
        expected = ("excluded", "observed_rustc") if rustc else ("accepted", "none")
        if (row["status"], row["exclusion_reason"]) != expected:
            raise ValueError("acceptance disagrees with compiler observations")
        if tuple(int(row[k]) for k in ("callbacks", "populated_callbacks", "messages",
                                      "picture_skips", "exit_code")) != (366, 302, 32106, 0, 0):
            raise ValueError("incomplete or different replay workload")
        if row["command_digest"] != "62b9cab3a98e7ac6":
            raise ValueError("command digest mismatch")
        for name in ("frames_csv_sha256", "process_log_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", row[name]):
                raise ValueError("invalid retained-output digest")
        for name, value in row.items():
            if name.endswith(("_ns", "_bytes", "_s", "_percent")):
                number = Decimal(value)
                if not number.is_finite() or number < 0:
                    raise ValueError("invalid numeric summary")
                row[name] = number
        for name in ("callback", "redraw"):
            if row[f"{name}_p50_ns"] > row[f"{name}_p95_ns"]:
                raise ValueError("unordered callback quantiles")
        for name in ("cpu_load_before_percent", "cpu_load_after_percent"):
            if row[name] > 100:
                raise ValueError("invalid CPU load")
        pairs[pair, arm] = row
    if set(pairs) != {(pair, arm) for pair in range(1, 29) for arm in ARMS}:
        raise ValueError("all 28 pairs and both arms are required, including exclusions")
    return pairs


def recompute(pairs):
    accepted = {arm: [r for (pair, a), r in sorted(pairs.items())
                      if a == arm and r["status"] == "accepted"] for arm in ARMS}
    if any(not rows for rows in accepted.values()):
        raise ValueError("each arm needs accepted runs")

    def med(arm, field):
        return median(row[field] for row in accepted[arm])

    def span(arm, field):
        values = [row[field] for row in accepted[arm]]
        return f"{median(values):.3f} s ({min(values):.3f}-{max(values):.3f})"

    def ns(arm, field):
        return med(arm, field) / 1_000_000

    def reduction(field):
        return (1 - med("motion_cow", field) / med("cow_control", field)) * 100

    counts = [len(accepted[arm]) for arm in ARMS]
    excluded = [28 - count for count in counts]
    loads = [r[field] for r in pairs.values()
             for field in ("cpu_load_before_percent", "cpu_load_after_percent")]
    lines = [
        f"Twenty-eight pairs: accepted control/motion **{counts[0]} / {counts[1]}**; "
        f"excluded **{excluded[0]} / {excluded[1]}** for observed rustc.",
        f"CPU-load snapshot range: **{min(loads):.0f}-{max(loads):.0f}%**.",
        "",
        "| Fixed 6.1 s history | COW control | Motion + COW |",
        "| --- | ---: | ---: |",
    ]
    table = [
        ("Launcher elapsed, median (range)", lambda a: span(a, "launcher_elapsed_s")),
        ("Process CPU, median (range)", lambda a: span(a, "process_cpu_s")),
        ("Requested allocation bytes", lambda a: f"{med(a, 'requested_allocation_bytes') / 1_000_000_000:.2f} GB"),
        ("Populated callback wall, median / p95", lambda a: f"{ns(a, 'callback_p50_ns'):.2f} / {ns(a, 'callback_p95_ns'):.2f} ms"),
        ("Populated ingress, p95", lambda a: f"{ns(a, 'ingress_p95_ns'):.2f} ms"),
        ("Populated redraw preparation, median / p95", lambda a: f"{ns(a, 'redraw_p50_ns'):.2f} / {ns(a, 'redraw_p95_ns'):.2f} ms"),
    ]
    for label, fmt in table:
        lines.append(f"| {label} | {fmt('cow_control')} | {fmt('motion_cow')} |")
    lines += [
        "",
        f"Reductions: callback p95 **{reduction('callback_p95_ns'):.1f}%**; "
        f"median launcher elapsed **{reduction('launcher_elapsed_s'):.1f}%**; "
        f"requested allocation bytes **{reduction('requested_allocation_bytes'):.1f}%**.",
        "",
        "| Pair | COW control | Motion + COW | Elapsed reduction |",
        "| --- | ---: | ---: | ---: |",
    ]
    for pair in range(1, 29):
        before, after = (pairs[pair, arm] for arm in ARMS)
        if before["status"] == after["status"] == "accepted":
            b, a = before["launcher_elapsed_s"], after["launcher_elapsed_s"]
            lines.append(f"| {pair} | {b:.3f} s | {a:.3f} s | {(1 - a / b) * 100:.1f}% |")
    lines += [""]
    for arm in ARMS:
        ids = [str(pair) for pair in range(1, 29)
               if pairs[pair, arm]["status"] == "accepted"]
        lines.append(f"Accepted {arm} pairs: {', '.join(ids)}.")
    run = pairs[9, "motion_cow"]
    lines += [
        "",
        f"Motion pair 9: ingress is **{run['slow16_ingress_ns'] / run['slow16_callback_wall_ns'] * 100:.1f}%** "
        "of the slowest 16 populated callbacks' wall time.",
        f"Its largest populated callback has **{run['max_callback_messages']} messages**, "
        f"**{run['max_callback_wall_ns'] / 1_000_000:.1f} ms** wall and "
        f"**{run['max_callback_ingress_ns'] / 1_000_000:.1f} ms** ingress.",
    ]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="also verify REPORT.md's generated block")
    args = parser.parse_args()
    folder = Path(__file__).resolve().parent
    result = recompute(read_runs(folder / "runs.csv"))
    print(result, end="")
    if args.check:
        report = (folder / "REPORT.md").read_text(encoding="utf-8")
        recorded = report.split(BEGIN + "\n", 1)[1].split(END, 1)[0]
        if recorded != result + "\n":
            raise ValueError("REPORT.md aggregate block differs from runs.csv; regenerate it")
        print("REPORT.md aggregate block matches runs.csv.", file=sys.stderr)


if __name__ == "__main__":
    main()
