"""Summarize the fixed policy using the original gates; never recalibrate them."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics

from analyze import describe, load


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    study = Path(__file__).resolve().parent
    repo = study.parents[2]
    original = read(study / "results.json")
    plan = read(study / "plan.json")
    amendment = read(study / "half-turn-fix-plan.json")
    assert digest(study / "plan.json") == amendment["originalPlanSha256"]
    assert digest(repo / "src/reference.rs") == amendment["fixedPolicySha256"]
    assert amendment["thresholds"] == original["thresholds"]
    result = {
        "dataOrigin": "local-simulator",
        "contract": original["contract"],
        "originalResultsSha256": digest(study / "results.json"),
        "amendmentPlanSha256": digest(study / "half-turn-fix-plan.json"),
        "sourceSha256": {path: digest(repo / path) for path in (
            "src/reference.rs", "src/client/mod.rs", "examples/reference_study.rs",
            "tests/reference_half_turn.rs")},
        "analysisSha256": digest(Path(__file__)),
        "thresholds": amendment["thresholds"],
        "packs": {},
    }
    binaries = set()
    for name in ("regression", "fresh-validation", "replay", *(f"cost-{n}" for n in (4, 16, 64, 128))):
        identity = read(args.raw / f"{name}-identity.json")
        assert identity["exit_code"] == 0 and identity["affinity_mask"] == 4
        assert identity["policy_sha256"] == amendment["fixedPolicySha256"]
        binaries.add(identity["binary_sha256"])
    assert len(binaries) == 1
    result["binarySha256"] = binaries.pop()
    for name, seed_key in (("regression", "regressionSeeds"), ("fresh-validation", "freshValidationSeeds")):
        seeds, blocks = load(args.raw / f"{name}.jsonl")
        expected = amendment[seed_key]
        assert seeds == list(range(expected[0], expected[1] + 1))
        stats = describe(blocks, plan)
        result["packs"][name] = {
            "seeds": seeds,
            "status": "reused regression seeds" if name == "regression" else "fresh held-out seeds",
            "statistics": stats,
            "qualified": {kind: all(metrics[m]["ci95"][0] >= amendment["thresholds"][m]
                                    for m in amendment["thresholds"]) for kind, metrics in stats.items()},
        }
    expected_replay = b"".join((args.raw / "fresh-validation.jsonl").read_bytes().splitlines(keepends=True)[:2])
    assert (args.raw / "replay.jsonl").read_bytes() == expected_replay
    result["replayByteIdentical"] = True
    result["cost"] = []
    for n in (4, 16, 64, 128):
        rows = [json.loads(line) for line in (args.raw / f"cost-{n}.jsonl").read_text().splitlines()]
        assert len(rows) == 3 and all(row["decisions"] == 200000 and row["policy_state_bytes"] == 0 for row in rows)
        cpu = [row["cpu_us_per_decision"] for row in rows]
        result["cost"].append({"agents": n, "frames": rows[0]["frames"], "cpuMedian": statistics.median(cpu),
                               "cpuMin": min(cpu), "cpuMax": max(cpu),
                               "wallMedian": statistics.median(row["wall_us_per_decision"] for row in rows)})
    result["environment"] = original["environment"] | {
        "isolation": "shared Windows desktop; one logical CPU inherited from the launcher; Rust/tools checks finished before cost runs"
    }
    result["rawAccess"] = "local-only: " + args.raw.as_posix()
    result["rawSha256"] = {path.name: digest(path) for path in sorted(args.raw.iterdir()) if path.is_file()}
    with args.output.open("x", encoding="utf-8", newline="\n") as output:
        json.dump(result, output, indent=2)
        output.write("\n")
    print(json.dumps({"packs": {name: pack["qualified"] for name, pack in result["packs"].items()},
                      "cost": result["cost"]}, indent=2))


if __name__ == "__main__":
    main()
