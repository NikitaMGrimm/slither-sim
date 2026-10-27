"""Select compact count/score series from completed local sanity runs."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
from statistics import fmean


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    rows = []
    samples = []
    for name in ["main-32", "pit-food-32", "main-64", "pit-food-64"]:
        raw = (args.output / (name + ".json")).read_bytes()
        data = json.loads(raw)
        series = data["samples"]
        end = series[-1]
        late = [s["food"] for s in series if s["time"] >= 240.]
        middle = [s["food"] for s in series if 180. <= s["time"] < 240.]
        rows.append({
            "name": name, "arm": data["arm"], "seats": data["seats"], "seed": data["seed"],
            "seconds": data["seconds"], "ticks": data["ticks"], "pending": data["pending"],
            "config": data["config"],
            "initialFood": series[0]["food"], "minimumFood": data["minimumFood"],
            "maximumFood": data["maximumFood"], "endFood": end["food"], "endTarget": end["target"],
            "lateSampledMinimumFood": min(late), "lateSampledMaximumFood": max(late),
            "lateMeanFood": fmean(late), "previousMinuteMeanFood": fmean(middle),
            "endInside": end["inside"], "endOutside": end["outside"],
            "maximumSampledOutside": max(s["outside"] for s in series),
            "endInsideNutrition": end["insideNutrition"], "endOutsideNutrition": end["outsideNutrition"],
            "endLiving": end["living"], "endMeanScore": end["meanScore"],
            "endMedianScore": end["medianScore"], "endMaxScore": end["maxScore"],
            "kills": end["kills"], "killsPerMinute": end["kills"] * 60. / data["seconds"],
            "deaths": end["deaths"], "deathsWithFood": end["deathsWithFood"],
            "rawResultSha256": hashlib.sha256(raw).hexdigest(),
        })
        samples.extend({"name": name, "arm": data["arm"], "seats": data["seats"], "seed": data["seed"], **s} for s in series)
    args.destination.mkdir(parents=True, exist_ok=True)
    results = {"dataOrigin": "local-simulator", "analysisSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), "runs": rows, "sourcePins": json.loads((args.output / "source-pins.json").read_text(encoding="utf-8")), "attempts": [json.loads(line) for line in (args.output / "attempts.jsonl").read_text(encoding="utf-8").splitlines()]}
    (args.destination / "results.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8", newline="\n")
    with (args.destination / "samples.csv").open("w", encoding="utf-8", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=list(samples[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(samples)
    for row in rows:
        print(row["name"], "min/peak/end/target", row["minimumFood"], row["maximumFood"], row["endFood"], row["endTarget"], "late-range", row["lateSampledMinimumFood"], row["lateSampledMaximumFood"], "late-mean", round(row["lateMeanFood"], 1))


if __name__ == "__main__":
    main()
