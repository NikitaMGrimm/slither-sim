"""Collect an advisory PR comparison over the catalog's ci suite; standard library only.

Binaries, logs and failed attempts are retained in fresh owned directories; the
baseline worktree is removed after the builds. Performance interpretation
requires a quiet runner and matched inputs.
"""

import argparse
import copy
from contextlib import contextmanager, nullcontext
import json
import math
import os
from pathlib import Path
import platform
import re
import subprocess
import time

from perf_baseline import capture_trace, digest, load_catalog, same_plan
import perf_build
from perf_build import HARNESS, TESTS
import perf_compare
import perf_stats


ROOT = Path(__file__).resolve().parents[1]
MARKER = "<!-- slither-report-preview -->"
# Operational v2 budget, provisional rather than a calibrated optimum.
# JOB_TIMEOUT_SECONDS must match the workflow's timeout-minutes; the reserve leaves
# time to write the report and upload artifacts before GitHub cancels the job.
JOB_TIMEOUT_SECONDS = 1800
RESERVE_SECONDS = 90
STAGE_CAP_SECONDS = 1500
RESULTS = {"Faster": "🟢 Faster", "Slower": "🔴 Slower", "Within": "⚪ Within ±5%",
           "Undecided": "🟡 Undecided", "Unavailable": "⚠️ Unavailable", "Redefined": "🟡 Redefined"}

# Timing selection and deterministic regression coverage have separate purposes.
REPORT_CASES = ("step-food", "observe", "official-100-step", "official-100-bot",
                "host-official-500", "host-battledome-80")
TRACE_CASES = ("step-default", "step-food", "step-body", "observe", "portable", "protocol",
               "official-100-step", "official-100-bot", "battledome-32-step", "client-battledome-64")


@contextmanager
def head_configs(catalog, output):
    """Resolve constructors on head, then supply the same Config to both runtimes."""
    configs = {}
    for workload in catalog.workloads:
        definition = json.loads(workload.definition)
        if "config" not in definition:
            continue
        configs[workload.name] = definition["config"]
        world = definition.get("world", {})
        kind = world.get("kind")
        key = {"official": f"Official({world.get('snakes')})", "sandbox": "Sandbox", "battledome": "Battledome"}.get(kind)
        if key:
            configs[f"world:{key}"] = definition["config"]
    path = output / "head-configs.json"
    path.write_text(json.dumps(configs, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    prior = os.environ.get("SLITHER_PERF_CONFIGS")
    os.environ["SLITHER_PERF_CONFIGS"] = str(path.resolve())
    try:
        yield
    finally:
        if prior is None:
            os.environ.pop("SLITHER_PERF_CONFIGS", None)
        else:
            os.environ["SLITHER_PERF_CONFIGS"] = prior


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True, timeout=30).strip()


def parse_tests(log):
    target = None
    results = {}
    for line in log.splitlines():
        match = re.search(r"Running tests[/\\](\w+)\.rs", line)
        if match:
            target = match[1]
        match = re.fullmatch(r"test (\S+) \.\.\. (ok|FAILED|ignored)(?:, .*)?", line.strip())
        if target and match:
            results[f"{target}::{match[1]}"] = match[2]
    return results


def transition(base, candidate):
    if base is None or candidate is None:
        return "unavailable"
    if base == "FAILED" and candidate == "ok":
        return "fixed"
    if base == "ok" and candidate == "FAILED":
        return "regressed"
    return "unchanged" if base == candidate else "changed"


def test_names(text):
    return set(re.findall(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+(\w+)", text))


def metric_rows(base, candidate):
    def index(report):
        if report is None:
            return {}
        if report.get("schema") != 1:
            raise ValueError("unsupported fidelity schema")
        result = {}
        for metric in report["metrics"]:
            if (metric["id"] in result or not isinstance(metric["samples"], int)
                    or metric["samples"] <= 0 or not math.isfinite(metric["value"])
                    or metric["value"] < 0):
                raise ValueError("invalid or duplicate fidelity metric")
            result[metric["id"]] = metric
        return result

    before, after = index(base), index(candidate)
    rows = []
    for key in sorted(before.keys() | after.keys()):
        a, b = before.get(key), after.get(key)
        row = {"id": key, "baseline": a, "candidate": b, "reading": "Unavailable"}
        if a and b and a["unit"] == b["unit"] and a["samples"] == b["samples"]:
            delta = b["value"] - a["value"]
            row["absolute_delta"] = delta
            row["reading"] = ("Unchanged" if math.isclose(a["value"], b["value"], rel_tol=1e-9, abs_tol=1e-12)
                              else "Lower error" if delta < 0 else "Higher error")
        rows.append(row)
    return rows


def cell(value):
    return str(value).replace("|", "\\|").replace("\n", " ").replace("\r", " ").replace("`", "'")


def status(reading):
    icon = {"Lower error": "🟢", "Equal": "🟢", "Higher error": "🔴",
            "Changed": "🟡", "Unavailable": "⚠️"}.get(reading, "⚪")
    return f"{icon} {reading}"


def percent(value):
    """Two decimals with a true minus sign; a value that rounds to zero has no sign."""
    text = f"{abs(value):.2f}%"
    return text if text == "0.00%" else ("−" if value < 0 else "+") + text


def change_cell(result):
    """Estimate and asymmetric interval (contract §4); rounding never affects the label."""
    if result is None:
        return "—"
    if result["interval_percent"] is None:
        return f"{percent(result['change_percent'])} [interval unavailable]"
    low, high = result["interval_percent"]
    return f"{percent(result['change_percent'])} [{percent(low)}, {percent(high)}]"


def stopping(row):
    """One Details line: pairs used and why sampling stopped."""
    name, used, complete = f"`{cell(row['case'])}`", row["n_used"], row["n_complete"]
    if row["label"] == "Redefined":
        return f"- {name}: head only, redefined; {cell(row['failure'])}."
    if row["label"] == "Unavailable":
        why = {"measurement_failure": f"{row['failure'] or row['reason'].replace('_', ' ')} (see raw logs)",
               "not_started": "time cap before sampling started"}.get(
            row["termination"], f"time cap before {perf_stats.LOOKS[0]} complete pairs ({complete} collected)")
        return f"- {name}: unavailable; {cell(why)}."
    parts = [f"{used} pairs used"]
    if row["reason"] == "degenerate_residuals":
        parts.append("interval unavailable (degenerate residuals)")
    parts.append({"sampling": "sampling continues", "resolved": "stopped when resolved", "max_pairs": "stopped at the last look"}.get(
        row["termination"], "time cap" + (f" ({complete} complete pairs collected)" if complete > used else "")))
    return f"- {name}: " + "; ".join(parts) + "."


def render(report):
    performance = report.get("performance", [])
    lines = [MARKER, "## Comparison", "",
             f"Base `{report['base'][:8]}` → merge `{report['merge'][:8]}` · [Run and data]({report['run_url']})", "",
             "### Performance", "", "Operation wall time, ms/op. Negative change means less time.", "",
             "| Workload | Base | PR | Change | Result |",
             "| --- | ---: | ---: | ---: | --- |"]
    if any(row.get("result", {}).get("metric") == "cpuSeconds" for row in performance if row.get("result")):
        lines[7] = "ms/op; CPU marks host process CPU per tick; other rows use operation wall time. Negative change means less time."
    for row in performance:
        result = row["result"]
        costs = [f"{result[f'{label}_ms_per_operation']:.5f}" if result else "—" for label in ("baseline", "candidate")]
        if "head_ms_per_operation" in row:
            costs[1] = f"{row['head_ms_per_operation']:.5f}"
        name = row['case'] + (" · CPU" if result and result.get("metric") == "cpuSeconds" else "")
        lines.append(f"| {cell(name)} | {costs[0]} | {costs[1]} | {change_cell(result)} | {RESULTS[row['label']]} |")
    if not performance:
        lines.append("| — | — | — | — | ⚠️ Unavailable |")
    lines += ["", "### Regression and fidelity", "",
              "RMSE: turns in rad/s; speed ramps in protocol speed/s.", "",
              "| Measurement | Base | PR | Result | Cases |",
              "| --- | ---: | ---: | --- | ---: |"]
    labels = {"turn-large-rmse": "Turn error · large snakes", "turn-small-rmse": "Turn error · small snakes",
              "speed-ramp-rmse:run3-large-radius-20260927": "Speed ramp · large radius",
              "speed-ramp-rmse:run3-single-20260927": "Speed ramp · single",
              "speed-ramp-rmse:visibility-covariates-20260928": "Speed ramp · visibility"}
    for row in report.get("fidelity", []):
        a, b = row["baseline"], row["candidate"]
        ref = b or a
        av = f"{a['value']:.6f}" if a else "—"
        bv = f"{b['value']:.6f}" if b else "—"
        label = labels.get(row["id"], f"{ref['label']} ({ref['unit']})")
        lines.append(f"| {cell(label)} | {av} | {bv} | {status(row['reading'])} | {ref['samples']} |")
    before, after = report.get("tests", {}).get("baseline", {}), report.get("tests", {}).get("candidate", {})
    names = sorted(set(report.get("expected_tests", [])) | before.keys() | after.keys())
    changes = {kind: sum(transition(before.get(n), after.get(n)) == kind for n in names)
               for kind in ("fixed", "regressed", "unavailable", "changed", "unchanged")}
    def passed(values):
        if not names:
            return "Unavailable"
        missing = sum(name not in values for name in names)
        value = f"{sum(values.get(name) == 'ok' for name in names)}/{len(names)}"
        return value + (f"; {missing} unavailable" if missing else "")

    test_result = [f"{icon} {changes[key]} {label}" for key, icon, label in (
        ("fixed", "🟢", "fixed"), ("regressed", "🔴", "regressed"),
        ("unavailable", "⚠️", "unavailable"), ("changed", "🟡", "changed")) if changes[key]]
    if not test_result:
        failing = sum(after.get(name) == "FAILED" for name in names)
        if not names:
            test_result = ["⚠️ Unavailable"]
        elif failing:
            test_result = [f"🔴 {failing} failing"]
        elif all(after.get(name) == "ok" for name in names):
            test_result = ["🟢 Passed"]
        else:
            test_result = ["⚪ Unchanged"]
    lines.append(f"| Regression tests | {passed(before)} | {passed(after)} | {' · '.join(test_result)} | {len(names)} |")
    traces = report.get("traces", [])
    equal = sum(row["reading"] == "Equal" for row in traces)
    trace_status = ("Unavailable" if not traces or any(t["reading"] == "Unavailable" for t in traces)
                    else "Equal" if equal == len(traces) else "Changed")
    lines.append(f"| State traces | Reference | {equal}/{len(traces)} equal | {status(trace_status)} | {len(traces)} |")
    if report.get("errors") or changes["unavailable"]:
        lines += ["", "⚠️ **Incomplete report:** see details for missing results."]
    lines += ["", "<details><summary>Details</summary>", "",
              f"- PR head: `{report['head']}`. Runner: {cell(report['platform'])}.",
              "- Paired operation wall time after warmup, with balanced order and covered checks at 8/12/26, "
              "then doubling through 53,248 pairs (finite safety guard). "
              "Base/PR are geometric means; Change shows estimate [interval] against a −5% to +5% band.",
              "- V2 order-adjusted t intervals provide ≥95% simultaneous coverage across 10 workloads and 14 pre-budgeted looks "
              "under independent normal log errors. Resolved rows stop; time-capped rows retain their last "
              "completed look. Labels use unrounded bounds.",
              # Remove only after hosted calibration has run and been reviewed.
              "- Hosted model calibration is not established; performance labels are advisory and conditional on the timing model.",
              "- Both runtimes use the same selected test sources and fixtures. Errors describe those fixtures, not overall accuracy.",
              "- Turn errors are duration weighted; speed ramps are grouped by source. Changed traces indicate changed behavior."]
    for row in performance:
        result = row.get("result") or {}
        if result.get("metric") == "cpuSeconds":
            lines.append(f"- `{cell(row['case'])}` simulation busy wall: {result['baseline_busy_wall_ms']:.5f} → "
                         f"{result['candidate_busy_wall_ms']:.5f} ms/tick (diagnostic, same covered pairs).")
    for name, data in report.get("metric_probe", {}).get("workloads", {}).items():
        lines.append(f"- `{cell(name)}` metric: {cell(data['selected'])}; A/A order-adjusted log SD: "
                     f"CPU {data.get('cpu_sd_percent')}%, operation/busy wall {data.get('wall_sd_percent')}%. "
                     "Raw records retain CPU scope, wall time and elapsed time; socket units are ms/tick.")
    if traces:
        lines.append("- State trace windows (operations): " + ", ".join(
            f"`{cell(t['case'])}` {t.get('iterations', '—')}" for t in traces) + ".")
    lines += [stopping(row) for row in performance]
    for name in names:
        if before.get(name) != after.get(name) or after.get(name) != "ok":
            lines.append(f"- `{cell(name)}`: {before.get(name, 'unavailable')} → {after.get(name, 'unavailable')} "
                         f"({transition(before.get(name), after.get(name))}).")
    for change in report.get("suite_changes", []):
        lines.append(f"- PR-pinned input changed: `{cell(change)}`.")
    for name in report.get("removed_tests", []):
        lines.append(f"- Test removed from selected PR sources: `{cell(name)}`; removal is not counted as a fix.")
    for name in report.get("added_tests", []):
        lines.append(f"- Added test: `{cell(name)}`; evaluated on both runtimes.")
    for trace in traces:
        if trace["reading"] != "Equal":
            lines.append(f"- `{trace['case']}` trace: {trace['reading']}.")
    for error in report.get("errors", []):
        lines.append(f"- **Incomplete:** {cell(error)}.")
    lines += ["- Measurements use PR code and the selected fixtures; inspect the raw data for any disputed result.",
              "- Raw results, hashes and logs are linked above.", "", "</details>", ""]
    if report.get("snapshot", {}).get("state") == "sampling":
        lines[3:3] = ["**Sampling in progress:** provisional covered checkpoint; a failed or cancelled run withdraws these claims.", ""]
    return "\n".join(lines)


def save_report(output, report):
    """A snapshot directory is exclusively created and never changed after ready."""
    output.mkdir(parents=True, exist_ok=False)
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    (output / "report.md").write_text(render(report), encoding="utf-8", newline="\n")
    (output / "ready").touch(exist_ok=False)


class Snapshots:
    def __init__(self, output):
        self.output, self.sequence = output, 0

    def checkpoint(self, report, deadline):
        if self.sequence >= 3:
            return
        self.sequence += 1
        snapshot = copy.deepcopy(report)
        snapshot["snapshot"] = {"schema": "slither-pr-progress-v1", "sequence": self.sequence, "state": "sampling"}
        directory = self.output / "progress" / str(self.sequence)
        save_report(directory, snapshot)
        if os.environ.get("PR_REPORT_CHECKPOINT_UPLOAD") == "1":
            started = time.perf_counter()
            # The next workflow step acknowledges only after upload completes.
            # An expired pause cannot resume timing: the sampler checks D first.
            while not (directory / "resume").exists() and time.perf_counter() < deadline:
                time.sleep(min(0.25, max(0, deadline - time.perf_counter())))
            with (self.output / "checkpoint-pauses.jsonl").open("a", encoding="utf-8") as log:
                log.write(json.dumps({"sequence": self.sequence, "seconds": time.perf_counter() - started}) + "\n")


def collect(output, head, job_start):
    """`job_start` is the time.monotonic() anchor recorded by the job's first step."""
    phases = {"collector": time.monotonic() - job_start}
    merge = git("rev-parse", "HEAD")
    base = git("rev-parse", "HEAD^1")
    if git("rev-parse", "HEAD^2") != head:
        raise ValueError("checkout must be the PR merge commit for the expected head")
    report = {"schema": 1, "base": base, "head": head, "merge": merge,
              "repository": os.environ["GITHUB_REPOSITORY"], "pr": int(os.environ["PR_NUMBER"]),
              "run": int(os.environ["GITHUB_RUN_ID"]), "attempt": int(os.environ["GITHUB_RUN_ATTEMPT"]),
              "run_url": f"{os.environ['GITHUB_SERVER_URL']}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}",
              "platform": platform.platform(), "errors": [], "tests": {}, "traces": [],
              "phases_seconds_after_job_start": phases}
    work = output.with_name(output.name + "-work")
    fidelity = {}

    def measured(snapshot, build, environment):
        label = snapshot.label
        test_log = output / f"{label}-tests.log"
        print(f"[{label}] Running the selected regression suite", flush=True)
        code = perf_build.command(["cargo", "test", "--locked", "--no-fail-fast", "--test", "movement_turns", "--test", "physics", "--", "--test-threads=1"],
                                  snapshot.checkout, test_log, environment)
        report["tests"][label] = parse_tests(test_log.read_text(encoding="utf-8"))
        if code:
            report["errors"].append(f"{label} selected tests exited {code}")
        if build is None:
            return
        metrics_log = output / f"{label}-fidelity.json"
        if perf_build.command([str(build.binaries["fidelity_metrics"]), str(ROOT / "tests/fixtures")], snapshot.checkout, metrics_log):
            report["errors"].append(f"{label} fidelity metrics unavailable")
        else:
            fidelity[label] = json.loads(metrics_log.read_text(encoding="utf-8"))

    with perf_build.Session(ROOT, work) as session:
        baseline = session.worktree("baseline", base)
        candidate = perf_build.checkout_snapshot(ROOT, "candidate")
        report["added_tests"], report["removed_tests"], report["expected_tests"] = [], [], []
        for name in TESTS:
            path = Path("tests") / f"{name}.rs"
            old, new = test_names((baseline.checkout / path).read_text()), test_names((ROOT / path).read_text())
            report["expected_tests"] += [f"{name}::{n}" for n in sorted(new)]
            report["added_tests"] += [f"{name}::{n}" for n in sorted(new - old)]
            report["removed_tests"] += [f"{name}::{n}" for n in sorted(old - new)]
        baseline = perf_build.overlay(candidate, baseline)
        report["inputs"], report["suite_changes"] = baseline.inputs, list(baseline.suite_changes)
        report["rustc"] = subprocess.check_output(["rustc", "-Vv"], text=True, timeout=30)
        report["cargo_locks"] = {s.label: digest(s.checkout / "Cargo.lock") for s in (baseline, candidate)}
        phases["builds"] = time.monotonic() - job_start
        # The checkout's default target, where the workflow restores the dependency cache.
        builds, errors = perf_build.build_all(
            [baseline, candidate], perf_build.SharedTarget(ROOT / "target"), ("perf_baseline", "fidelity_metrics"),
            logs=output, bins=work, hook=measured)
    report["errors"] += errors
    report["binaries"] = {label: build.hashes for label, build in builds.items()}
    report["identities"] = {label: build.record for label, build in builds.items()}
    report["fidelity"] = metric_rows(fidelity.get("baseline"), fidelity.get("candidate"))
    binaries = {label: build.binaries["perf_baseline"] for label, build in builds.items()}
    primary_catalog = load_catalog(binaries["candidate"]) if "candidate" in binaries else None
    with head_configs(primary_catalog, output) if primary_catalog else nullcontext():
        catalogs, suites = {}, {}
        for label, binary in binaries.items():
            try:
                catalogs[label] = load_catalog(binary)
                suites[label] = catalogs[label].select(cases=REPORT_CASES)
            except (RuntimeError, ValueError, OSError, subprocess.TimeoutExpired) as error:
                report["errors"].append(f"{label} workload catalog: {error}")
        if len(suites) == 2:
            try:
                same_plan(suites)
                if catalogs["baseline"].seed != catalogs["candidate"].seed:
                    raise ValueError("baseline and candidate catalog seeds differ")
                if len(suites["candidate"]) > perf_stats.WORKLOAD_SLOTS:
                    raise ValueError(f"more than {perf_stats.WORKLOAD_SLOTS} ci workloads need a new error budget")
            except ValueError as error:
                report["errors"].append(str(error))
                suites = {}
        workloads = suites.get("candidate", suites.get("baseline", ()))
        report["workloads"] = [w.plan() for w in workloads]
        phases["parity"] = time.monotonic() - job_start
        trace_catalog = catalogs.get("candidate", catalogs.get("baseline"))
        trace_workloads = tuple(w for w in trace_catalog.workloads if w.name in TRACE_CASES and w.available) if trace_catalog else ()
        for workload in trace_workloads:
            print(f"[parity] Capturing {workload.name} traces", flush=True)
            hashes = {}
            for label in suites:
                directory = output / f"traces-{label}"
                directory.mkdir(exist_ok=True)
                try:
                    hashes[label] = capture_trace(binaries[label], workload.name, workload.trace_iterations,
                                                  directory, terminal=not workload.legacy)
                except Exception as error:
                    report["errors"].append(f"{label} {workload.name} trace: {error}")
            report["traces"].append({"case": workload.name, "iterations": workload.trace_iterations, **hashes,
                                     "reading": "Unavailable" if len(hashes) != 2
                                     else "Equal" if hashes["baseline"]["sha256"] == hashes["candidate"]["sha256"] else "Changed"})
        if "candidate" in suites and workloads:
            from perf_report_probe import probe
            report["metric_probe"] = probe(binaries["candidate"], suites["candidate"], output / "metric-probe",
                                           deadline=job_start + JOB_TIMEOUT_SECONDS - RESERVE_SECONDS)
            measure(report, binaries, {label: builds[label].hashes[binaries[label].name] for label in binaries},
                    catalogs, {"baseline": (), **suites}, output / "performance", job_start)
        phases["finished"] = time.monotonic() - job_start
        return report


def measure(report, binaries, hashes, catalogs, suites, output, job_start):
    """Freeze the plan, then sample until each row stops or the stage deadline (contract §1, §6)."""
    stage = time.perf_counter()
    report["phases_seconds_after_job_start"]["performance"] = time.monotonic() - job_start
    # Convert the cross-process monotonic anchor to this process's perf_counter() clock.
    job_deadline = stage + job_start + JOB_TIMEOUT_SECONDS - RESERVE_SECONDS - time.monotonic()
    deadline = min(stage + STAGE_CAP_SECONDS, job_deadline)
    # A row whose definition differs measures different inputs on each side.
    matched = [w for w, other in zip(suites["candidate"], suites["baseline"])
               if (w.fingerprint, w.operation) == (other.fingerprint, other.operation)]
    redefined = {}
    for w in suites["candidate"]:
        if w in matched:
            continue
        reason = "runtime resolves a different definition even with the head Config; comparison withheld"
        try:
            from perf_baseline import native
            result = native(binaries["candidate"], w.name, w.iterations, output.parent / f"{w.name}-redefined-stderr.txt", deadline=deadline)
            error = perf_compare.child_error(result, w)
            if error:
                raise ValueError(error)
            redefined[w.name] = {**perf_stats.final([], 0, "measurement_failure", reason), "label": "Redefined",
                                 "head_ms_per_operation": result["workload"]["timing"]["wallSeconds"] * 1000 / w.iterations,
                                 "n_attempted": 0}
        except Exception as error:
            report["errors"].append(f"{w.name} redefined head measurement: {error}")
    metrics = {w.name: report.get("metric_probe", {}).get("workloads", {}).get(w.name, {}).get("selected", "wallSeconds") for w in matched}
    plan = {"plan": "slither-perf-sequential-v3", "seed": catalogs["candidate"].seed,
            "metric": "operation wall time; socket metric fixed by retained A/A probe",
            "metrics": metrics,
            "catalogs": {label: catalog.sha256 for label, catalog in catalogs.items()},
            "executables": hashes,
            "workloads": [{"name": w.name, "operation": w.operation, "definitionSha256": w.fingerprint,
                           "operations_per_block": w.iterations, "blocks": 1} for w in matched],
            "warmup": "one discarded baseline-then-candidate pair per workload, after the harness's fixed "
                      "internal warmup in every process",
            "order": perf_stats.ORDER, "looks": perf_stats.LOOKS, "band": [0.95, 1.05],
            "eta_denominators": perf_stats.ETA_DENOMINATORS,
            "critical": {str(df): value for df, value in perf_stats.T_CRITICAL.items()},
            "deadline": {"stage_cap_seconds": STAGE_CAP_SECONDS, "job_timeout_seconds": JOB_TIMEOUT_SECONDS,
                         "reserve_seconds": RESERVE_SECONDS, "stage_budget_seconds": deadline - stage}}
    print(f"[performance] Sampling for at most {max(0, deadline - stage):.0f} s", flush=True)
    output.mkdir()
    snapshots = Snapshots(output.parent)
    mismatch = {**perf_stats.final([], 0, "measurement_failure", "workload definitions differ between base and PR"),
                "n_attempted": 0}

    def checkpoint(rows, deadline):
        report["performance"] = [{"case": w.name, **rows.get(w.name, redefined.get(w.name, mismatch))} for w in suites["candidate"]]
        report["performance_plan_sha256"] = digest(output / "plan.json")
        snapshots.checkpoint(report, deadline)

    try:
        options = {"checkpoint": checkpoint} if os.environ.get("PR_REPORT_CHECKPOINT_UPLOAD") == "1" else {}
        rows = perf_compare.sequential(binaries, hashes, matched, output, deadline, plan, **options)
    except Exception as error:  # Infrastructure failure: the report is incomplete, not a timing claim.
        failure = f"performance sampler: {type(error).__name__}: {error}"
        report["errors"].append(failure)
        prior = {r["case"]: r for r in report.get("performance", [])}
        report["performance"] = [{"case": w.name, **perf_stats.final(
            prior.get(w.name, {}).get("history", []), prior.get(w.name, {}).get("n_complete", 0),
            "measurement_failure", failure), "n_attempted": prior.get(w.name, {}).get("n_attempted", 0)}
            for w in suites["candidate"]]
        return
    report["performance"] = [{"case": w.name, **rows.get(w.name, redefined.get(w.name, mismatch))} for w in suites["candidate"]]
    report["performance_plan_sha256"] = digest(output / "plan.json")
    report["budget"] = {"stage_budget_seconds": deadline - stage, "stage_seconds": time.perf_counter() - stage,
                        "limited_by": "stage cap" if stage + STAGE_CAP_SECONDS <= job_deadline else "job reserve"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--head", required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.head):
        parser.error("head must be a full Git SHA")
    # The workflow's first step records time.monotonic(), a system-wide clock on the runner.
    job_start = float(os.environ["PR_REPORT_JOB_START"])
    if not (math.isfinite(job_start) and 0 <= time.monotonic() - job_start < JOB_TIMEOUT_SECONDS):
        parser.error("PR_REPORT_JOB_START must be a recent time.monotonic() reading")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    try:
        report = collect(output, args.head, job_start)
        report["snapshot"] = {"schema": "slither-pr-progress-v1", "sequence": 5, "state": "complete"}
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
        markdown = render(report)
        (output / "report.md").write_text(markdown, encoding="utf-8", newline="\n")
        if os.environ.get("GITHUB_STEP_SUMMARY"):
            with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as summary:
                summary.write(markdown)
    except Exception as error:
        (output / "failure.json").write_text(json.dumps({"type": type(error).__name__, "error": str(error)}) + "\n", encoding="utf-8")
        raise
    finally:
        (output / "done").touch()


if __name__ == "__main__":
    main()
