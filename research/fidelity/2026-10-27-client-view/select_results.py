"""Select bounded aggregate results; never export raw frames, names, paths or endpoints."""
import argparse
import json
from pathlib import Path
from replay import dump, sha

LABELS = ("official-low2", "official-low3", "official-second1", "local-seed7")
PRIMARY = ("snake_anchor/visible_own_alive", "snake_motion/visible_own_alive",
           "snake_anchor/visible_100_169_cruise", "snake_motion/visible_100_169_cruise",
           "prey_motion/center_in_viewport_own_alive", "prey_motion/all_received")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--measurements", type=Path, required=True)
    p.add_argument("--evidence", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    a = p.parse_args()
    here = Path(__file__).resolve().parent
    result = dict(schemaVersion=1, status="exploratory; no fidelity pass/fail or runtime change",
        simulatorRevision="aee542100496b8cbc031b8ab19bb58e78bd8af2b",
        clientRevision="8d0772c015d6b9b73b80253db8c7349aa600b201",
        access="Raw captures and complete per-packet outputs are local-only, outside Git; no public download.",
        quantile="nearest rank, pooled packets within each window only; windows never pooled",
        sourceSha256={str(f.relative_to(here)).replace("\\", "/"): sha(f)
            for f in sorted([here / "harness/src/main.rs", here / "harness/Cargo.toml",
                here / "harness/Cargo.lock", *here.glob("*.py"), here / "host-config.json"])},
        localHost=json.loads((a.evidence / "local-seed7/manifest.json").read_text()),
        sources=[], primary=[], sensitivity=[], attempts=[
            {"label": "official-low1", "status": "excluded: header/meta only, zero incoming frames",
             "source_sha256": ""},
            {"label": "development", "status": "retained: initial synthetic packet authoring failures; corrected before final checks"},
            {"label": "development-replays", "status": "retained: early per-run outputs, then final v2 lifecycle/culling gates and full-frame parity; same windows"},
            {"label": "repository-guard", "status": "wrong-cwd invocation rejected unrelated simulator files; stopped and rerun successfully in pinned client checkout"}])
    for label in LABELS:
        census_name = "local-census.json" if label == "local-seed7" else label + "-census.json"
        census = json.loads((a.evidence / census_name).read_text())
        result["sources"].append(dict(label=label, source_sha256=census["archiveSha256"],
            census_status=census["status"], full_archive_incoming_frames=census["incomingFrames"],
            census_sha256=sha(a.evidence / census_name)))
        for mode in ("receipt60", "server60", "receipt125"):
            folder = a.measurements / (label + "-" + mode)
            summary = json.loads((folder / "summary.json").read_text())
            attempt = json.loads((folder / "attempt.json").read_text())
            assert attempt["exit_code"] == 0
            assert summary["provenance"]["source_sha256"] == census["archiveSha256"]
            assert summary["audit"]["frames"] == summary["audit"]["whole_frame_parity_checks"]
            row = dict(label=label, mode=mode, summary_sha256=sha(folder / "summary.json"),
                rows_sha256=sha(folder / "rows.jsonl"), binary_sha256=attempt["binary_sha256"],
                provenance=summary["provenance"], audit=summary["audit"])
            if mode == "receipt60":
                row["groups"] = {key: summary["groups"].get(key, {}) for key in PRIMARY}
                result["primary"].append(row)
            else:
                row["groups"] = {key: {k: v for k, v in summary["groups"].get(key, {}).items()
                    if k in ("head_innovation", "heading_error_rad", "innovation")}
                    for key in PRIMARY if "100_169" not in key}
                result["sensitivity"].append(row)
    # Locator is intentionally private; export only the digest for the zero-exposure archive.
    excluded = a.evidence / "excluded-low1-sha256.txt"
    result["attempts"][0]["source_sha256"] = excluded.read_text().strip()
    dump(a.output, result)


if __name__ == "__main__":
    main()
