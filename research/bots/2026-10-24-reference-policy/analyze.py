"""Freeze calibration thresholds, then evaluate fresh portable seed blocks.

Standard library only. Raw JSONL stays outside Git; retain the small result JSON.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import statistics

METRICS = ("survival", "food_value")
KINDS = ("reference-control", "greedy", "circle", "idle", "suicide")


def quantile(values, fraction):
    values = sorted(values)
    position = (len(values) - 1) * fraction
    lower = math.floor(position)
    return values[lower] + (values[math.ceil(position)] - values[lower]) * (position - lower)


def load(path):
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8-sig").splitlines()]
    identities = {(row["seed"], row["rotation"]) for row in rows}
    if len(rows) != 48 or len(identities) != 48:
        raise ValueError("Expected 24 complete seed blocks, two unique rotations each")
    if any(row["contract"] != "slither-client-v2" or len(row["rows"]) != 16 for row in rows):
        raise ValueError("Unexpected contract or population")
    seeds = sorted({row["seed"] for row in rows})
    if len(seeds) != 24 or any((seed, rotation) not in identities for seed in seeds for rotation in (0, 5)):
        raise ValueError("Incomplete seed blocks")
    blocks = {kind: [] for kind in KINDS}
    for seed in seeds:
        worlds = [row for row in rows if row["seed"] == seed]
        for kind in KINDS:
            ratios = {metric: [] for metric in METRICS}
            for world in worlds:
                refs = [r for r in world["rows"] if r["kind"] == "reference"]
                candidates = refs[:4] if kind == "reference-control" else [r for r in world["rows"] if r["kind"] == kind]
                controls = refs[4:] if kind == "reference-control" else refs
                if len(refs) != 8 or len(candidates) != (4 if kind == "reference-control" else 2):
                    raise ValueError("Unexpected policy counts")
                for metric in METRICS:
                    denominator = statistics.mean(r[metric] for r in controls)
                    if denominator <= 0:
                        raise ValueError("Inconclusive zero reference metric")
                    ratios[metric].append(statistics.mean(r[metric] for r in candidates) / denominator)
            blocks[kind].append({metric: statistics.mean(ratios[metric]) for metric in METRICS})
    return seeds, blocks


def describe(blocks, plan):
    rng = random.Random(plan["bootstrapSeed"])
    result = {}
    for kind, rows in blocks.items():
        result[kind] = {}
        for metric in METRICS:
            values = [row[metric] for row in rows]
            draws = [statistics.mean(rng.choices(values, k=len(values)))
                     for _ in range(plan["bootstrapResamples"])]
            result[kind][metric] = dict(mean=statistics.mean(values), p05=quantile(values, .05),
                                       p95=quantile(values, .95),
                                       ci95=[quantile(draws, .025), quantile(draws, .975)])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("freeze", "validate"))
    parser.add_argument("raw", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--frozen", type=Path)
    args = parser.parse_args()
    plan_path = Path(__file__).with_name("plan.json")
    plan = json.loads(plan_path.read_text())
    seeds, blocks = load(args.raw)
    expected = plan["calibrationSeeds" if args.mode == "freeze" else "validationSeeds"]
    if seeds != list(range(expected[0], expected[1] + 1)):
        raise ValueError("Raw seeds do not match the predeclared pack")
    stats = describe(blocks, plan)
    result = dict(plan_sha256=hashlib.sha256(plan_path.read_bytes()).hexdigest(),
                  raw_sha256=hashlib.sha256(args.raw.read_bytes()).hexdigest(), seeds=seeds,
                  statistics=stats)
    if args.mode == "freeze":
        thresholds = {metric: math.floor(stats["reference-control"][metric]["p05"] * 20) / 20 for metric in METRICS}
        if thresholds["survival"] <= max(stats[k]["survival"]["p95"] for k in ("idle", "suicide")):
            raise ValueError("No calibrated survival separation")
        if thresholds["food_value"] <= stats["circle"]["food_value"]["p95"]:
            raise ValueError("No calibrated food separation")
        result["thresholds"] = thresholds
    else:
        if args.frozen is None:
            parser.error("validate requires --frozen")
        frozen = json.loads(args.frozen.read_text())
        if frozen["plan_sha256"] != result["plan_sha256"]:
            raise ValueError("Plan changed after threshold freeze")
        result["thresholds"] = frozen["thresholds"]
        result["qualified"] = {kind: all(stats[kind][metric]["ci95"][0] >= frozen["thresholds"][metric]
                                       for metric in METRICS) for kind in KINDS}
    with args.output.open("x", encoding="utf-8", newline="\n") as output:
        json.dump(result, output, indent=2)
        output.write("\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
