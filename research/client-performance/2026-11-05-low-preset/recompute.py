"""Recompute selected authored Low-preset results; --check verifies REPORT.md."""
from pathlib import Path
import argparse
import collections
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
    before_cases = collections.Counter((c["revision"], c["case"]) for c in coincident["before"])
    assert before_cases == {("bed9a259", "body pairs"): 6, ("4a49757f", "late-barrier caller pair"): 2}
    assert all(not c["identical"] and c["changed_pixels"] > 0 for c in coincident["before"])
    after_keys = set()
    pairs = {"body pairs": None, "one body pair": 1, "late-barrier caller pair": 1}
    for c in coincident["after"]:
        key = (c["case"], c["adapter"], c["foods"], c["mode"])
        assert key not in after_keys and c["identical"] and c["retained"]
        assert c["rgba_bytes"] == 3840 * 2160 * 4
        assert c["eligible_pairs"] == (pairs[c["case"]] or c["foods"] // 2)
        assert c["samples"] == (1 if c["mode"] == "Native" else 4)
        after_keys.add(key)
    assert after_keys == {
        (case, adapter, foods, mode)
        for case, foods in [("body pairs", 4096), ("body pairs", 8192), ("one body pair", 25000),
                            ("late-barrier caller pair", 4096)]
        for adapter in ["rtx", "amd"] for mode in ["Native", "Original"]}
    assert coincident["standard_controls_passed"] == 16
    tests = coincident["regression_tests"]
    assert set(tests["candidate"].values()) == {"passed"}
    assert tests["revision 4a49757f with the new tests"]["coincident_low_bodies_pair_like_the_generic_packer"] == "failed"
    assert tests["revision 4a49757f with the new tests"]["caller_pairs_cross_the_empty_low_late_barrier"] == "failed"
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
        assert statistics.median(low) <= statistics.median(auto), (adapter, mode, zoom, "auto")
        (stock if zoom == "stock" else lead).append(statistics.median(auto) - statistics.median(low))
        table.append(f"| {adapter} / {mode} / {zoom} | {cell(before)} | {cell(low)} | {cell(auto)} | {cell(medium)} |")
    before = statistics.median(values("rtx", "Native", "far", "baseline", "low"))
    after = statistics.median(values("rtx", "Native", "far", "candidate", "low"))
    result = (f"Low RTX/Vulkan Native 4K at factor 0.065: CPU preparation, resource sync and submission fall from "
              f"{before:.3f} to {after:.3f} ms ({100 * (1 - after / before):.1f}% lower) against main `7cb5ec37`, "
              "medians of five runs. "
              f"Low's median is below Auto's in all 12 hardware/world/zoom cells: by {min(lead):.3f}–{max(lead):.3f} ms "
              f"at far and whole-map zoom and by {min(stock):.3f}–{max(stock):.3f} ms at stock zoom. "
              "Low is below Medium in all 12 cells. "
              "All 144 full-frame RGBA SHA-256 pairs are identical, and all 16 coincident-pair controls "
              "match the canonical renderer.")
    unique_runs = {(r["round"], r["adapter"], r["arm"]): r for r in rows}
    assert len(unique_runs) == 20
    overlap = sum(r["compiler_observed"] == "True" for r in unique_runs.values())
    summary = (f"20 interleaved runs; {sum(int(r['frames']) for r in rows):,} measured frames, each cell visited forward and reversed; "
               f"{overlap} of 20 runs observed a compiler or linker process. "
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
