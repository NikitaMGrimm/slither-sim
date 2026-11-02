"""Validate and summarize the sparse-path follow-up series (stdlib only)."""
from collections import Counter
from itertools import product
import math
from statistics import median

from reduce_runs import require

SIZES = ("1920-original", "1920-native", "3840-native")
CASES = ("body", "low-quality", "one-glow")
HEADLINES = (("replay", "0", "960-original"), ("replay", "0", "1920-original"),
             ("replay", "0", "1920-native"), ("synthetic", "0", "1920-native"),
             ("synthetic", "-120", "1920-native"))


def validate(rows, suite, binaries):
    sides = {"sparse": ("reviewed", "fixed"), "cache": ("fixed", "retire"),
             "revision": ("before", "after")}[suite]
    cells = HEADLINES if suite == "revision" else tuple(product(SIZES, ("toggle",) if suite == "cache" else CASES))
    fields = ("workload", "zoom", "size") if suite == "revision" else ("size", "case")
    expected = Counter((str(p), *cell, side) for p in range(1, 6) for cell in cells for side in sides)
    require(Counter((r["pair"], *(r[k] for k in fields), r["side"]) for r in rows) == expected,
            f"{suite}: missing, duplicate or extra pair/cell/arm")
    require(len({r["run_id"] for r in rows}) == len(rows), f"{suite}: duplicate run id")
    launches = {}
    for r in rows:
        labels = "-".join(r[k] for k in fields)
        require(r["run_id"] == f"{suite}-{r['pair']}-{labels}-{r['side']}", "revision id/labels disagree")
        launch_id = r["run_id"] if suite == "revision" else f"{suite}-{r['pair']}-{r['side']}"
        require(r["launch_id"] == launch_id, "revision launch id/labels disagree")
        require(r["binary_sha256"] == binaries[r["side"]], "wrong revision executable")
        require(r["exit"] == "0", "failed revision run")
        require(0 <= float(r["load_min"]) <= float(r["load_max"]) <= 100, "invalid revision load")
        require(r["compile_overlap"] in ("0", "1"), "missing revision process observations")
        for key in ("raw_csv_sha256", "process_log_sha256", "receipt_sha256"):
            require(len(r[key]) == 64 and all(c in "0123456789abcdef" for c in r[key]), "missing revision journal binding")
        for key, value in r.items():
            if key.endswith("_ms"):
                require(math.isfinite(float(value)) and float(value) >= 0, "invalid revision timing")
        for metric in ("gpu", "submit"):
            require(float(r[metric + "_p95_ms"]) >= float(r[metric + "_median_ms"]), "invalid revision quantiles")
        observation = tuple(r[k] for k in ("launch_index", "relative_start_s", "binary_sha256", "exit", "load_min", "load_max",
                                           "compile_overlap", "processes_max", "browsers_max", "node_max",
                                           "raw_csv_sha256", "process_log_sha256", "receipt_sha256"))
        require(launch_id not in launches or launches[launch_id] == observation, "inconsistent launch observations")
        launches[launch_id] = observation
        if suite == "revision":
            replay = r["workload"] == "replay"
            require(r["skips"] == "0" and int(r["messages"]) == (32106 if replay else 0), "incomplete revision input")
            require(int(r["samples"]) == (302 if replay else 141) and int(r["callbacks"]) == (366 if replay else 181), "incomplete revision callbacks")
            require(r["world_samples"] == "4" and int(r["attachment_pixels"]) == (2073600 if r["size"] == "1920-native" else 1267500), "wrong revision world attachment")
        else:
            require(int(r["samples"]) == (90 if suite == "cache" else 30), "incomplete sparse/cache samples")
            require(r["world_samples"] == ("1" if r["size"] == "3840-native" else "4"), "wrong sparse/cache sample count")
            base = 2 if r["size"] == "1920-original" else 1
            if suite == "sparse":
                extra = 0
                if r["side"] == "reviewed":
                    extra = 2 if r["case"] == "one-glow" else int(r["case"] == "low-quality" and r["world_samples"] == "4")
                require(int(r["passes_min"]) == int(r["passes_max"]) == base + extra, "unexpected sparse pass count")
            else:
                require((int(r["passes_min"]), int(r["passes_max"])) == (base + 2, base + 3), "unexpected cache pass counts")
            require(0 <= int(r["target_bytes_min"]) <= int(r["target_bytes_max"]), "invalid target accounting")
    expected_launches = 50 if suite == "revision" else 10
    require(len(launches) == expected_launches, "incomplete revision launches")
    require(Counter(int(v[0]) for v in launches.values()) == Counter(range(1, expected_launches + 1)), "missing/duplicate revision launch order")
    ordered = sorted(launches.values(), key=lambda v: int(v[0]))
    starts = [float(v[1]) for v in ordered]
    require(all(math.isfinite(v) and v >= 0 for v in starts) and starts == sorted(starts), "nonchronological revision launches")
    for pair in range(1, 6):
        for cell in cells:
            arms = {r["side"]: r for r in rows if r["pair"] == str(pair) and tuple(r[k] for k in fields) == cell}
            a, b = (arms[s] for s in sides)
            require(int(b["launch_index"]) - int(a["launch_index"]) == (1 if pair % 2 else -1), "revision arms not matched/alternating")
            if suite == "revision":
                for key in ("samples", "messages", "callbacks", "draws_peak", "body_quads_peak", "outline_quads_peak", "glow_quads_peak", "other_quads_peak",
                            "body_screen_area_peak", "outline_screen_area_peak", "glow_screen_area_peak", "other_screen_area_peak"):
                    require(a[key] == b[key], "revision CPU picture/coverage differs: " + key)


def size_label(size):
    return {"1920-original": "1920×1080 Original", "1920-native": "1920×1080 Native", "3840-native": "3840×2160 Native"}[size]


def summary(rows, side, key):
    return median(float(r[key]) for r in rows if r["side"] == side)


def sparse_table(rows):
    lines = ["| Sparse authored picture / output | Reviewed GPU median / p95 ms | Fixed GPU median / p95 ms | Fixed median range ms | Reviewed → fixed passes |",
             "| --- | ---: | ---: | ---: | ---: |"]
    for size, case in product(SIZES, CASES):
        cell = [r for r in rows if (r["size"], r["case"]) == (size, case)]
        values = [f"{summary(cell, side, 'gpu_median_ms'):.4f} / {summary(cell, side, 'gpu_p95_ms'):.4f}" for side in ("reviewed", "fixed")]
        fixed = [float(r["gpu_median_ms"]) for r in cell if r["side"] == "fixed"]
        passes = [next(r["passes_min"] for r in cell if r["side"] == s) for s in ("reviewed", "fixed")]
        lines.append(f"| {case} / {size_label(size)} | {' | '.join(values)} | {min(fixed):.4f}–{max(fixed):.4f} | {' → '.join(passes)} |")
    return "\n".join(lines)


def cache_table(rows):
    lines = ["| Factor flicker / output | Cache GPU median / p95 ms | Retire GPU median / p95 ms | Cache → retire CPU submit median / p95 ms | Retained target bytes: cache / retire range |",
             "| --- | ---: | ---: | ---: | ---: |"]
    for size in SIZES:
        cell = [r for r in rows if r["size"] == size]
        gpu = [f"{summary(cell, s, 'gpu_median_ms'):.4f} / {summary(cell, s, 'gpu_p95_ms'):.4f}" for s in ("fixed", "retire")]
        cpu = [f"{summary(cell, s, 'submit_median_ms'):.4f} / {summary(cell, s, 'submit_p95_ms'):.4f}" for s in ("fixed", "retire")]
        memory = [f"{min(int(r['target_bytes_min']) for r in cell if r['side'] == s):,}–{max(int(r['target_bytes_max']) for r in cell if r['side'] == s):,}" for s in ("fixed", "retire")]
        lines.append(f"| {size_label(size)} | {' | '.join(gpu)} | {' → '.join(cpu)} | {' / '.join(memory)} |")
    return "\n".join(lines)


def headline_table(rows, label):
    lines = ["| Crowded history / output | Main control GPU median / p95 ms | Fixed GPU median / p95 ms | Fixed run-median range ms |",
             "| --- | ---: | ---: | ---: |"]
    for cell in HEADLINES:
        selected = [r for r in rows if (r["workload"], r["zoom"], r["size"]) == cell]
        gpu = [f"{summary(selected, s, 'gpu_median_ms'):.3f} / {summary(selected, s, 'gpu_p95_ms'):.3f}" for s in ("before", "after")]
        medians = [float(r["gpu_median_ms"]) for r in selected if r["side"] == "after"]
        lines.append(f"| {label(*cell)} | {' | '.join(gpu)} | {min(medians):.3f}–{max(medians):.3f} |")
    return "\n".join(lines)


def host_rows(rows):
    """Multi-cell sparse/cache rows share a process; count each launch once."""
    return list({r["launch_id"]: r for r in rows}.values())


def validate_reference(rows, pictures, original_rows, original_pictures):
    require([r for r in rows if r["side"] == "before"] == [r for r in original_rows if r["side"] == "before"], "revision control receipts changed")
    for r in pictures:
        original = next(p for p in original_pictures if all(p[k] == r[k] for k in ("workload", "zoom", "size", "frame")))
        require(r["before_rgba_sha256"] == original["before_rgba_sha256"], "revision control readback changed")
