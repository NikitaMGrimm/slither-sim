"""Recompute bounded offscreen results; no recording, GPU or network required."""
from __future__ import annotations

import argparse
import csv
import difflib
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
import statistics
import sys

ROOT = Path(__file__).resolve().parent
CONFIGS = {
    ("rtx", "far", "native"), ("rtx", "far", "original"),
    ("rtx", "whole", "native"), ("rtx", "whole", "original"),
    ("igpu", "far", "native"),
}
IMAGES = {
    f"authored/{adapter}/{quality}/{view}"
    for adapter in ["rtx", "igpu"] for quality in ["native", "original"]
    for view in ["stock", "far", "whole"]
} | {"authored/rtx/native/far-serial", "authored/rtx/native/far-late20"}


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def digest(value: str, length: int = 64) -> None:
    assert re.fullmatch(f"[0-9a-f]{{{length}}}", value), value


def load(sources_dir: str | Path | None = None) -> dict:
    # Full authored source archives stay outside Git; their hashes are bound here.
    archives = json.loads((ROOT / "source-archives.json").read_text())
    where = Path(sources_dir) if sources_dir else (ROOT if (ROOT / "sources.json.gz").exists() else None)

    def archive(name: str):
        if where is None:
            return None
        raw = (where / name).read_bytes()
        assert sha(raw) == archives[name], name
        return json.loads(gzip.decompress(raw))

    with (ROOT / "runs.csv").open(newline="", encoding="utf-8") as stream:
        runs = list(csv.DictReader(stream))
    return {
        "runs": runs,
        "identities": json.loads((ROOT / "identities.json").read_text()),
        "images": json.loads((ROOT / "image-controls.json").read_text()),
        "canonical": json.loads((ROOT / "canonical-controls.json").read_text()),
        "sources": archive("sources.json.gz"),
        "measurement_patch": (ROOT / "measurement.patch").read_text(encoding="utf-8"),
        "rebase": json.loads((ROOT / "rebase-controls.json").read_text()),
        "rebase_sources": archive("rebase-sources.json.gz"),
        "rebase_driver_patch": (ROOT / "rebase-driver.patch").read_text(encoding="utf-8"),
        "historical_hashes": {p: archives[p] if p in archives else sha((ROOT / p).read_bytes()) for p in ["runs.csv", "identities.json", "image-controls.json", "canonical-controls.json", "sources.json.gz", "measurement.patch"]},
    }


def validate_rebase(data: dict) -> None:
    rebase = data["rebase"]
    assert rebase["base_revision"] == "60bac3eae6967afc0f62325ad2f48d034f950159"
    digest(rebase["candidate_revision"], 40)
    assert rebase["physical"] == [3840, 2160] and rebase["timing_rerun"] is False
    assert rebase["historical_bindings"] == data["historical_hashes"]
    assert sha(data["rebase_driver_patch"].encode()) == rebase["driver_delta_sha256"]
    assert "-            while let Some(p) = g.take_frame() {" in data["rebase_driver_patch"]
    assert "+            while let Some(p) = g.take_frame_with_menu(" in data["rebase_driver_patch"]
    assert rebase["prior_driver_sha256"] == data["identities"]["baseline"]["driver_sha256"]
    identities = rebase["identities"]
    assert set(identities) == {"baseline", "endpoints"}
    assert identities["baseline"]["revision"] == rebase["base_revision"]
    assert identities["endpoints"]["revision"] == rebase["candidate_revision"]
    assert identities["baseline"]["binary_sha256"] != identities["endpoints"]["binary_sha256"]
    assert len({r["driver_sha256"] for r in identities.values()}) == 1
    assert identities["baseline"]["driver_sha256"] != rebase["prior_driver_sha256"]
    projector = "client/apps/wgpu-slice/src/pellet_batch.rs"
    sources = data["rebase_sources"]
    for arm, identity in identities.items():
        for key in ["binary_sha256", "driver_sha256"]:
            digest(identity[key])
        assert set(identity["runtime_source_sha256"]) == set(identity["measured_source_sha256"])
        if sources is None:
            continue
        assert set(identity["runtime_source_sha256"]) == set(sources["common"]) | {projector}
        for path in identity["runtime_source_sha256"]:
            for kind in ["runtime", "measured"]:
                source = sources["projector"][arm] if path == projector else sources["common"][path][kind]
                assert sha(source.encode()) == identity[kind + "_source_sha256"][path]
    assert set(rebase["images"]) == IMAGES
    for arms in rebase["images"].values():
        assert set(arms) == set(identities)
        for arm, row in arms.items():
            assert row["callbacks"] == 600 and row["skips"] == 0 and row["distinct_images"] > 1
            assert 0 < row["retained_callbacks"] <= 600 and 0 < row["menu_callbacks"] <= 600
            assert row["census_callbacks"] == 600
            for key in ["retained_callbacks", "menu_callbacks", "census_callbacks"]:
                assert row[key] == arms["baseline"][key]
            for key in ["binary_sha256", "driver_sha256"]:
                assert row[key] == identities[arm][key]
            for key in ["image_sequence_sha256", "census_sha256"]:
                digest(row[key])
                assert row[key] == arms["baseline"][key]
    canonical = rebase["canonical"]
    assert set(canonical) == set(identities)
    for arm, row in canonical.items():
        for key in ["binary_sha256", "driver_sha256"]:
            assert row[key] == identities[arm][key]
        for key in ["launch_sha256", "log_sha256"]:
            digest(row[key])
        assert row["callbacks"] == 600 and row["skips"] == 0 and row["canonical_active"] is True
        assert row["canonical_quads"] > 0
        for key in ["quad_digest", "canonical_digest"]:
            digest(row[key], 16)
            assert row[key] != "d1fba762150c532c"
        for key in ["quad_digest", "canonical_digest", "canonical_quads", "messages"]:
            assert row[key] == canonical["baseline"][key]


def validate(data: dict) -> dict:
    validate_rebase(data)
    ids = data["identities"]
    assert set(ids) == {"baseline", "endpoints"}
    assert ids["baseline"]["binary_sha256"] != ids["endpoints"]["binary_sha256"]
    assert len({v["driver_sha256"] for v in ids.values()}) == 1
    for arm, identity in ids.items():
        for key in ["binary_sha256", "driver_sha256", "runtime_source_sha256", "measured_source_sha256"]:
            digest(identity[key])
        if data["sources"] is not None:
            assert sha(data["sources"][arm].encode()) == identity["runtime_source_sha256"]
        # The projector is not modified by passive timing instrumentation.
        assert identity["runtime_source_sha256"] == identity["measured_source_sha256"]
        for hashes in identity["passive_instrumentation"].values():
            assert set(hashes) == {"runtime", "measured"}
            for value in hashes.values():
                digest(value)
    assert ids["baseline"]["passive_instrumentation"] == ids["endpoints"]["passive_instrumentation"]
    if data["sources"] is not None:
        validate_sources(data)
    return validate_controls(data)


def validate_sources(data: dict) -> None:
    ids = data["identities"]
    assert set(data["sources"]["instrumentation"]) == set(ids["baseline"]["passive_instrumentation"])
    for path, hashes in ids["baseline"]["passive_instrumentation"].items():
        for kind, expected in hashes.items():
            assert sha(data["sources"]["instrumentation"][path][kind].encode()) == expected
    patch = []
    for path, pair in sorted(data["sources"]["instrumentation"].items()):
        patch.extend(difflib.unified_diff(pair["runtime"].splitlines(keepends=True), pair["measured"].splitlines(keepends=True), fromfile="a/" + path, tofile="b/" + path, n=0))
    assert data["measurement_patch"] == "".join(patch)
    delivered = data["sources"]["delivered"]
    assert sha(delivered.encode()) == ids["endpoints"]["delivered_source_sha256"]
    marker = "#[cfg(test)]\npub(crate) mod tests {"
    assert marker in delivered and marker in data["sources"]["endpoints"]
    assert delivered.split(marker)[0] == data["sources"]["endpoints"].split(marker)[0]


def validate_controls(data: dict) -> dict:
    ids = data["identities"]
    assert set(data["images"]) == IMAGES
    for arms in data["images"].values():
        assert set(arms) == set(ids)
        for arm, row in arms.items():
            assert row["callbacks"] == 600 and row["skips"] == 0
            assert row["distinct_images"] > 1
            for key in ["binary_sha256", "driver_sha256"]:
                assert row[key] == ids[arm][key]
            for key in ["image_sequence_sha256", "census_sha256"]:
                digest(row[key])
                assert row[key] == arms["baseline"][key]
    canonical = data["canonical"]
    assert set(canonical) == set(ids)
    for arm, row in canonical.items():
        for key in ["binary_sha256", "driver_sha256"]:
            assert row[key] == ids[arm][key]
        for key in ["launch_sha256", "log_sha256"]:
            digest(row[key])
        assert row["canonical_active"] is True
        for key in ["quad_digest", "canonical_quads", "canonical_digest", "callbacks", "messages", "skips"]:
            assert row[key] == canonical["baseline"][key]
    c = canonical["baseline"]
    assert c["canonical_active"] is True and c["canonical_quads"] > 0
    assert c["callbacks"] == 600 and c["skips"] == 0 and c["messages"] > 0
    for key in ["quad_digest", "canonical_digest"]:
        digest(c[key], 16)
        assert c[key] != "d1fba762150c532c"

    pairs = {}
    for row in data["runs"]:
        config = row["adapter"], row["view"], row["quality"]
        assert config in CONFIGS and row["arm"] in ids
        assert row["physical"] == "3840x2160"
        assert int(row["callbacks"]) == 600 and int(row["messages"]) == 29044 and int(row["skips"]) == 0
        assert int(row["census_entries"]) == 600 and int(row["art_census_entries"]) == 300
        assert 0 < int(row["submitted_min"]) <= int(row["submitted_max"])
        for key in ["binary_sha256", "driver_sha256"]:
            assert row[key] == ids[row["arm"]][key]
        for key in ["frames_sha256", "load_sha256", "census_sha256", "numeric_sha256"]:
            digest(row[key])
        digest(row["command_digest"], 16)
        for key, value in row.items():
            if key.endswith("_ms") or key.startswith("load_") and key != "load_sha256":
                assert math.isfinite(float(value)) and float(value) >= 0
        group = pairs.setdefault((*config, int(row["round"])), {})
        assert row["arm"] not in group
        group[row["arm"]] = row
    clean = {config: [] for config in CONFIGS}
    for key, pair in pairs.items():
        assert set(pair) == set(ids)
        for name in ["census_sha256", "numeric_sha256", "command_digest"]:
            assert len({r[name] for r in pair.values()}) == 1
        if any(r["pair_status"] == "lock_lost" for r in pair.values()):
            assert all(r["pair_status"] == "lock_lost" and r["lock_valid"] == "False" and r["exclusion"] for r in pair.values())
            continue
        for row in pair.values():
            assert row["lock_valid"] == "True"
            digest(row["timing_lock_sha256"])
            assert 0 <= float(row["lock_age_start_s"]) <= float(row["lock_age_end_s"]) < 3600
        assert len({row["timing_lock_sha256"] for row in pair.values()}) == 1
        accepted = all(r["status"] == "accepted" and int(r["compile_samples"]) == 0 for r in pair.values())
        assert all(r["pair_status"] == ("accepted" if accepted else "compile_overlap_pair") for r in pair.values())
        if accepted:
            clean[key[:3]].append(pair)
    assert all(len(pairs) == 5 for pairs in clean.values())
    return clean


def table(data: dict) -> str:
    clean = validate(data)
    lines = [
        "| Adapter / view / quality | Callback before ms p50/p95 | After ms p50/p95 | Pellet prepare ms p50/p95, before → after | Paired callback p50 delta range ms |",
        "| --- | ---: | ---: | ---: | ---: |",
    ]
    med = statistics.median
    for config, pairs in sorted(clean.items()):
        def value(arm: str, field: str) -> float:
            return med(float(p[arm][field]) for p in pairs)
        fields = ["wall_ns_p50_ms", "wall_ns_p95_ms"]
        before = "/".join(f"{value('baseline', field):.3f}" for field in fields)
        after = "/".join(f"{value('endpoints', field):.3f}" for field in fields)
        preparation = " → ".join("/".join(f"{value(arm, field):.3f}" for field in ["pellet_prepare_p50_ms", "pellet_prepare_p95_ms"]) for arm in ["baseline", "endpoints"])
        deltas = [float(p["endpoints"][fields[0]]) - float(p["baseline"][fields[0]]) for p in pairs]
        lines.append(f"| {' / '.join(config)} | {before} | {after} | {preparation} | {min(deltas):+.3f} to {max(deltas):+.3f} |")
    return "\n".join(lines)


def spread(data: dict) -> str:
    lines = [
        "| Adapter / view / quality | Paired pellet p50 delta range ms | Paired callback p95 delta range ms | Per-run callback p95 range ms, before → after |",
        "| --- | ---: | ---: | ---: |",
    ]
    for config, pairs in sorted(validate(data).items()):
        def deltas(field):
            return [float(p["endpoints"][field]) - float(p["baseline"][field]) for p in pairs]
        def bounds(values):
            return f"{min(values):+.3f} to {max(values):+.3f}"
        ranges = []
        for arm in ["baseline", "endpoints"]:
            values = [float(p[arm]["wall_ns_p95_ms"]) for p in pairs]
            ranges.append(f"{min(values):.3f}–{max(values):.3f}")
        lines.append(f"| {' / '.join(config)} | {bounds(deltas('pellet_prepare_p50_ms'))} | {bounds(deltas('wall_ns_p95_ms'))} | {' → '.join(ranges)} |")
    return "\n".join(lines)


def summary(data: dict) -> str:
    validate(data)
    rows = data["runs"]
    selected = [r for r in rows if r["pair_status"] == "accepted"]
    compilers = sum(r["pair_status"] == "compile_overlap_pair" and r["status"] == "compile_overlap" for r in rows)
    partners = sum(r["pair_status"] == "compile_overlap_pair" and r["status"] == "accepted" for r in rows)
    lost = sum(r["pair_status"] == "lock_lost" for r in rows)
    loads = [float(r["load_p50"]) for r in selected]
    return (f"Retained timing attempts: {len(rows)}; selected: {len(selected)} in 25 complete blocks; "
            f"compiler/link overlap: {compilers}; clean partners excluded with their block: {partners}; "
            f"lock-loss exclusions: {lost}. Selected per-run median machine load: "
            f"{min(loads):.1f}–{max(loads):.1f}% (median {statistics.median(loads):.1f}%).")


def main() -> None:
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--sources", help="directory holding the out-of-Git source archives")
    args = parser.parse_args()
    data = load(args.sources)
    result = table(data)
    if args.check:
        report = (ROOT / "REPORT.md").read_text(encoding="utf-8")
        assert report.split("<!-- TIMING -->\n")[1].split("\n<!-- /TIMING -->")[0] == result
        assert report.split("<!-- SPREAD -->\n")[1].split("\n<!-- /SPREAD -->")[0] == spread(data)
        assert report.split("<!-- SUMMARY -->\n")[1].split("\n<!-- /SUMMARY -->")[0] == summary(data)
        print("Historical timing and original/rebased exact image, census and canonical controls pass.")
    else:
        print(result)


if __name__ == "__main__":
    main()
