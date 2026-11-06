"""Recompute selected authored Low-preset results; --check verifies REPORT.md."""
from pathlib import Path
import argparse
import csv
import itertools
import json
import math
import statistics

ROOT = Path(__file__).resolve().parent


def recompute():
    rows = list(csv.DictReader((ROOT / "runs.csv").open(newline="")))
    expected = set(itertools.product(
        range(5), ["rtx", "amd"], ["baseline", "candidate"],
        ["auto", "low", "medium", "high"], ["stock", "far", "whole"],
        ["Native", "Original"],
    ))
    actual = set()
    for r in rows:
        key = (int(r["round"]), r["adapter"], r["arm"], r["preset"], r["zoom"], r["mode"])
        assert key not in actual
        actual.add(key)
        assert int(r["frames"]) == 100
        for k in ["cpu_p50_ms", "cpu_p95_ms", "prepare_p50_ms", "sync_p50_ms", "submit_p50_ms", "wait_p50_ms"]:
            assert math.isfinite(float(r[k])) and float(r[k]) >= 0
        assert float(r["cpu_p95_ms"]) >= float(r["cpu_p50_ms"])
        assert 0 <= float(r["load_min_pct"]) <= float(r["load_median_pct"]) <= float(r["load_max_pct"]) <= 100
        for k in ["frames_sha256", "load_sha256"]:
            assert len(r[k]) == 64 and all(c in "0123456789abcdef" for c in r[k])
    assert actual == expected
    controls = json.loads((ROOT / "image-controls.json").read_text())
    assert not controls["failures"] and len(controls["comparisons"]) == 144
    image_keys = set()
    for c in controls["comparisons"]:
        key = (c["adapter"], c["preset"], c["zoom"], c["mode"], c["state"])
        assert key not in image_keys
        image_keys.add(key)
        assert c["identical"] and c["before_sha256"] == c["after_sha256"]
        assert len(c["before_sha256"]) == 64 and c["rgba_bytes"] == 3840 * 2160 * 4
        assert c["samples"] == (1 if c["mode"] == "Native" else 4)
        if c["zoom"] == "far":
            assert abs(c["zoom_factor"] - 0.065) < 1e-12
        if c["zoom"] == "whole":
            assert 0 < c["zoom_factor"] < 0.065
    assert image_keys == set(itertools.product(
        ["rtx", "amd"], ["auto", "low", "medium", "high"],
        ["stock", "far", "whole"], ["Native", "Original"], [1, 12, 240],
    ))
    identity = json.loads((ROOT / "identities.json").read_text())
    assert identity["baseline"]["driver_sha256"] == identity["candidate"]["driver_sha256"]
    assert identity["baseline"]["revision"].startswith("7cb5ec37")
    palette = json.loads((ROOT / "palette-controls.json").read_text())
    assert len(palette["comparisons"]) == 36
    palette_keys = set()
    for c in palette["comparisons"]:
        key = (c["adapter"], c["zoom"], c["mode"], c["state"])
        assert key not in palette_keys
        palette_keys.add(key)
        assert c["actual_sha256"] == c["canonical_sha256"]
        assert c["rgba_bytes"] == 3840 * 2160 * 4
    assert palette_keys == set(itertools.product(
        ["rtx", "amd"], ["stock", "far", "whole"], ["Native", "Original"], [1, 12, 240],
    ))
    coincident = json.loads((ROOT / "coincident-controls.json").read_text())
    assert coincident["before"] and all(
        not c["identical"] and c["changed_pixels"] > 0 and c["revision"] == "bed9a259"
        for c in coincident["before"])
    after_keys = set()
    for c in coincident["after"]:
        key = (c["adapter"], c["foods"], c["mode"])
        assert key not in after_keys and c["identical"] and c["retained"]
        assert c["rgba_bytes"] == 3840 * 2160 * 4 and c["eligible_pairs"] == c["foods"] // 2
        assert c["samples"] == (1 if c["mode"] == "Native" else 4)
        after_keys.add(key)
    assert after_keys == set(itertools.product(["rtx", "amd"], [4096, 8192], ["Native", "Original"]))
    assert coincident["regression_test"]["bed9a259"] == "failed"
    assert coincident["regression_test"]["candidate"] == "passed"
    rejected = json.loads((ROOT / "rejected-grouping.json").read_text())
    assert rejected["status"] == "rejected"
    assert rejected["changed_pixels"] == 10606 and rejected["changed_channels"] == 12062
    assert rejected["maximum_channel_difference"] == 2
    assert rejected["actual_rgba_sha256"] != rejected["canonical_rgba_sha256"]

    def values(adapter, mode, zoom, arm, preset):
        return [float(r["cpu_p50_ms"]) for r in rows if
                (r["adapter"], r["mode"], r["zoom"], r["arm"], r["preset"]) ==
                (adapter, mode, zoom, arm, preset)]

    def cell(v):
        return f"{statistics.median(v):.3f} [{min(v):.3f}, {max(v):.3f}]"

    table = ["| Adapter / world / zoom | Low main (ms) | Low candidate (ms) | Auto candidate (ms) | Medium candidate (ms) |",
             "| --- | ---: | ---: | ---: | ---: |"]
    stock, lead = [], []
    for adapter, mode, zoom in itertools.product(["rtx", "amd"], ["Native", "Original"], ["stock", "far", "whole"]):
        before = values(adapter, mode, zoom, "baseline", "low")
        low = values(adapter, mode, zoom, "candidate", "low")
        auto = values(adapter, mode, zoom, "candidate", "auto")
        medium = values(adapter, mode, zoom, "candidate", "medium")
        assert len(before) == len(low) == len(auto) == len(medium) == 5
        assert statistics.median(low) <= statistics.median(medium), (adapter, mode, zoom, "medium")
        if zoom == "stock":
            # Stock zoom is a tie: report the median difference, and require it
            # to stay inside both arms' run ranges rather than claim a lead.
            assert min(low) <= max(auto) and min(auto) <= max(low), (adapter, mode, zoom, "auto")
            stock.append(statistics.median(low) - statistics.median(auto))
        else:
            assert statistics.median(low) <= statistics.median(auto), (adapter, mode, zoom, "auto")
            lead.append(statistics.median(auto) - statistics.median(low))
        table.append(f"| {adapter} / {mode} / {zoom} | {cell(before)} | {cell(low)} | {cell(auto)} | {cell(medium)} |")
    before = statistics.median(values("rtx", "Native", "far", "baseline", "low"))
    after = statistics.median(values("rtx", "Native", "far", "candidate", "low"))
    result = (f"Low RTX/Vulkan Native 4K at factor 0.065: CPU preparation, resource sync and submission fall from "
              f"{before:.3f} to {after:.3f} ms ({100 * (1 - after / before):.1f}% lower) against main `7cb5ec37`, "
              "medians of five runs. "
              f"At far and whole-map zoom Low's median is {min(lead):.3f}–{max(lead):.3f} ms below Auto in all 8 cells; "
              f"at stock zoom Low and Auto tie, with Low's median {min(stock):+.3f} to {max(stock):+.3f} ms from Auto's, "
              "inside both run ranges. Low is below Medium in all 12 cells. "
              "All 144 full-frame RGBA SHA-256 pairs are identical, and all 8 coincident-pair controls "
              "match the canonical renderer.")
    unique_runs = {(r["round"], r["adapter"], r["arm"]): r for r in rows}
    assert len(unique_runs) == 20
    overlap = sum(r["compiler_observed"] == "True" for r in unique_runs.values())
    clean = sorted(n for n in {k[0] for k in unique_runs}
                   if all(r["compiler_observed"] == "False" for k, r in unique_runs.items() if k[0] == n))
    clean_rows = [r for r in rows if r["round"] in clean and r["arm"] == "candidate"]

    def clean_value(adapter, mode, zoom, preset):
        return statistics.median(float(r["cpu_p50_ms"]) for r in clean_rows if
                                 (r["adapter"], r["mode"], r["zoom"], r["preset"]) == (adapter, mode, zoom, preset))
    cells = list(itertools.product(["rtx", "amd"], ["Native", "Original"], ["stock", "far", "whole"]))
    clean_wins = sum(clean_value(*c, "low") <= clean_value(*c, "auto") for c in cells) if clean else 0
    summary = (f"20 interleaved runs; {sum(int(r['frames']) for r in rows):,} measured frames, each cell visited forward and reversed; "
               f"{overlap} of 20 runs observed a compiler/linker from unrelated work despite a quiet gate; "
               f"only round {', '.join(clean)} had none in all four runs, and there Low is at or below Auto in "
               f"{clean_wins} of 12 cells. "
               f"Per-run median machine CPU load: {min(float(r['load_median_pct']) for r in rows):.1f}"
               f"–{max(float(r['load_median_pct']) for r in rows):.1f}%; "
               f"all sampled load: {min(float(r['load_min_pct']) for r in rows):.1f}"
               f"–{max(float(r['load_max_pct']) for r in rows):.1f}%.")
    return {"RESULT": result, "TIMING": "\n".join(table), "SUMMARY": summary}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    path = ROOT / "REPORT.md"
    text = path.read_text(encoding="utf-8")
    updated = text
    for name, value in recompute().items():
        start, end = f"<!-- {name} -->\n", f"\n<!-- /{name} -->"
        assert updated.count(start) == updated.count(end) == 1
        prefix, rest = updated.split(start)
        _, suffix = rest.split(end)
        updated = prefix + start + value + end + suffix
    if args.check:
        assert updated == text, "REPORT.md does not match retained selected results"
        print("Selected timing matrix, exact image controls and report agree.")
    else:
        path.write_text(updated, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
