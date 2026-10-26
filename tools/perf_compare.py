"""Alternate baseline/candidate release harnesses within one quiet window.

Copied-binary mode (--baseline/--candidate) times two copied harnesses; no builds or tracing.
--base REF builds REF and the current working tree through perf_build, checks
full-state trace parity, records step profiles, then runs the same paired timings.
Workloads come from both harnesses' `--list` catalogs, whose plans must match.
Uses the same operation and Windows process counters as perf_baseline.py; no host
listeners or environment changes. --max-wall-seconds shares a deadline across
metadata and workload children (after the builds in --base mode); reserve cleanup
time separately. See docs/performance.md#comparison-deadlines.

`sequential()` is the PR comparison's error-controlled sampler (tools/pr_report.py);
these command-line modes keep a fixed number of descriptive pairs.

Exit 0: complete and every trace equal. Exit 3: complete, but a trace or workload
definition changed (timings kept, labeled behavior-changed). Exit 1: incomplete;
failure.json and partial outputs remain.
"""

import argparse
import json
import math
from pathlib import Path
import statistics
import sys
import time

from perf_baseline import ROOT, SUITES, capture_trace, command_output, digest, load_catalog, native, same_plan
import perf_build
import perf_stats


# examples/perf_observation.rs has no --list, so its cases keep this guarded table.
# The first six are also the cases of the pinned perf_observation_phases recipe.
FROZEN_OBSERVATION = {"observe-dense": 1000, "observe-compact": 500,
                      "observe-wide": 10000, "observe-strip": 1000,
                      "observe-one": 10000, "observe-empty": 100000,
                      "observe-sparse": 500, "observe-clustered": 2000}
CHANGED = 3


def resolve_plan(binaries, cases, suite, *, deadline=None):
    """Return ({case: iterations}, catalogs, selections); frozen observation has no catalogs."""
    if cases and all(case in FROZEN_OBSERVATION for case in cases):
        if len(set(cases)) != len(cases):
            raise ValueError("duplicate workload names requested")
        return {case: FROZEN_OBSERVATION[case] for case in cases}, None, None
    if cases and any(case in FROZEN_OBSERVATION for case in cases):
        raise ValueError("observation cases need perf_observation executables; "
                         "run them separately from perf_baseline catalog workloads")
    budget = {"deadline": deadline} if deadline is not None else {}
    catalogs = {label: load_catalog(path, **budget) for label, path in binaries.items()}
    selections = {label: catalog.select(cases, suite) for label, catalog in catalogs.items()}
    same_plan(selections)
    workloads = selections["candidate"]
    if not workloads:
        raise ValueError(f"no available workloads selected from suite {suite}")
    return {w.name: w.iterations for w in workloads}, catalogs, selections


def manifest(binaries, hashes, plan, catalogs, repeats, max_wall_seconds, budget, **extra):
    return {"schema": "slither-perf-compare-v1",
            "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "source_commit": command_output("git", "rev-parse", "HEAD", **budget),
            "source_status": command_output("git", "status", "--short", **budget),
            "rustc": command_output("rustc", "-Vv", **budget),
            "harnesses": {key: {"path": str(path), "sha256": hashes[key]}
                          for key, path in binaries.items()},
            "workloads": plan,
            "catalogs": {label: catalog.record() for label, catalog in catalogs.items()} if catalogs else None,
            "repeats": repeats, "warmup_repetitions": 1,
            "max_wall_seconds": max_wall_seconds,
            "deadline_scope": "metadata and all workload children share one deadline; cleanup can take up to 10 additional seconds; no OS CPU/memory cap",
            "order": "alternate label order per repetition and case; adjacent matched pairs",
            "notes": "Native wall excludes setup/reset/output drop; process CPU includes them. Observation harness separately reports outputDrop wall intervals. Quiet agreement and build identities recorded in report.",
            **extra}


def compare(binaries, hashes, plan, output, repeats, *, deadline=None):
    """Adjacent alternating pairs after one warmup; writes results.jsonl and summary.json."""
    budget = {"deadline": deadline} if deadline is not None else {}
    rows = []
    with (output / "results.jsonl").open("w", encoding="utf-8", newline="\n") as results:
        for repetition in range(-1, repeats):
            for case_index, case in enumerate(plan):
                order = ("baseline", "candidate") if (repetition + case_index) % 2 == 0 else ("candidate", "baseline")
                for label in order:
                    if deadline is not None and time.perf_counter() >= deadline:
                        raise TimeoutError("performance run deadline exceeded before next child")
                    row = native(binaries[label], case, plan[case],
                                 output / f"{case}-{repetition}-{label}-stderr.txt", **budget)
                    row.update({"label": label, "repetition": repetition})
                    results.write(json.dumps(row, separators=(",", ":")) + "\n")
                    results.flush()
                    rows.append(row)
                    print(json.dumps({"case": case, "repetition": repetition, "label": label,
                                      "cpu": row["cpu_seconds"],
                                      "inner_wall": row["workload"]["timing"]["wallSeconds"]}), flush=True)
    if deadline is not None and time.perf_counter() >= deadline:
        raise TimeoutError("performance run deadline exceeded before completion")
    if hashes != {key: digest(path) for key, path in binaries.items()}:
        raise RuntimeError("benchmark executable changed during comparison")
    summary = {}
    for case in plan:
        pairs = []
        for repetition in range(repeats):
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
        summary[case] = {"iterations": plan[case], "pairs": pairs,
                         "median_wall_ratio_candidate_over_baseline": statistics.median(
                             pair["wall_ratio_candidate_over_baseline"] for pair in pairs),
                         "median_cpu_ratio_candidate_over_baseline": statistics.median(cpu_ratios) if cpu_ratios else None}
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8", newline="\n")
    return summary


def child_error(row, workload):
    """Why a harness timing record cannot be one side of a matched pair, or None."""
    report = row.get("workload")
    timing = report.get("timing") if isinstance(report, dict) else None
    if not isinstance(timing, dict):
        return "no timing record"
    # The report's operation name is the harness's internal label, not the catalog's;
    # the frozen plan pins the operation through the workload definition hash.
    if (report.get("kind"), report.get("case"), report.get("mode"),
            report.get("iterations")) != ("report", workload.name, "timing", workload.iterations):
        return "record does not match the plan's case or operation count"
    if type(timing.get("operations")) is not int or timing["operations"] != workload.iterations:
        return f"timed {timing.get('operations')!r} operations instead of {workload.iterations}"
    wall = timing.get("wallSeconds")
    if type(wall) not in (int, float) or not math.isfinite(wall) or wall <= 0:
        return f"invalid operation wall time {wall!r}"
    return None


def sequential(binaries, hashes, workloads, output, deadline, plan, *, checkpoint=None):
    """The PR comparison sampler: statistics contract v1 §1, §2 and §6.

    `workloads` are in frozen catalog order with matched plans on both sides; `deadline`
    is a perf_counter() value. Writes plan.json before sampling, every child attempt to
    results.jsonl and every complete pair to pairs.jsonl. Returns {case: perf_stats.final row}.
    """
    write_json(output / "plan.json", plan)
    rows = {w.name: {"pairs": [], "history": [], "termination": None, "failure": None, "attempted": 0}
            for w in workloads}
    results = (output / "results.jsonl").open("w", encoding="utf-8", newline="\n")
    pairs_log = (output / "pairs.jsonl").open("w", encoding="utf-8", newline="\n")

    def child(workload, label, index, position, order):
        """Returns (status, record): ok, failed or incomplete_due_to_cap."""
        name = "warmup" if index is None else index
        record = {"case": workload.name, "label": label, "warmup": index is None, "pair_index": index,
                  "order": order, "position": position, "started_monotonic": time.perf_counter()}
        try:
            row = native(binaries[label], workload.name, workload.iterations,
                         output / f"{workload.name}-{name}-{label}-stderr.txt", deadline=deadline)
            error = child_error(row, workload)
            record.update(row)
            status = "failed" if error else "ok"
        except TimeoutError as exception:
            # Distinguish the global deadline from a hung child at its own 120 s limit.
            status = "incomplete_due_to_cap" if time.perf_counter() >= deadline else "failed"
            error = f"{type(exception).__name__}: {exception}"
        except (RuntimeError, OSError, EOFError, ValueError) as exception:
            status, error = "failed", f"{type(exception).__name__}: {exception}"
        record.update(status=status, error=error, ended_monotonic=time.perf_counter())
        results.write(json.dumps(record, separators=(",", ":")) + "\n")
        results.flush()
        return status, record

    def pair(workload, index):
        """One adjacent pair (§2); a warmup has index None and runs baseline first."""
        order = 1 if index is None else perf_stats.ORDER[index - 1]
        labels = ("baseline", "candidate") if order == 1 else ("candidate", "baseline")
        records = {}
        for position, label in enumerate(labels):
            if time.perf_counter() >= deadline:  # §6: never start a child at or after D
                return "incomplete_due_to_cap", None
            status, records[label] = child(workload, label, index, position, order)
            if status != "ok":
                where = "warmup" if index is None else f"pair {index}"
                return status, f"{label} failed at {where}"
        return "ok", records

    def stop(name, termination, failure=None):
        rows[name].update(termination=termination, failure=failure)

    corrupted = False

    def summary():
        nonlocal corrupted
        corrupted = corrupted or hashes != {key: digest(path) for key, path in binaries.items()}
        if corrupted:
            for name in rows:
                stop(name, "measurement_failure", "benchmark executable changed during sampling")
            active.clear()
        return {name: {**perf_stats.final(
                    row["history"], len(row["pairs"]), row["termination"] or "sampling", row["failure"]),
                       "n_attempted": row["attempted"]} for name, row in rows.items()}

    try:
        active = list(workloads)
        capped = False
        # §2: one fixed, discarded baseline-then-candidate warmup pair per workload.
        for workload in list(active):
            status, detail = pair(workload, None)
            if status == "incomplete_due_to_cap":
                capped = True
                break
            if status != "ok":
                stop(workload.name, "measurement_failure", detail)
                active.remove(workload)
        else:
            # Rounds in catalog order, one pair per active workload; every row keeps its own index.
            for round_index, _ in enumerate(perf_stats.ORDER, 1):
                for workload in list(active):
                    row = rows[workload.name]
                    index = len(row["pairs"]) + 1
                    row["attempted"] = index
                    status, detail = pair(workload, index)
                    if status == "incomplete_due_to_cap":
                        capped = True
                        break
                    if status != "ok":
                        stop(workload.name, "measurement_failure", detail)
                        active.remove(workload)
                        continue
                    record = {"workload_id": workload.name, "pair_index": index,
                              "order": perf_stats.ORDER[index - 1],
                              "baseline_wall_seconds": detail["baseline"]["workload"]["timing"]["wallSeconds"],
                              "candidate_wall_seconds": detail["candidate"]["workload"]["timing"]["wallSeconds"],
                              "operations_per_block": workload.iterations, "blocks": 1}
                    metric = plan.get("metrics", {}).get(workload.name, "wallSeconds")
                    record["metric"] = metric
                    if metric == "cpuSeconds":
                        for label in ("baseline", "candidate"):
                            record[f"{label}_cpu_seconds"] = detail[label]["workload"]["timing"].get("cpuSeconds")
                    row["pairs"].append(record)
                    pairs_log.write(json.dumps(record, separators=(",", ":")) + "\n")
                    pairs_log.flush()
                    print(json.dumps({"case": workload.name, "pair": index,
                                      "ratio": record["candidate_wall_seconds"] / record["baseline_wall_seconds"]}),
                          flush=True)
                    if index not in perf_stats.LOOKS:
                        continue
                    if not perf_stats.valid_pairs(row["pairs"], workload.iterations, 1):
                        stop(workload.name, "measurement_failure", f"invalid pair data at {index} pairs")
                        active.remove(workload)
                        continue
                    # Keep real wall values in raw pairs. The statistical routine
                    # consumes a view in the chosen unit, fixed before sampling.
                    selected = [{**p, **({f"{label}_wall_seconds": p[f"{label}_cpu_seconds"] for label in ("baseline", "candidate")}
                                                   if metric == "cpuSeconds" else {})} for p in row["pairs"]]
                    if not perf_stats.valid_pairs(selected, workload.iterations, 1):
                        stop(workload.name, "measurement_failure", f"invalid {metric} at {index} pairs")
                        active.remove(workload)
                        continue
                    look = perf_stats.analyze(selected)
                    if metric != "wallSeconds":
                        look["metric"] = metric
                        for label in ("baseline", "candidate"):
                            look[f"{label}_busy_wall_ms"] = 1000 * perf_stats.geometric_block_time([p[f"{label}_wall_seconds"] for p in row["pairs"]]) / workload.iterations
                    row["history"].append(look)
                    print(f"[performance] {workload.name}: {look['label']} at {index} pairs", flush=True)
                    if look["label"] == "Unavailable":
                        stop(workload.name, "measurement_failure", f"numeric failure at {index} pairs")
                    elif look["label"] in perf_stats.RESOLVED:
                        stop(workload.name, "resolved")
                    elif index == perf_stats.LOOKS[-1]:
                        stop(workload.name, "max_pairs")
                    else:
                        continue
                    active.remove(workload)
                if capped or not active:
                    break
                # Publish only covered complete rounds. The callback may pause for
                # artifact upload; no measurement child runs during that pause.
                if checkpoint is not None and round_index in perf_stats.LOOKS:
                    checkpoint(summary(), deadline)
                    if corrupted:
                        break
        # Deadline: rows never attempted (including after a capped warmup) did not start.
        for workload in active:
            stop(workload.name, "time_cap" if rows[workload.name]["attempted"] else "not_started")
    finally:
        results.close()
        pairs_log.close()
    final = summary()
    write_json(output / "sequential.json", final)
    return final


def parity(binaries, selections, output, *, deadline=None):
    """Trace each workload on both sides. Deadline expiry stops the run; other failures are Unavailable."""
    budget = {"deadline": deadline} if deadline is not None else {}
    rows = []
    for index, workload in enumerate(selections["candidate"]):
        traces, errors = {}, {}
        for label, binary in binaries.items():
            directory = output / f"traces-{label}"
            directory.mkdir(exist_ok=True)
            try:
                traces[label] = capture_trace(binary, workload.name, workload.trace_iterations, directory,
                                              terminal=not workload.legacy, **budget)
            except TimeoutError:
                raise
            except (RuntimeError, OSError, EOFError) as error:
                errors[label] = str(error)
        definitions = {label: selected[index].fingerprint for label, selected in selections.items()}
        rows.append({"case": workload.name, "traceIterations": workload.trace_iterations,
                     "reading": "Unavailable" if errors else
                     "Equal" if traces["baseline"]["sha256"] == traces["candidate"]["sha256"] else "Changed",
                     "definition": "Equal" if len(set(definitions.values())) == 1 else "Changed",
                     "synthetic": workload.synthetic, **traces, "errors": errors})
    return rows


def ratio(value):
    return "—" if value is None else f"{value:.3f}"


def markdown(summary, heading, parity_rows=None):
    readings = {row["case"]: row for row in parity_rows or []}
    lines = ["# Performance comparison", "", heading, ""]
    changed = [row["case"] for row in readings.values()
               if row["reading"] == "Changed" or row["definition"] == "Changed"]
    if changed:
        lines += [f"**Behavior changed** in {', '.join(f'`{case}`' for case in changed)}; "
                  "timings compare different outputs.", ""]
    lines += ["| Workload | Iterations | Trace | Wall ratio | CPU ratio | Wall pair range |",
              "| --- | ---: | --- | ---: | ---: | ---: |"]
    for case, data in summary.items():
        walls = [pair["wall_ratio_candidate_over_baseline"] for pair in data["pairs"]]
        trace = readings.get(case)
        reading = "—" if trace is None else trace["reading"] + (
            "; definition changed" if trace["definition"] == "Changed" else "")
        lines.append(f"| `{case}` | {data['iterations']} | {reading} | "
                     f"{ratio(data['median_wall_ratio_candidate_over_baseline'])} | "
                     f"{ratio(data['median_cpu_ratio_candidate_over_baseline'])} | "
                     f"{min(walls):.3f}–{max(walls):.3f} |")
    lines += ["", "Ratios are candidate/baseline medians of adjacent pairs after one warmup; "
              "below 1 is faster. They are descriptive, not a significance test.", ""]
    return "\n".join(lines)


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8", newline="\n")


def failure(output, error, started, max_wall_seconds):
    write_json(output / "failure.json", {"type": type(error).__name__, "error": str(error),
                                         "elapsed_wall_seconds": time.perf_counter() - started,
                                         "max_wall_seconds": max_wall_seconds,
                                         "diagnostics": "Child stderr, logs and partial results remain in this directory."})


def run_copied(args):
    binaries = {"baseline": args.baseline.resolve(strict=True),
                "candidate": args.candidate.resolve(strict=True)}
    hashes = {key: digest(path) for key, path in binaries.items()}
    args.output.mkdir(parents=True, exist_ok=False)
    started = time.perf_counter()
    deadline = started + args.max_wall_seconds if args.max_wall_seconds is not None else None
    budget = {"deadline": deadline} if deadline is not None else {}
    try:
        plan, catalogs, _ = resolve_plan(binaries, args.cases, args.suite, **budget)
        write_json(args.output / "manifest.json", manifest(
            binaries, hashes, plan, catalogs, args.repeats, args.max_wall_seconds, budget))
        summary = compare(binaries, hashes, plan, args.output, args.repeats, **budget)
        (args.output / "summary.md").write_text(markdown(summary, "Copied executables: " + ", ".join(
            f"{label} `{hashes[label][:12]}`" for label in binaries) + "."), encoding="utf-8", newline="\n")
    except BaseException as error:
        failure(args.output, error, started, args.max_wall_seconds)
        raise
    return 0


def run_base(args):
    output = args.output.resolve()
    perf_build.require_outside_snapshot(ROOT, output)
    output.mkdir(parents=True, exist_ok=False)
    started = time.perf_counter()
    try:
        builds = perf_build.prepare_local(ROOT, args.base, output)
        window = time.perf_counter()
        deadline = window + args.max_wall_seconds if args.max_wall_seconds is not None else None
        budget = {"deadline": deadline} if deadline is not None else {}
        binaries = {label: build.binaries["perf_baseline"] for label, build in builds.items()}
        hashes = {label: build.hashes[binaries[label].name] for label, build in builds.items()}
        for build in builds.values():
            build.verify()
        plan, catalogs, selections = resolve_plan(binaries, args.cases, args.suite, **budget)
        if catalogs is None:
            raise ValueError("--base builds perf_baseline only; compare observation cases with copied executables")
        identities = {label: build.record for label, build in builds.items()}
        write_json(output / "manifest.json", manifest(
            binaries, hashes, plan, catalogs, args.repeats, args.max_wall_seconds, budget,
            base_ref=args.base, identities=identities,
            deadline_start="after both builds; build time is not bounded by --max-wall-seconds"))
        comparison = {"schema": "slither-perf-comparison-v1", "base_ref": args.base,
                      "identities": identities, "suite": None if args.cases else args.suite,
                      "plan": [w.plan() for w in selections["candidate"]],
                      "catalogs": {label: catalog.sha256 for label, catalog in catalogs.items()},
                      "repeats": args.repeats, "complete": False}
        print("[parity] Capturing full-state traces", flush=True)
        comparison["parity"] = parity(binaries, selections, output, **budget)
        write_json(output / "comparison.json", comparison)
        unavailable = [row["case"] for row in comparison["parity"] if row["reading"] == "Unavailable"]
        if unavailable:
            raise RuntimeError(f"trace parity unavailable for {', '.join(unavailable)}; "
                               "see traces-*/ stderr files")
        if not args.no_profiles:
            print("[profile] Recording step profiles", flush=True)
            profiles = [{"label": label, "case": w.name,
                         **native(binaries[label], w.name, w.iterations,
                                  output / f"{w.name}-profile-{label}-stderr.txt", profile=True, **budget)}
                        for w in selections["candidate"] if w.profile for label in binaries]
            write_json(output / "profiles.json", profiles)
        print("[performance] Running alternating pairs", flush=True)
        summary = compare(binaries, hashes, plan, output, args.repeats, **budget)
        for build in builds.values():
            build.verify()
        changed = any(row["reading"] == "Changed" or row["definition"] == "Changed"
                      for row in comparison["parity"])
        heading = (f"Base `{args.base}` (`{identities['baseline']['commit'][:12]}`, candidate measurement "
                   f"inputs overlaid) → working tree (`{identities['candidate']['tree'][:12]}`). "
                   f"Pairs per workload: {args.repeats}.")
        (output / "summary.md").write_text(markdown(summary, heading, comparison["parity"]),
                                           encoding="utf-8", newline="\n")
        comparison.update({"complete": True, "behavior": "changed" if changed else "equal",
                           "exit_code": CHANGED if changed else 0})
        write_json(output / "comparison.json", comparison)
        return comparison["exit_code"]
    except BaseException as error:
        failure(output, error, started, args.max_wall_seconds)
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--base", metavar="REF", help="build REF and the working tree, then compare")
    source.add_argument("--baseline", type=Path, help="copied baseline harness (with --candidate)")
    parser.add_argument("--candidate", type=Path, help="copied candidate harness")
    parser.add_argument("--output", type=Path, required=True, help="new owned output directory")
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--max-wall-seconds", type=float,
                        help="shared metadata/workload deadline; allow up to 10s extra for child cleanup")
    parser.add_argument("--suite", choices=SUITES, default="ci",
                        help="catalog suite: ci rows or every available row (default ci)")
    parser.add_argument("--cases", nargs="+", metavar="CASE",
                        help="catalog names from `perf_baseline --list` in any suite, or perf_observation "
                             "cases; replaces the suite selection")
    parser.add_argument("--no-profiles", action="store_true", help="skip step profiles in --base mode")
    args = parser.parse_args(argv)
    if (args.baseline is None) != (args.candidate is None):
        parser.error("--baseline and --candidate are used together")
    if args.base is not None and args.candidate is not None:
        parser.error("--base replaces --baseline/--candidate")
    if not 1 <= args.repeats <= 10:
        parser.error("repeats must be 1..10")
    if args.max_wall_seconds is not None and (
            not math.isfinite(args.max_wall_seconds) or args.max_wall_seconds <= 0):
        parser.error("max-wall-seconds must be finite and positive")
    return run_base(args) if args.base is not None else run_copied(args)


if __name__ == "__main__":
    sys.exit(main())
