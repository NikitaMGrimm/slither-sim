"""Reduce the sparse-path revision's owner-local receipts (stdlib only)."""
import argparse
from collections import Counter
from datetime import datetime
import json
from pathlib import Path
import re
from statistics import median

from reduce_runs import read_csv, require, sha, write_csv, quantile, selected


def observation(root, name):
    path = root / f"{name}-processes.json"
    observations = json.loads(path.read_text(encoding="utf-8-sig"))
    if isinstance(observations, dict):
        observations = [observations]
    return datetime.fromisoformat(observations[0]["utc"]), dict(
        processes_max=max(len(o["processes"]) for o in observations),
        browsers_max=max(sum(p["ProcessName"].lower() in {"chrome", "msedge", "firefox"} for p in o["processes"]) for o in observations),
        node_max=max(sum(p["ProcessName"].lower() == "node" for p in o["processes"]) for o in observations),
        process_log_sha256=sha(path),
    )


def reduce(root, output, suite):
    receipts = list(root.glob(f"{suite}-*-receipt.json"))
    require(len(receipts) == (50 if suite == "revision" else 10), "incomplete revision journal")
    ordered = []
    for path in receipts:
        receipt = json.loads(path.read_text(encoding="utf-8-sig"))
        name = receipt["id"]
        require(receipt["exit"] == 0, "failed revision run")
        log = (root / f"{name}.log").read_text(encoding="utf-8-sig")
        require("complete=true" in log, "incomplete revision run")
        for token in ("device: 5056", "device_type: IntegratedGpu", "backend: Dx12", 'driver: "32.0.21001.9024"'):
            require(token in log, "revision adapter differs")
        start, host = observation(root, name)
        ordered.append((start, receipt, host, path))
    ordered.sort(key=lambda r: r[0])
    rows = []
    for index, (start, receipt, host, path) in enumerate(ordered, 1):
        name = receipt["id"]
        raw = read_csv(root / f"{name}.csv")
        base = dict(pair=receipt["pair"], side=receipt["side"], launch_id=name,
                    launch_index=index, relative_start_s=f"{(start-ordered[0][0]).total_seconds():.6f}",
                    binary_sha256=receipt["binarySha256"].lower(),
                    exit=receipt["exit"], compile_overlap=int(receipt["compileOverlap"]),
                    load_min=receipt["loadMin"], load_max=receipt["loadMax"], **host,
                    raw_csv_sha256=sha(root / f"{name}.csv"), receipt_sha256=sha(path))
        if suite == "revision":
            cell = selected(raw, "history", receipt["workload"])
            complete = re.search(r"complete=true,messages=(\d+),digest=([0-9a-f]{16}),skips=(\d+)",
                                 (root / f"{name}.log").read_text(encoding="utf-8-sig"))
            require(complete is not None and complete[3] == "0", "incomplete/skipped history")
            workload = receipt["workload"]
            require(int(complete[1]) == (32106 if workload == "replay" else 0), "incomplete history input")
            require(len(cell) == (302 if workload == "replay" else 141), "incomplete selected callbacks")
            require(len({r["frame"] for r in cell}) == len(cell), "repeated history callbacks")
            require(len({r["frame"] for r in raw}) == len(raw) and all(r["repeat"] == "0" for r in raw), "repeated raw callbacks")
            row = dict(run_id=name, workload=receipt["workload"], zoom=receipt["zoom"], size=receipt["size"], **base)
            row["samples"] = len(cell)
            row.update(messages=int(complete[1]), skips=int(complete[3]),
                       callbacks=len(raw), attachment_pixels=cell[0]["attachment_pixels"],
                       world_samples=cell[0]["samples"])
            for metric in ("gpu", "sync", "submit"):
                values = [float(r[metric + "_ns"]) / 1e6 for r in cell]
                row[metric + "_median_ms"] = f"{median(values):.9f}"
                row[metric + "_p95_ms"] = f"{quantile(values,.95):.9f}"
            for key in ("draws", "body_quads", "outline_quads", "glow_quads", "other_quads"):
                row[key + "_peak"] = max(int(r[key]) for r in cell)
            for key in ("body", "outline", "glow", "other"):
                row[key + "_screen_area_peak"] = f"{max(float(r[key + '_screen_area']) for r in cell):.9f}"
            rows.append(row)
            continue
        expected_cases = ("toggle",) if suite == "cache" else ("body", "low-quality", "one-glow")
        samples = 90 if suite == "cache" else 30
        require(Counter((r["size"], r["case"], r["iteration"]) for r in raw) == Counter((size, case, str(i)) for size in ("1920-original", "1920-native", "3840-native") for case in expected_cases for i in range(samples)), "incomplete sparse/lifecycle samples")
        for size in ("1920-original", "1920-native", "3840-native"):
            for case in expected_cases:
                cell = [r for r in raw if (r["size"], r["case"]) == (size, case)]
                row = dict(run_id=f"{suite}-{receipt['pair']}-{size}-{case}-{receipt['side']}", size=size, case=case, samples=len(cell), **base)
                for metric in ("gpu", "submit"):
                    values = [float(r[metric + "_ns"]) / 1e6 for r in cell]
                    row[metric + "_median_ms"] = f"{median(values):.9f}"
                    row[metric + "_p95_ms"] = f"{quantile(values,.95):.9f}"
                for key in ("passes", "target_bytes"):
                    row[key + "_min"] = min(int(r[key]) for r in cell)
                    row[key + "_max"] = max(int(r[key]) for r in cell)
                row["world_samples"] = cell[0]["samples"]
                rows.append(row)
    write_csv(output / (suite + "-runs.csv"), rows)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("suite", choices=("sparse", "cache", "revision"))
    parser.add_argument("--output", type=Path, default=Path(__file__).parent)
    args = parser.parse_args()
    reduce(args.raw, args.output, args.suite)
    print("Reduced complete revision run journals; identifying process details remain local.")
