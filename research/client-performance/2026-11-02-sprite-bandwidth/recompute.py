"""Check complete retained datasets and regenerate every report table (stdlib)."""
import argparse
from collections import Counter
from itertools import product
import json
import math
from pathlib import Path
from statistics import median

from reduce_runs import PASS_NAMES, read_csv, require

HERE = Path(__file__).parent
WORKLOADS = ("replay", "synthetic")
ZOOMS = ("0", "-120")
SIZES = ("960-original", "1920-original", "1920-native")
SIDES = ("before", "after")
MODES = ("msaa4", "msaa1", "no-glow", "half-glow")
CELLS = tuple(product(WORKLOADS, ZOOMS, SIZES))


def validate_runs(rows, series, binaries):
    expected = Counter((str(pair), w, z, size, side) for pair in range(1, 6) for w, z, size in CELLS for side in SIDES)
    actual = Counter((r["pair"], r["workload"], r["zoom"], r["size"], r["side"]) for r in rows)
    require(actual == expected, f"{series}: missing, duplicate or extra pair/cell/arm")
    require(len({r["run_id"] for r in rows}) == len(rows), f"{series}: duplicate run id")
    prefix = "primary" if series == "frozen" else "history"
    for row in rows:
        pair, workload, zoom, size, side = (row[k] for k in ("pair", "workload", "zoom", "size", "side"))
        require(row["run_id"] == f"{prefix}-{pair}-{workload}-{zoom}-{size}-{side}", "run id disagrees with its labels")
        require(row["binary_sha256"] == binaries[side], "run bound to the wrong executable")
        require(row["exit"] == row["skips"] == "0", "failed/skipped primary run")
        require(int(row["messages"]) == (32106 if workload == "replay" else 0), "incomplete input")
        expected_callbacks = 3 if series == "frozen" else 302 if workload == "replay" else 141
        require(int(row["callbacks"]) == expected_callbacks, "incomplete callback selection")
        require(int(row["samples"]) == (60 if series == "frozen" else expected_callbacks), "incomplete picture samples")
        require(int(row["attachment_pixels"]) == (2073600 if size == "1920-native" else 1267500), "wrong world attachment")
        require(row["world_samples"] == "4", "unmatched world sample count")
        require(0 <= float(row["load_min"]) <= float(row["load_max"]) <= 100, "invalid host-load observations")
        require(row["compile_overlap"] in ("0", "1"), "missing process observations")
        for key in ("gpu_median_ms", "gpu_p95_ms", "sync_median_ms", "submit_median_ms"):
            require(math.isfinite(float(row[key])) and float(row[key]) >= 0, "invalid timing")
        for key in ("raw_csv_sha256", "process_log_sha256", "receipt_sha256"):
            require(len(row[key]) == 64 and all(c in "0123456789abcdef" for c in row[key]), "missing journal binding")
    require({int(r["launch_index"]) for r in rows} == set(range(1, 121)), "missing/duplicate launch order")
    ordered = sorted(rows, key=lambda r: int(r["launch_index"]))
    starts = [float(r["relative_start_s"]) for r in ordered]
    require(starts == sorted(starts), "launch order is not chronological")
    for pair in range(1, 6):
        for workload, zoom, size in CELLS:
            arms = {r["side"]: r for r in rows if r["pair"] == str(pair) and (r["workload"], r["zoom"], r["size"]) == (workload, zoom, size)}
            a, b = arms["before"], arms["after"]
            order = int(b["launch_index"]) - int(a["launch_index"])
            require(order == (1 if pair % 2 else -1), "arms were not matched/alternating")
            for key in ("samples", "callbacks", "messages", "draws_peak", "body_quads_peak", "outline_quads_peak", "glow_quads_peak", "other_quads_peak", "body_screen_area_peak", "outline_screen_area_peak", "glow_screen_area_peak", "other_screen_area_peak"):
                require(a[key] == b[key], f"{series}: output/coverage differs: {key}")


def cell_label(workload, zoom, size):
    w = "Replay" if workload == "replay" else "Authored"
    view = "default" if zoom == "0" else "whole map"
    physical = "960×540" if size.startswith("960") else "1920×1080"
    policy = "Native" if size.endswith("native") else "Original"
    return f"{w} / {view} / {physical} {policy}"


def result_table(rows):
    lines = [
        "| Workload / view / physical output | Before GPU median / p95 ms | After GPU median / p95 ms | After run-median range ms |",
        "| --- | ---: | ---: | ---: |",
    ]
    for workload, zoom, size in CELLS:
        cell = [r for r in rows if (r["workload"], r["zoom"], r["size"]) == (workload, zoom, size)]
        values = {}
        for side in SIDES:
            arms = [r for r in cell if r["side"] == side]
            values[side] = [median(float(r[k]) for r in arms) for k in ("gpu_median_ms", "gpu_p95_ms")]
        after = [float(r["gpu_median_ms"]) for r in cell if r["side"] == "after"]
        a, b = values["before"], values["after"]
        lines.append(f"| {cell_label(workload, zoom, size)} | {a[0]:.3f} / {a[1]:.3f} | {b[0]:.3f} / {b[1]:.3f} | {min(after):.3f}–{max(after):.3f} |")
    return "\n".join(lines)


def diagnostics_tables(runs, snapshots):
    expected = Counter(product(WORKLOADS, ZOOMS, MODES))
    require(Counter((r["workload"], r["zoom"], r["variant"]) for r in runs) == expected, "incomplete diagnostic run matrix")
    require(len({r["run_id"] for r in runs}) == 16, "duplicate diagnostic run ids")
    expected_snapshots = Counter((w, z, mode, frame) for w, z, mode in expected for frame in (("120", "240", "365") if w == "replay" else ("60", "120", "180")))
    require(Counter((r["workload"], r["zoom"], r["variant"], r["frame"]) for r in snapshots) == expected_snapshots, "incomplete diagnostic snapshots")
    lines = ["| Initial diagnostic / Original | MSAA4 median / p95 ms | MSAA1 median / p95 ms | No food glow median / p95 ms | Half food glow median / p95 ms |", "| --- | ---: | ---: | ---: | ---: |"]
    coverage = ["| Initial maximum glow coverage / Original | Snapshot frame | Food body / outline / glow screen areas | Glow / all food MSAA4 sample-equivalent millions |", "| --- | ---: | ---: | ---: |"]
    for workload, zoom in product(WORKLOADS, ZOOMS):
        cell = {r["variant"]: r for r in runs if (r["workload"], r["zoom"]) == (workload, zoom)}
        values = []
        for mode in MODES:
            row = cell[mode]
            require(row["run_id"] == f"diagnostic-1-{workload}-{zoom}-960-original-{mode}", "diagnostic id/labels disagree")
            require(row["exit"] == "0" and row["samples"] == "30", "incomplete diagnostic")
            values.append(f"{float(row['gpu_median_ms']):.3f} / {float(row['gpu_p95_ms']):.3f}")
        label = cell_label(workload, zoom, "960-original")
        lines.append(f"| {label} | " + " | ".join(values) + " |")
        base = [r for r in snapshots if (r["workload"], r["zoom"], r["variant"]) == (workload, zoom, "msaa4")]
        peak = max(base, key=lambda r: float(r["glow_screen_area"]))
        areas = [float(peak[k + "_screen_area"]) for k in ("body", "outline", "glow")]
        factor = int(peak["attachment_pixels"]) * 4 / 1e6
        coverage.append(f"| {label} | {peak['frame']} | {areas[0]:.3f} / {areas[1]:.3f} / {areas[2]:.3f} | {areas[2]*factor:.3f} / {sum(areas)*factor:.3f} |")
    for row in snapshots:
        require(row["samples"] == "10", "incomplete diagnostic snapshot")
        require(row["world_samples"] == ("1" if row["variant"] == "msaa1" else "4"), "incorrect diagnostic MSAA")
        require(row["run_id"] == f"diagnostic-1-{row['workload']}-{row['zoom']}-960-original-{row['variant']}", "diagnostic snapshot id/labels disagree")
    return "\n".join(lines), "\n".join(coverage)


def host_table(series_rows):
    lines = ["| Series | Runs retained | Observed compilation | CPU-load snapshot range | Maximum observed browser / Node processes |", "| --- | ---: | ---: | ---: | ---: |"]
    for series, rows in series_rows.items():
        lo = min(float(r["load_min"]) for r in rows)
        hi = max(float(r["load_max"]) for r in rows)
        compilers = sum(int(r["compile_overlap"]) for r in rows)
        browsers = max(int(r["browsers_max"]) for r in rows)
        nodes = max(int(r["node_max"]) for r in rows)
        lines.append(f"| {series} | {len(rows)} | {compilers} | {lo:.0f}–{hi:.0f}% | {browsers} / {nodes} |")
    return "\n".join(lines)


def parity_table(runs, pictures, binaries):
    require(Counter((r["workload"], r["zoom"], r["size"], r["side"]) for r in runs)
            == Counter((*cell, side) for cell in CELLS for side in SIDES), "incomplete parity cells")
    require(len({r["run_id"] for r in runs}) == 24, "duplicate parity run ids")
    for r in runs:
        require(r["run_id"] == f"parity-1-{r['workload']}-{r['zoom']}-{r['size']}-{r['side']}", "parity id/labels disagree")
        require(r["binary_sha256"] == binaries[r["side"]], "wrong parity executable")
        require(r["skips"] == "0" and int(r["messages"]) == (32106 if r["workload"] == "replay" else 0), "incomplete parity input")
        require(len(r["digest"]) == 16 and all(c in "0123456789abcdef" for c in r["digest"]), "invalid output digest")
    expected_pictures = Counter((*cell, frame) for cell in CELLS for frame in
                                (("120", "240", "365") if cell[0] == "replay" else ("60", "120", "180")))
    require(Counter((r["workload"], r["zoom"], r["size"], r["frame"]) for r in pictures) == expected_pictures, "incomplete readback cells")
    lines = ["| Picture check / output | Exact core/resource/send digest | RGB max / p95 bytes | RGB RMS range bytes |",
             "| --- | --- | ---: | ---: |"]
    for cell in CELLS:
        pair = [r for r in runs if (r["workload"], r["zoom"], r["size"]) == cell]
        require(len({r["digest"] for r in pair}) == 1, "core/resource/send mismatch")
        images = [r for r in pictures if (r["workload"], r["zoom"], r["size"]) == cell]
        for r in images:
            require(int(r["rgb_components"]) == (960 * 540 * 3 if cell[2].startswith("960") else 1920 * 1080 * 3), "wrong readback dimensions")
            require(0 <= int(r["rgb_nonzero"]) <= int(r["rgb_components"]), "invalid component count")
            require(0 <= int(r["rgb_p95"]) <= int(r["rgb_max"]) <= 255 and r["alpha_max"] == "0", "invalid raster summary")
            for key in ("rgb_rms", "rgb_signed_mean"):
                require(math.isfinite(float(r[key])) and abs(float(r[key])) <= 255, "invalid raster statistic")
            for key in ("before_rgba_sha256", "after_rgba_sha256"):
                require(len(r[key]) == 64 and all(c in "0123456789abcdef" for c in r[key]), "missing readback binding")
        rms = [float(r["rgb_rms"]) for r in images]
        lines.append(f"| {cell_label(*cell)} | `{pair[0]['digest']}` | {max(int(r['rgb_max']) for r in images)} / {max(int(r['rgb_p95']) for r in images)} | {min(rms):.3f}–{max(rms):.3f} |")
    return "\n".join(lines)


def profile_table(rows, binary):
    frames = lambda w: ("120", "240", "365") if w == "replay" else ("60", "120", "180")
    expected = Counter((w, z, frame, stage) for w, z in product(WORKLOADS, ZOOMS)
                       for frame in frames(w) for stage in PASS_NAMES)
    require(Counter((r["workload"], r["zoom"], r["frame"], r["stage"]) for r in rows) == expected, "incomplete GPU-stage profile")
    for r in rows:
        require(r["run_id"] == f"profile-1-{r['workload']}-{r['zoom']}-1920-native-after", "profile id/labels disagree")
        require(r["samples"] == "10" and r["binary_sha256"] == binary, "profile source/samples disagree")
        require(math.isfinite(float(r["mean_ms"])) and float(r["mean_ms"]) >= 0, "invalid profile timing")
        require(len(r["raw_log_sha256"]) == len(r["raw_csv_sha256"]) == 64, "missing profile binding")
    lines = ["| Final Native 1920×1080 diagnostic | Snapshot | World passes incl. composites ms | Glow targets ms | Outline target ms | Blit / HUD ms |",
             "| --- | ---: | ---: | ---: | ---: | ---: |"]
    for w, z in product(WORKLOADS, ZOOMS):
        for frame in frames(w):
            values = {r["stage"]: float(r["mean_ms"]) for r in rows if (r["workload"], r["zoom"], r["frame"]) == (w, z, frame)}
            world = sum(values[k] for k in ("world", "world-before-glows", "world-after-glows"))
            glow = sum(values[k] for k in ("food-glows-coarse", "food-glows-fine", "food-glows-half"))
            lines.append(f"| {cell_label(w,z,'1920-native')} | {frame} | {world:.3f} | {glow:.3f} | {values['food-outlines']:.3f} | {values['blit']:.3f} / {values['overlay']:.3f} |")
    return "\n".join(lines)


def replace_or_check(report, name, value, check):
    start, end = f"<!-- BEGIN {name} -->\n", f"\n<!-- END {name} -->"
    require(report.count(start) == report.count(end) == 1, f"missing/duplicate {name} markers")
    head, rest = report.split(start)
    actual, tail = rest.split(end)
    if check:
        require(actual == value, f"report {name} differs from retained summaries")
    return head + start + value + end + tail


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--write", action="store_true", help="replace only generated report blocks")
    args = parser.parse_args()
    require(not (args.check and args.write), "choose --check or --write")
    provenance = json.loads((HERE / "provenance.json").read_text(encoding="utf-8"))
    series_rows = {}
    blocks = {}
    for series in ("history", "pre-boundary", "frozen"):
        rows = read_csv(HERE / f"{series}-runs.csv")
        validate_runs(rows, "history" if series == "pre-boundary" else series, provenance[series]["binaries"])
        series_rows[series] = rows
        blocks[series.upper() + " RESULTS"] = result_table(rows)
    blocks["DIAGNOSTICS"], blocks["COVERAGE"] = diagnostics_tables(read_csv(HERE / "diagnostics.csv"), read_csv(HERE / "diagnostic-snapshots.csv"))
    blocks["HOST OBSERVATIONS"] = host_table(series_rows)
    blocks["PICTURE CHECKS"] = parity_table(read_csv(HERE / "parity-runs.csv"), read_csv(HERE / "pixels.csv"), provenance["history"]["binaries"])
    blocks["PRE-MIP PICTURE CHECKS"] = parity_table(read_csv(HERE / "pre-mip-parity-runs.csv"), read_csv(HERE / "pre-mip-pixels.csv"), provenance["pre-mip"]["binaries"])
    blocks["FINAL GPU PROFILE"] = profile_table(read_csv(HERE / "profile-stages.csv"), provenance["history"]["binaries"]["after"])
    interrupted = read_csv(HERE / "interrupted-runs.csv")
    require(Counter(r["run_id"] for r in interrupted) == Counter(f"history-1-replay-0-{size}-{side}" for size in SIZES for side in SIDES), "incomplete interrupted series journal")
    for r in interrupted:
        side = r["run_id"].rsplit("-", 1)[1]
        stopped = r["run_id"] == "history-1-replay-0-1920-native-after"
        require(r["binary_sha256"] == provenance["pre-mip"]["binaries"][side], "wrong interrupted-series executable")
        require((r["complete"] == "0") == stopped and (r["exit"] != "0") == stopped, "incorrect interruption journal")
        require(bool(r["raw_csv_sha256"]) != stopped and len(r["receipt_sha256"]) == 64, "missing interrupted run binding")
    report_path = HERE / "REPORT.md"
    report = report_path.read_text(encoding="utf-8")
    for name, value in blocks.items():
        print(name + "\n" + value + "\n")
        report = replace_or_check(report, name, value, args.check)
    if args.write:
        report_path.write_text(report, encoding="utf-8", newline="\n")
    print("PASS: complete paired matrices, executable bindings, selections, interleaving and report blocks.")


if __name__ == "__main__":
    main()
