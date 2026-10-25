"""Select compact CSV series and summarize the two local measurement directories."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import statistics


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows = []
    identities = {}
    for label in ("before", "after"):
        folder = getattr(args, label)
        manifest = json.loads((folder / "manifest.json").read_text())
        assert len(manifest["runs"]) == 6
        assert all(run["status"] == "complete" for run in manifest["runs"])
        identities[label] = {key: value for key, value in manifest.items() if key != "source_diff"}
        identities[label]["source_diff_sha256"] = hashlib.sha256(manifest["source_diff"].encode()).hexdigest()
        identities[label]["series_sha256"] = {}
        for agents in (32, 64):
            endpoints = []
            midpoints = []
            late_living = []
            late_food = []
            peaks = []
            for seed in (1, 2, 3):
                name = f"n{agents}-s{seed}.csv"
                data = (folder / name).read_bytes()
                identities[label]["series_sha256"][name] = hashlib.sha256(data).hexdigest()
                with (folder / name).open(newline="") as stream:
                    series = list(csv.DictReader(stream))
                assert len(series) == 61 and float(series[-1]["time"]) >= 600
                for row in series:
                    rows.append({"phase": label, **row})
                endpoints.append({key: float(value) for key, value in series[-1].items()})
                midpoints.append({key: float(value) for key, value in series[30].items()})
                late_living.extend(float(row["living"]) for row in series[30:])
                late_food.extend(float(row["food"]) for row in series[30:])
                peaks.extend(float(row["max_score"]) for row in series)
            summary = {key: {"mean": statistics.mean(row[key] for row in endpoints),
                             "min": min(row[key] for row in endpoints),
                             "max": max(row[key] for row in endpoints)} for key in endpoints[0]}
            summary["late_mean_living"] = statistics.mean(late_living)
            summary["late_min_living"] = min(late_living)
            summary["late_max_living"] = max(late_living)
            summary["late_food_min"] = min(late_food)
            summary["late_food_max"] = max(late_food)
            summary["peak_sampled_score"] = max(peaks)
            summary["deaths_per_minute"] = statistics.mean(row["deaths"] / (row["time"] / 60) for row in endpoints)
            summary["late_deaths_per_minute"] = statistics.mean(
                (end["deaths"] - mid["deaths"]) / ((end["time"] - mid["time"]) / 60)
                for end, mid in zip(endpoints, midpoints))
            summary["minute5_mean_score"] = statistics.mean(row["mean_score"] for row in midpoints)
            summary["minute5_food"] = statistics.mean(row["food"] for row in midpoints)
            summary["late_grown_fraction"] = sum(row["late_grown_lives"] for row in endpoints) / sum(row["late_lives"] for row in endpoints)
            summary["late_grown_lives_total"] = int(sum(row["late_grown_lives"] for row in endpoints))
            summary["late_lives_total"] = int(sum(row["late_lives"] for row in endpoints))
            identities.setdefault("summary", {})[f"{label}-{agents}"] = summary
    assert identities["before"]["harness_sha256"] == identities["after"]["harness_sha256"]
    assert identities["before"]["policy_sha256"] == identities["after"]["policy_sha256"]
    with (args.output / "samples.csv").open("x", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    (args.output / "results.json").write_text(json.dumps(identities, indent=2) + "\n", encoding="utf-8", newline="\n")
    for name, summary in identities["summary"].items():
        print(name, json.dumps({key: summary[key] for key in ("food", "ambient", "death", "food_value", "snake_mass", "mean_score", "median_score", "max_score", "deaths_per_minute", "late_deaths_per_minute", "minute5_mean_score", "minute5_food", "late_mean_living", "late_grown_fraction")}))


if __name__ == "__main__":
    main()
