"""Same-binary (A/A) calibration pilot for the PR comparison; standard library only.

Retains the sampling schedule of the pilot in
research/performance/2026-10-24-pr-comparison-statistics/CALIBRATION-PLAN.md: one
workload group, m = 1, 2 and 4 reset-identical blocks per process, six fresh pairs
per workload and m, no early stopping. Both roles run the same copied, hashed
executable with identical arguments. The PR comparison's deadline applies.
Raw records are the result; summary.json holds per-job estimates for review, not
a calibration decision. See docs/maintainers/pr-reports.md#calibration.
"""

import argparse
import json
import math
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time

from perf_baseline import digest, load_catalog, native
import perf_compare
import perf_stats
# Retain the pilot budget and schedule; v2 changes the retired proxy rows.
# Historical source-pinned v1 records keep their original definitions.
JOB_TIMEOUT_SECONDS, RESERVE_SECONDS, STAGE_CAP_SECONDS = 900, 60, 620


PLAN = "slither-perf-calibration-pilot-v2"
GROUPS = {"A": ("step-default", "portable", "official-100-bot", "battledome-8-step"),
          "B": ("step-food", "step-body", "observe", "protocol", "official-100-step", "client-battledome-64")}
BLOCKS = (1, 2, 4)
# Every permutation of m once, balancing m's position within a workload's round.
BLOCK_ORDER = ((1, 2, 4), (1, 4, 2), (4, 2, 1), (2, 4, 1), (4, 1, 2), (2, 1, 4))
PAIRS = len(BLOCK_ORDER)
ORDER = perf_stats.ORDER[:PAIRS]  # three baseline-first and three candidate-first pairs


def child_error(row, workload, blocks):
    report = row.get("workload")
    timings = report.get("blocks") if isinstance(report, dict) else None
    if (not isinstance(timings, list) or len(timings) != blocks
            or (report.get("kind"), report.get("case"), report.get("iterations"))
            != ("report", workload.name, workload.iterations)):
        return f"expected {blocks} blocks of {workload.iterations} {workload.name} operations"
    for timing in timings:
        error = perf_compare.child_error({"workload": {**report, "mode": "timing", "timing": timing}}, workload)
        if error:
            return error
    return None


def estimate(pairs, blocks, failure=None):
    """Per-job variance components for one workload and m (plan: model, estimators)."""
    x = [[math.log(c) - math.log(b) for b, c in zip(p["baseline_blocks"], p["candidate_blocks"])] for p in pairs]
    y = [math.fsum(row) / blocks for row in x]
    r = len(pairs)
    cost = math.fsum(p["elapsed_seconds"] for p in pairs) / r if pairs else None
    result = {"pairs": r, "blocks": blocks, "seconds_per_pair": cost,
              "status": "failed" if failure else "complete" if r == PAIRS else "incomplete",
              "failure": failure}
    if failure or r != PAIRS:  # only a warmed, complete, balanced cell supports estimates
        return result
    order = [p["order"] for p in pairs]
    mu = math.fsum(y) / r
    gamma = math.fsum(o * v for o, v in zip(order, y)) / r
    s2 = math.fsum((v - mu - gamma * o) ** 2 for v, o in zip(y, order)) / (r - 2)
    result.update(mu_hat=mu, gamma_hat=gamma, paired_residual_variance=s2, Q=s2 * cost)
    if blocks > 1:
        wms = math.fsum((v - row_mean) ** 2 for row, row_mean in zip(x, y) for v in row) / (r * (blocks - 1))
        # Negative untruncated values are kept: truncation is a convention, not evidence.
        result.update(v_w=wms, v_b_untruncated=s2 - wms / blocks, v_b=max(0.0, s2 - wms / blocks))
    return result


def cost_model(cells):
    """Least-squares C(m) = h + c*m over the complete cells' observed pair costs."""
    points = [(cell["blocks"], cell["seconds_per_pair"]) for cell in cells
              if cell["pairs"] == PAIRS and not cell.get("failure")]
    if len(points) < 2:
        return None
    mean_m = math.fsum(m for m, _ in points) / len(points)
    mean_c = math.fsum(c for _, c in points) / len(points)
    slope = (math.fsum((m - mean_m) * (c - mean_c) for m, c in points)
             / math.fsum((m - mean_m) ** 2 for m, _ in points))
    return {"h": mean_c - slope * mean_m, "c": slope}


def calibrate(binary, workloads, output, deadline):
    """Warmups then six rounds; returns (pairs, failures), each indexed by workload and m."""
    data = {w.name: {m: [] for m in BLOCKS} for w in workloads}
    failures = {w.name: {} for w in workloads}
    attempts = (output / "results.jsonl").open("w", encoding="utf-8", newline="\n")

    def child(workload, blocks, label, index, position, order):
        name = f"{workload.name}-m{blocks}-{'warmup' if index is None else index}-{label}"
        record = {"case": workload.name, "blocks": blocks, "label": label, "warmup": index is None,
                  "pair_index": index, "order": order, "position": position,
                  "started_monotonic": time.perf_counter()}
        try:
            row = native(binary, workload.name, workload.iterations, output / f"{name}-stderr.txt",
                         deadline=deadline, blocks=blocks)
            record.update(row)
            error = child_error(row, workload, blocks)
            status = "failed" if error else "ok"
        except TimeoutError as exception:
            status = "incomplete_due_to_cap" if time.perf_counter() >= deadline else "failed"
            error = f"{type(exception).__name__}: {exception}"
        except (RuntimeError, OSError, EOFError, ValueError) as exception:
            status, error = "failed", f"{type(exception).__name__}: {exception}"
        record.update(status=status, error=error, ended_monotonic=time.perf_counter())
        attempts.write(json.dumps(record, separators=(",", ":")) + "\n")
        attempts.flush()
        return status, record

    def pair(workload, blocks, index):
        """Returns False once the deadline stops sampling; failures are recorded and skipped."""
        order = 1 if index is None else ORDER[index - 1]
        records = {}
        for position, label in enumerate(("baseline", "candidate") if order == 1 else ("candidate", "baseline")):
            if time.perf_counter() >= deadline:
                return False
            status, records[label] = child(workload, blocks, label, index, position, order)
            if status == "incomplete_due_to_cap":
                return False
            if status != "ok":
                where = "warmup" if index is None else f"pair {index}"
                failures[workload.name][blocks] = f"{label} failed at {where}: {records[label]['error']}"
                return True
        if index is not None:
            data[workload.name][blocks].append({
                "pair_index": index, "order": order,
                **{f"{label}_blocks": [t["wallSeconds"] for t in records[label]["workload"]["blocks"]]
                   for label in records},
                "elapsed_seconds": sum(records[label]["process_wall_seconds"] for label in records)})
        return True

    try:
        schedule = [(w, m, None) for w in workloads for m in BLOCKS]
        schedule += [(w, m, index) for index, ms in enumerate(BLOCK_ORDER, 1) for w in workloads for m in ms]
        for workload, blocks, index in schedule:
            # A failed warmup cannot qualify later pairs; do not retry failed cells.
            if blocks in failures[workload.name]:
                continue
            if not pair(workload, blocks, index):
                break
    finally:
        attempts.close()
    return data, failures


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--binary", required=True, type=Path, help="built perf_baseline; copied before use")
    parser.add_argument("--group", required=True, choices=sorted(GROUPS))
    parser.add_argument("--output", required=True, type=Path, help="new output directory")
    args = parser.parse_args()
    job_start = float(os.environ["PR_REPORT_JOB_START"])
    if not (math.isfinite(job_start) and 0 <= time.monotonic() - job_start < JOB_TIMEOUT_SECONDS):
        parser.error("PR_REPORT_JOB_START must be a recent time.monotonic() reading")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = output / f"bin/{args.binary.name}"
    binary.parent.mkdir()
    shutil.copy2(args.binary, binary)
    sha = digest(binary)
    catalog = load_catalog(binary)
    workloads = catalog.select(GROUPS[args.group])
    stage = time.perf_counter()
    deadline = min(stage + STAGE_CAP_SECONDS,
                   stage + job_start + JOB_TIMEOUT_SECONDS - RESERVE_SECONDS - time.monotonic())
    manifest = {"plan": PLAN, "group": args.group, "executable_sha256": sha, "catalog_sha256": catalog.sha256,
                "seed": catalog.seed, "rustc": subprocess.check_output(["rustc", "-Vv"], text=True, timeout=30),
                "platform": platform.platform(), "processor": platform.processor(), "cpus": os.cpu_count(),
                "runner": {key: os.environ.get(key) for key in ("RUNNER_OS", "RUNNER_ARCH", "ImageOS",
                                                                "ImageVersion", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT")},
                "workloads": [{"name": w.name, "operations_per_block": w.iterations,
                               "definitionSha256": w.fingerprint} for w in workloads],
                "blocks": BLOCKS, "block_order": BLOCK_ORDER, "pair_order": ORDER,
                "warmup": "one discarded baseline-then-candidate pair per workload and m before counted pairs",
                "stage_budget_seconds": deadline - stage}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
    data, failures = calibrate(binary, workloads, output, deadline)
    summary = {"plan": PLAN, "stage_seconds": time.perf_counter() - stage,
               "executable_unchanged": digest(binary) == sha, "workloads": {}}
    for name, cells in data.items():
        estimates = [estimate(cells[m], m, failures[name].get(m)) for m in BLOCKS]
        summary["workloads"][name] = {"cells": estimates, "cost_model": cost_model(estimates), "pairs": cells}
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8", newline="\n")
    incomplete = sorted(f"{n} m={c['blocks']}" for n, w in summary["workloads"].items()
                        for c in w["cells"] if c["status"] != "complete")
    print(f"[calibration] {summary['stage_seconds']:.0f} s; incomplete cells: {', '.join(incomplete) or 'none'}")
    return 0 if summary["executable_unchanged"] else 1


if __name__ == "__main__":
    sys.exit(main())
