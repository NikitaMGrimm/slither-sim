"""Reduce owner-local journals into non-identifying summaries (stdlib only)."""
import argparse
from collections import Counter
import csv
from datetime import datetime
import hashlib
import json
import math
from pathlib import Path
import re
from statistics import median

PASS_NAMES = ("world", "world-before-glows", "world-after-glows", "food-outlines",
              "food-glows-coarse", "food-glows-fine", "food-glows-half", "blit", "overlay")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def quantile(values, fraction):
    values = sorted(values)
    require(bool(values), "empty sample selection")
    return values[max(0, math.ceil(len(values) * fraction) - 1)]


def read_csv(path):
    with path.open(encoding="utf-8-sig", newline="") as stream:
        return list(csv.DictReader(stream))


def write_csv(path, rows):
    require(bool(rows), f"no rows for {path.name}")
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def selected(rows, series, workload):
    if series != "history":
        return rows
    if workload == "replay":
        return [r for r in rows if float(r["model_ms"]) >= 1000 and int(r["received_foods"]) >= 10000]
    return [r for r in rows if int(r["frame"]) >= 40]


def summarize(root, receipt_path, series):
    receipt = json.loads(receipt_path.read_text(encoding="utf-8-sig"))
    name = receipt["id"]
    raw_path = root / f"{name}.csv"
    log_path = root / f"{name}.log"
    processes_path = root / f"{name}-processes.json"
    rows = selected(read_csv(raw_path), series, receipt["workload"])
    require(receipt["exit"] == 0, f"failed run {name}")
    log = log_path.read_text(encoding="utf-8-sig")
    complete = re.search(r"complete=true,messages=(\d+),digest=([0-9a-f]{16}),skips=(\d+)", log)
    require(complete is not None, f"missing completion {name}")
    observations = json.loads(processes_path.read_text(encoding="utf-8-sig"))
    if isinstance(observations, dict):
        observations = [observations]
    start = datetime.fromisoformat(observations[0]["utc"])
    result = dict(
        run_id=name, pair=receipt["pair"], workload=receipt["workload"],
        zoom=receipt["zoom"], size=receipt["size"], side=receipt["variant"],
        binary_sha256=receipt.get("binarySha256", "" ).lower(),
        launch_index=0, relative_start_s=0, samples=len(rows),
        callbacks=len({r["frame"] for r in rows}),
        exit=receipt["exit"], messages=int(complete[1]), skips=int(complete[3]),
        elapsed_s=receipt["wallSeconds"], cpu_s=receipt["cpuSeconds"],
        compile_overlap=int(receipt["compileOverlap"]),
        load_min=receipt["loadMin"], load_max=receipt["loadMax"],
    )
    categories = {
        "processes": lambda _: True,
        "cargo": lambda p: p == "cargo", "rustc": lambda p: p == "rustc",
        "link": lambda p: p in {"link", "lld-link", "rust-lld"},
        "node": lambda p: p == "node", "python": lambda p: "python" in p,
        "tests": lambda p: "test" in p,
        "browsers": lambda p: p in {"chrome", "msedge", "firefox"},
    }
    for key, accepts in categories.items():
        result[key + "_max"] = max(sum(accepts(p["ProcessName"].lower()) for p in o["processes"]) for o in observations)
    for key in ("gpu_ns", "sync_ns", "submit_ns"):
        values = [float(r[key]) / 1e6 for r in rows]
        result[key.replace("_ns", "_median_ms")] = f"{median(values):.9f}"
        result[key.replace("_ns", "_p95_ms")] = f"{quantile(values, .95):.9f}"
        result[key.replace("_ns", "_max_ms")] = f"{max(values):.9f}"
    for key in ("draws", "calls", "body_quads", "outline_quads", "glow_quads", "other_quads"):
        result[key + "_peak"] = max(int(r[key]) for r in rows)
    for key in ("body", "outline", "glow", "other"):
        result[key + "_screen_area_peak"] = f"{max(float(r[key + '_screen_area']) for r in rows):.12f}"
    result["attachment_pixels"] = rows[0]["attachment_pixels"]
    result["world_samples"] = rows[0]["samples"]
    require(all(r["attachment_pixels"] == result["attachment_pixels"] and r["samples"] == result["world_samples"] for r in rows), f"mixed targets {name}")
    result["raw_csv_sha256"] = sha(raw_path)
    result["process_log_sha256"] = sha(processes_path)
    result["receipt_sha256"] = sha(receipt_path)
    return start, result


def export_series(root, output, series):
    prefix = "primary" if series == "frozen" else series
    receipts = list(root.glob(f"{prefix}-*-receipt.json"))
    require(len(receipts) == 120, f"{series}: expected 120 completed runs, got {len(receipts)}")
    ordered = sorted(summarize(root, p, series) for p in receipts)
    first = ordered[0][0]
    rows = []
    for index, (start, row) in enumerate(ordered, 1):
        row["launch_index"] = index
        row["relative_start_s"] = f"{(start-first).total_seconds():.6f}"
        rows.append(row)
    write_csv(output / f"{series}-runs.csv", rows)


def export_diagnostics(root, output):
    summaries = []
    snapshots = []
    for path in sorted(root.glob("diagnostic-1-*-receipt.json")):
        receipt = json.loads(path.read_text(encoding="utf-8-sig"))
        name = receipt["id"]
        rows = read_csv(root / f"{name}.csv")
        for frame in sorted({r["frame"] for r in rows}, key=int):
            cell = [r for r in rows if r["frame"] == frame]
            first = cell[0]
            values = [float(r["gpu_ns"]) / 1e6 for r in cell]
            snapshots.append(dict(
                run_id=name, workload=receipt["workload"], zoom=receipt["zoom"],
                variant=receipt["variant"], frame=frame, model_ms=first["model_ms"],
                samples=len(cell), gpu_median_ms=f"{median(values):.9f}",
                gpu_p95_ms=f"{quantile(values, .95):.9f}",
                draws=first["draws"], attachment_pixels=first["attachment_pixels"],
                world_samples=first["samples"],
                **{key: first[key] for key in ("body_quads", "outline_quads", "glow_quads", "other_quads", "body_screen_area", "outline_screen_area", "glow_screen_area", "other_screen_area")},
            ))
        values = [float(r["gpu_ns"]) / 1e6 for r in rows]
        summaries.append(dict(
            run_id=name, workload=receipt["workload"], zoom=receipt["zoom"], variant=receipt["variant"],
            exit=receipt["exit"], samples=len(rows), gpu_median_ms=f"{median(values):.9f}",
            gpu_p95_ms=f"{quantile(values,.95):.9f}", compile_overlap=int(receipt["compileOverlap"]),
            load_min=receipt["loadMin"], load_max=receipt["loadMax"], raw_csv_sha256=sha(root/f"{name}.csv"),
        ))
    require(len(summaries) == 16 and len(snapshots) == 48, "incomplete diagnostic matrix")
    write_csv(output / "diagnostics.csv", summaries)
    write_csv(output / "diagnostic-snapshots.csv", snapshots)


def export_parity(root, output):
    runs = []
    pictures = []
    for path in sorted(root.glob("parity-*-receipt.json")):
        receipt = json.loads(path.read_text(encoding="utf-8-sig"))
        name = receipt["id"]
        log = (root / f"{name}.log").read_text(encoding="utf-8-sig")
        complete = re.search(r"complete=true,messages=(\d+),digest=([0-9a-f]{16}),skips=(\d+)", log)
        require(receipt["exit"] == 0 and complete is not None, f"incomplete parity run {name}")
        runs.append(dict(run_id=name, workload=receipt["workload"], zoom=receipt["zoom"],
                         size=receipt["size"], side=receipt["variant"],
                         binary_sha256=receipt["binarySha256"].lower(),
                         messages=complete[1], digest=complete[2], skips=complete[3],
                         raw_csv_sha256=sha(root / f"{name}.csv"), receipt_sha256=sha(path)))
        if receipt["variant"] != "after":
            continue
        width = 960 if receipt["size"].startswith("960") else 1920
        height = width * 9 // 16
        frames = (60, 120, 180) if receipt["workload"] == "synthetic" else (120, 240, 365)
        for frame in frames:
            after = root / f"{name}.{frame}.rgba"
            before = root / f"{name.removesuffix('after')}before.{frame}.rgba"
            a, b = before.read_bytes(), after.read_bytes()
            require(len(a) == len(b) == width * height * 4, "incorrect readback size")
            histogram = Counter()
            signed = 0
            squared = 0
            alpha_max = 0
            for offset in range(0, len(a), 4):
                alpha_max = max(alpha_max, abs(a[offset + 3] - b[offset + 3]))
                for channel in range(3):
                    delta = b[offset + channel] - a[offset + channel]
                    histogram[abs(delta)] += 1
                    signed += delta
                    squared += delta * delta
            count = width * height * 3
            cumulative = 0
            p95 = 0
            for difference, frequency in sorted(histogram.items()):
                cumulative += frequency
                if cumulative >= math.ceil(count * .95):
                    p95 = difference
                    break
            pictures.append(dict(workload=receipt["workload"], zoom=receipt["zoom"], size=receipt["size"],
                                 frame=frame, rgb_components=count, rgb_nonzero=count - histogram[0],
                                 rgb_max=max(histogram), rgb_p95=p95,
                                 rgb_rms=f"{math.sqrt(squared/count):.9f}", rgb_signed_mean=f"{signed/count:.9f}",
                                 alpha_max=alpha_max, before_rgba_sha256=sha(before), after_rgba_sha256=sha(after)))
    require(len(runs) == 24 and len(pictures) == 36, "incomplete parity/readback matrix")
    write_csv(output / "parity-runs.csv", runs)
    write_csv(output / "pixels.csv", pictures)


def export_profiles(root, output):
    results = []
    binary = sha(root / "candidate-full.exe")
    for path in sorted(root.glob("profile-1-*-after.log")):
        match = re.fullmatch(r"profile-1-(replay|synthetic)-(0|-120)-1920-native-after", path.stem)
        require(match is not None, "unexpected profile id")
        workload, zoom = match.groups()
        text = path.read_text(encoding="utf-8-sig")
        require(f"complete=true,messages={32106 if workload == 'replay' else 0}," in text and "skips=0" in text, "incomplete profile")
        raw = read_csv(path.with_suffix(".csv"))
        frames = ("120", "240", "365") if workload == "replay" else ("60", "120", "180")
        require(Counter(r["frame"] for r in raw) == Counter({f: 10 for f in frames}), "incomplete profile snapshots")
        require(all(r["attachment_pixels"] == "2073600" and r["samples"] == "4" for r in raw), "incorrect profile target")
        current = {stage: [] for stage in PASS_NAMES}
        for line in text.splitlines():
            if line.startswith("pass "):
                stage, value = line.removeprefix("pass ").split("=")
                require(stage in current, "unknown GPU pass")
                current[stage].append(float(value) / 1e6)
            elif line.startswith("snapshot="):
                frame = line.split(",")[0].split("=")[1]
                require(any(current.values()), "missing per-pass timestamps")
                totals = [float(r["gpu_ns"]) / 1e6 for r in raw if r["frame"] == frame]
                for i, total in enumerate(totals):
                    observed = sum(values[5 + i] for values in current.values() if values)
                    # Printed pass values round to nanoseconds; at most nine
                    # half-nanosecond decisions contribute to the total.
                    require(abs(observed - total) <= 0.00001, "incomplete per-pass timestamp sum")
                for stage, values in current.items():
                    require(len(values) in (0, 15), "incomplete profile repetitions")
                    values = values[5:] if values else [0.] * 10
                    results.append(dict(run_id=path.stem, workload=workload, zoom=zoom,
                                        frame=frame, stage=stage, samples=len(values),
                                        mean_ms=f"{sum(values)/len(values):.9f}",
                                        binary_sha256=binary, raw_log_sha256=sha(path),
                                        raw_csv_sha256=sha(path.with_suffix(".csv"))))
                current = {stage: [] for stage in PASS_NAMES}
        require(not any(current.values()), "unassigned profile samples")
    require(len(results) == 108, "incomplete four-cell profile")
    write_csv(output / "profile-stages.csv", results)


def export_integration(root, output):
    rows = []
    for path in sorted(root.glob("integration-1-*-receipt.json")):
        receipt = json.loads(path.read_text(encoding="utf-8-sig"))
        name, workload = receipt["id"], receipt["workload"]
        log = (root / f"{name}.log").read_text(encoding="utf-8-sig")
        complete = re.search(r"complete=true,messages=(\d+),digest=([0-9a-f]{16}),skips=(\d+)", log)
        require(receipt["exit"] == 0 and complete is not None, "incomplete integration run")
        selected_rows = selected(read_csv(root / f"{name}.csv"), "history", workload)
        frames = (120, 240, 365) if workload == "replay" else (60, 120, 180)
        rows.append(dict(run_id=name, workload=workload, zoom=receipt["zoom"], size=receipt["size"],
                         samples=len(selected_rows), messages=complete[1], digest=complete[2], skips=complete[3],
                         rgba_sha256s=";".join(sha(root / f"{name}.{f}.rgba") for f in frames),
                         binary_sha256=receipt["binarySha256"].lower(),
                         raw_csv_sha256=sha(root / f"{name}.csv"), receipt_sha256=sha(path)))
    require(len(rows) == 12, "incomplete post-rebase matrix")
    write_csv(output / "integration-runs.csv", rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("--output", type=Path, default=Path(__file__).parent)
    parser.add_argument("--series", choices=("history", "frozen", "diagnostics", "parity", "profile", "integration", "all"), default="all")
    args = parser.parse_args()
    require(args.output.is_dir(), "output directory must already exist")
    for series in ("history", "frozen"):
        if args.series in (series, "all"):
            export_series(args.raw, args.output, series)
    if args.series in ("diagnostics", "all"):
        export_diagnostics(args.raw, args.output)
    if args.series == "parity":
        export_parity(args.raw, args.output)
    if args.series == "profile":
        export_profiles(args.raw, args.output)
    if args.series == "integration":
        export_integration(args.raw, args.output)
    print("Reduced all requested journals; no packet, identity, position or process-path fields exported.")


if __name__ == "__main__":
    main()
