"""Alternate copied baseline/candidate release harnesses within one quiet window.

Uses the same operation and Windows process counters as perf_baseline.py.
No builds, tracing, host listeners or environment changes.
"""

import argparse
import json
from pathlib import Path
import statistics
import time

from perf_baseline import command_output, digest, native


WORKLOADS = {"step-default": 100000, "step-food": 10000, "step-body": 50000,
             "observe": 1000, "portable": 5000, "protocol": 5000}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--cases", nargs="+", choices=tuple(WORKLOADS), default=list(WORKLOADS))
    args = parser.parse_args()
    if not 1 <= args.repeats <= 10:
        parser.error("repeats must be 1..10")
    binaries = {"baseline": args.baseline.resolve(strict=True),
                "candidate": args.candidate.resolve(strict=True)}
    hashes = {key: digest(path) for key, path in binaries.items()}
    args.output.mkdir(parents=True, exist_ok=False)
    rows = []
    try:
        manifest = {"schema": "slither-perf-compare-v1",
                    "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                    "source_commit": command_output("git", "rev-parse", "HEAD"),
                    "source_status": command_output("git", "status", "--short"),
                    "rustc": command_output("rustc", "-Vv"),
                    "harnesses": {key: {"path": str(path), "sha256": hashes[key]}
                                  for key, path in binaries.items()},
                    "workloads": {case: WORKLOADS[case] for case in args.cases},
                    "repeats": args.repeats, "warmup_repetitions": 1,
                    "order": "alternate label order per repetition and case; adjacent matched pairs",
                    "notes": "Native wall excludes setup/reset/output drop; process CPU includes them. Quiet agreement and build identities recorded in report."}
        (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
        with (args.output / "results.jsonl").open("w", encoding="utf-8", newline="\n") as output:
            for repetition in range(-1, args.repeats):
                for case_index, case in enumerate(args.cases):
                    order = ("baseline", "candidate") if (repetition + case_index) % 2 == 0 else ("candidate", "baseline")
                    for label in order:
                        row = native(binaries[label], case, WORKLOADS[case],
                                     args.output / f"{case}-{repetition}-{label}-stderr.txt")
                        row.update({"label": label, "repetition": repetition})
                        output.write(json.dumps(row, separators=(",", ":")) + "\n")
                        output.flush()
                        rows.append(row)
                        print(json.dumps({"case": case, "repetition": repetition, "label": label,
                                          "cpu": row["cpu_seconds"],
                                          "inner_wall": row["workload"]["timing"]["wallSeconds"]}), flush=True)
        if hashes != {key: digest(path) for key, path in binaries.items()}:
            raise RuntimeError("benchmark executable changed during comparison")
        summary = {}
        for case in args.cases:
            pairs = []
            for repetition in range(args.repeats):
                pair = {row["label"]: row for row in rows
                        if row["case"] == case and row["repetition"] == repetition}
                wall = {key: row["workload"]["timing"]["wallSeconds"] for key, row in pair.items()}
                cpu = {key: row["cpu_seconds"] for key, row in pair.items()}
                pairs.append({"repetition": repetition,
                              "wall_ratio_candidate_over_baseline": wall["candidate"] / wall["baseline"],
                              "cpu_ratio_candidate_over_baseline": (cpu["candidate"] / cpu["baseline"])
                              if all(value is not None and value > 0 for value in cpu.values()) else None})
            cpu_ratios = [pair["cpu_ratio_candidate_over_baseline"] for pair in pairs
                          if pair["cpu_ratio_candidate_over_baseline"] is not None]
            summary[case] = {"iterations": WORKLOADS[case], "pairs": pairs,
                             "median_wall_ratio_candidate_over_baseline": statistics.median(
                                 pair["wall_ratio_candidate_over_baseline"] for pair in pairs),
                             "median_cpu_ratio_candidate_over_baseline": statistics.median(cpu_ratios) if cpu_ratios else None}
        (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8", newline="\n")
    except BaseException as error:
        (args.output / "failure.json").write_text(json.dumps({"type": type(error).__name__, "error": str(error)}) + "\n",
                                                  encoding="utf-8", newline="\n")
        raise


if __name__ == "__main__":
    main()
