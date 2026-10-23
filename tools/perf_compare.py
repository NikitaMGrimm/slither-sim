"""Alternate copied baseline/candidate release harnesses within one quiet window.

Uses the same operation and Windows process counters as perf_baseline.py.
No builds, tracing, host listeners or environment changes.
Use --max-wall-seconds to share a deadline across metadata/workload children;
reserve cleanup time separately. See docs/performance-deadlines.md.
"""

import argparse
import json
import math
from pathlib import Path
import statistics
import time

from perf_baseline import command_output, digest, native


WORKLOADS = {"step-default": 100000, "step-food": 10000, "step-body": 50000,
             "observe": 1000, "portable": 5000, "protocol": 5000}
DEFAULT_CASES = tuple(WORKLOADS)
# Explicit opt-in: these require examples/perf_observation.rs on both runtimes.
WORKLOADS.update({"observe-dense": 1000, "observe-compact": 500,
                  "observe-wide": 10000, "observe-strip": 1000,
                  "observe-one": 10000, "observe-empty": 100000})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--max-wall-seconds", type=float,
                        help="shared metadata/workload deadline; allow up to 10s extra for child cleanup")
    parser.add_argument("--cases", nargs="+", choices=tuple(WORKLOADS), default=list(DEFAULT_CASES))
    args = parser.parse_args()
    if not 1 <= args.repeats <= 10:
        parser.error("repeats must be 1..10")
    if args.max_wall_seconds is not None and (
            not math.isfinite(args.max_wall_seconds) or args.max_wall_seconds <= 0):
        parser.error("max-wall-seconds must be finite and positive")
    binaries = {"baseline": args.baseline.resolve(strict=True),
                "candidate": args.candidate.resolve(strict=True)}
    hashes = {key: digest(path) for key, path in binaries.items()}
    args.output.mkdir(parents=True, exist_ok=False)
    rows = []
    started = time.perf_counter()
    deadline = started + args.max_wall_seconds if args.max_wall_seconds is not None else None
    budget = {"deadline": deadline} if deadline is not None else {}
    try:
        manifest = {"schema": "slither-perf-compare-v1",
                    "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                    "source_commit": command_output("git", "rev-parse", "HEAD", **budget),
                    "source_status": command_output("git", "status", "--short", **budget),
                    "rustc": command_output("rustc", "-Vv", **budget),
                    "harnesses": {key: {"path": str(path), "sha256": hashes[key]}
                                  for key, path in binaries.items()},
                    "workloads": {case: WORKLOADS[case] for case in args.cases},
                    "repeats": args.repeats, "warmup_repetitions": 1,
                    "max_wall_seconds": args.max_wall_seconds,
                    "deadline_scope": "metadata and all workload children share one deadline; cleanup can take up to 10 additional seconds; no OS CPU/memory cap",
                    "order": "alternate label order per repetition and case; adjacent matched pairs",
                    "notes": "Native wall excludes setup/reset/output drop; process CPU includes them. Observation harness separately reports outputDrop wall intervals. Quiet agreement and build identities recorded in report."}
        (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
        with (args.output / "results.jsonl").open("w", encoding="utf-8", newline="\n") as output:
            for repetition in range(-1, args.repeats):
                for case_index, case in enumerate(args.cases):
                    order = ("baseline", "candidate") if (repetition + case_index) % 2 == 0 else ("candidate", "baseline")
                    for label in order:
                        if deadline is not None and time.perf_counter() >= deadline:
                            raise TimeoutError("performance run deadline exceeded before next child")
                        row = native(binaries[label], case, WORKLOADS[case],
                                     args.output / f"{case}-{repetition}-{label}-stderr.txt", **budget)
                        row.update({"label": label, "repetition": repetition})
                        output.write(json.dumps(row, separators=(",", ":")) + "\n")
                        output.flush()
                        rows.append(row)
                        print(json.dumps({"case": case, "repetition": repetition, "label": label,
                                          "cpu": row["cpu_seconds"],
                                          "inner_wall": row["workload"]["timing"]["wallSeconds"]}), flush=True)
        if deadline is not None and time.perf_counter() >= deadline:
            raise TimeoutError("performance run deadline exceeded before completion")
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
        (args.output / "failure.json").write_text(json.dumps({"type": type(error).__name__, "error": str(error),
                                                            "elapsed_wall_seconds": time.perf_counter() - started,
                                                            "max_wall_seconds": args.max_wall_seconds}) + "\n",
                                                  encoding="utf-8", newline="\n")
        raise


if __name__ == "__main__":
    main()
