"""Direct pinned published-release/candidate comparison. Never resolves PR merge parents.

Run only in an authorized quiet window. Uses the shared build, parity, regression
and v2 sequential machinery; incompatible old sources remain Unavailable.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import time

from perf_baseline import digest, load_catalog, same_plan
import perf_build
import perf_compare
import perf_stats
import pr_report


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "slither-release-perf-v1"
FEATURES = ("--no-default-features", "--features", "networking,parallel")
RELEASE_SETTINGS = {"OPT_LEVEL": "3", "LTO": "thin", "CODEGEN_UNITS": "1", "DEBUG": "false",
                    "INCREMENTAL": "false", "OVERFLOW_CHECKS": "false", "PANIC": "unwind", "STRIP": "none"}


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def request_key(measurement):
    return hashlib.sha256(canonical(measurement).encode("ascii")).hexdigest()


def validate_request(request, root, deadline):
    measurement = request["measurement"]
    if request.get("schema") != SCHEMA or request_key(measurement) != request.get("key"):
        raise ValueError("invalid measurement request identity")
    if measurement["protocol"] != perf_stats.PLAN or measurement["config"] != {
            "runner": "ubuntu-latest", "toolchain": "stable", "features": list(FEATURES),
            "profile": "release", "release_settings": RELEASE_SETTINGS, "jobs": 2, "suite": "ci", "blocks": 1,
            "job_seconds": 1800, "stage_seconds": 1500, "reserve_seconds": 90}:
        raise ValueError("unsupported measurement configuration")
    for label in ("baseline", "candidate", "harness"):
        pin = measurement[label]
        if perf_build.resolve(root, pin["commit"], deadline=deadline) != (pin["commit"], pin["tree"]):
            raise ValueError(f"{label} commit/tree differs from pinned request")
    if perf_build.resolve(root, "HEAD", deadline=deadline)[0] != measurement["harness"]["commit"]:
        raise ValueError("collector checkout must be the pinned trusted harness")
    if perf_build.output(["git", "status", "--porcelain", "--untracked-files=no"], root, deadline=deadline):
        raise ValueError("collector checkout has tracked modifications")


def render(report):
    request = report["request"]
    m = request["measurement"]
    lines = ["<!-- slither-release-perf-direct -->", "## Direct release performance", "",
             f"Published `{m['baseline']['tag']}` `{m['baseline']['commit']}` (tree `{m['baseline']['tree']}`)",
             f"→ candidate `{m['candidate']['commit']}` (tree `{m['candidate']['tree']}`).", "",
             f"Harness `{m['harness']['commit']}` · request `{request['key']}`.", "",
             "Operation wall time; v2 conditional intervals, Ubuntu, one block per process. "
             "This is a direct comparison of these revisions, separate from PR preview percentages. "
             "It does not establish the identity of any later shipped tree.", "",
             "| Workload | Change [interval] | Result | Parity | Pairs used / complete |",
             "| --- | ---: | --- | --- | ---: |"]
    traces = {r["case"]: r["reading"] for r in report.get("traces", [])}
    for row in report.get("performance", []):
        lines.append(f"| {pr_report.cell(row['case'])} | {pr_report.change_cell(row['result'])} | "
                     f"{row['label']} | {traces.get(row['case'], 'Unavailable')} | "
                     f"{row['n_used']} / {row['n_complete']} |")
    if not report.get("performance"):
        lines.append("| — | — | Unavailable | Unavailable | 0 / 0 |")
    lines += ["", "Selected movement_turns/physics regressions, fidelity residuals, full traces, "
              "build identities and every raw attempt are retained in the run artifacts.", "",
              "Coverage assumes independent normal constant-variance log errors, constant change "
              "and order effects. Hosted calibration is not established. Changed parity compares "
              "different behavior; missing evidence is Unavailable, never zero change."]
    for error in report["errors"]:
        lines.append(f"- {pr_report.cell(error)}")
    return "\n".join(lines) + "\n"


def job_cutoff(job_start):
    return time.perf_counter() + job_start + 1800 - 90 - time.monotonic()


def measure(report, builds, output, job_start, *, cutoff=None):
    cutoff = job_cutoff(job_start) if cutoff is None else cutoff
    perf_build.remaining(30, cutoff)
    binaries = {label: b.binaries["perf_baseline"] for label, b in builds.items()}
    hashes = {label: digest(path) for label, path in binaries.items()}
    catalogs = {}
    for label, binary in binaries.items():
        perf_build.remaining(30, cutoff)
        catalogs[label] = load_catalog(binary, deadline=cutoff)
    suites = {label: c.select(suite="ci") for label, c in catalogs.items()}
    same_plan(suites)
    workloads = suites["candidate"]
    if not 0 < len(workloads) <= perf_stats.WORKLOAD_SLOTS:
        raise ValueError("direct comparison requires 1..10 shared ci workloads")
    if catalogs["baseline"].seed != catalogs["candidate"].seed:
        raise ValueError("catalog seeds differ")
    if catalogs["baseline"].networking != catalogs["candidate"].networking or any(
            (a.fingerprint, a.operation) != (b.fingerprint, b.operation)
            for a, b in zip(suites["baseline"], suites["candidate"])):
        raise ValueError("workload definitions, operations or compiled features differ")
    report["workloads"] = [w.plan() for w in workloads]
    perf_build.remaining(120, cutoff)
    report["traces"] = perf_compare.parity(binaries, suites, output, deadline=cutoff)
    # Do not make timing claims for a row whose parity could not be collected.
    available = {r["case"] for r in report["traces"] if r["reading"] != "Unavailable"}
    workloads = [w for w in workloads if w.name in available]
    stage = time.perf_counter()
    deadline = min(stage + 1500, cutoff)
    perf_build.remaining(1500, deadline)
    plan = {"plan": perf_stats.PLAN, "comparison": report["request"],
            "seed": catalogs["candidate"].seed, "executables": hashes,
            "catalogs": {label: c.sha256 for label, c in catalogs.items()},
            "metric": "operation wall time", "blocks": 1,
            "workloads": [{**w.plan(), "definitionSha256": w.fingerprint, "operation": w.operation,
                           "operations_per_block": w.iterations, "blocks": 1} for w in workloads],
            "warmup": "one discarded baseline-then-candidate pair per workload",
            "order": perf_stats.ORDER, "looks": perf_stats.LOOKS,
            "eta_denominators": perf_stats.ETA_DENOMINATORS,
            "critical": perf_stats.T_CRITICAL, "band": [0.95, 1.05],
            "deadline": {"job_seconds": 1800, "stage_seconds": 1500, "reserve_seconds": 90,
                         "stage_budget_seconds": deadline - stage}}
    timing = output / "performance"
    timing.mkdir()
    rows = perf_compare.sequential(binaries, hashes, workloads, timing, deadline, plan)
    report["performance_plan_sha256"] = digest(timing / "plan.json")
    missing = {**perf_stats.final([], 0, "measurement_failure", "trace parity unavailable"), "n_attempted": 0}
    report["performance"] = [{"case": w.name, **rows.get(w.name, missing)} for w in suites["candidate"]]


def collect(request, output, job_start, root=ROOT):
    cutoff = job_cutoff(job_start)
    report = {"schema": SCHEMA, "request": request, "state": "unavailable", "errors": [],
              "tests": {}, "traces": [], "performance": [], "fidelity": []}
    perf_compare.write_json(output / "request.json", request)
    fidelity = {}

    def regression(snapshot, build, environment):
        log = output / f"{snapshot.label}-tests.log"
        code = perf_build.command(["cargo", "test", "--locked", *FEATURES, "--no-fail-fast",
                                   *(arg for name in perf_build.TESTS for arg in ("--test", name)),
                                   "--", "--test-threads=1"], snapshot.checkout, log, environment, deadline=cutoff)
        report["tests"][snapshot.label] = pr_report.parse_tests(log.read_text(encoding="utf-8"))
        if code:
            report["errors"].append(f"{snapshot.label} selected regressions exited {code}; see logs")
        if build:
            metrics = output / f"{snapshot.label}-fidelity.json"
            if perf_build.command([build.binaries["fidelity_metrics"], snapshot.checkout / "tests/fixtures"],
                                  snapshot.checkout, metrics, environment, deadline=cutoff):
                report["errors"].append(f"{snapshot.label} fidelity metrics unavailable")
            else:
                fidelity[snapshot.label] = json.loads(metrics.read_text(encoding="utf-8"))

    try:
        perf_build.remaining(120, cutoff)
        validate_request(request, root, cutoff)
        m = request["measurement"]
        targets = perf_build.LocalTargets(root)
        with targets.hold(("baseline", "candidate")), perf_build.Session(root, output / "work", deadline=cutoff) as session:
            harness = session.worktree("harness", m["harness"]["commit"])
            snapshots = [perf_build.overlay(harness, session.worktree(label, m[label]["commit"]), deadline=cutoff)
                         for label in ("baseline", "candidate")]
            if snapshots[0].inputs != snapshots[1].inputs:
                raise ValueError("measurement input overlays differ")
            report["inputs"] = snapshots[0].inputs
            report["expected_tests"] = [f"{name}::{test}" for name in perf_build.TESTS for test in sorted(
                pr_report.test_names((harness.checkout / f"tests/{name}.rs").read_text(encoding="utf-8")))]
            builds, errors = perf_build.build_all(snapshots, targets, ("perf_baseline", "fidelity_metrics"),
                                                  logs=output, bins=output / "binaries", hook=regression,
                                                  cargo_args=FEATURES, deadline=cutoff)
            report["errors"] += errors
        report["identities"] = {label: b.record for label, b in builds.items()}
        report["fidelity"] = pr_report.metric_rows(fidelity.get("baseline"), fidelity.get("candidate"))
        for label in ("baseline", "candidate"):
            missing = set(report["expected_tests"]) - report["tests"].get(label, {}).keys()
            if missing:
                report["errors"].append(f"{label}: {len(missing)} selected test outcomes unavailable")
        if len(builds) == 2:
            for build in builds.values():
                build.verify()
            measure(report, builds, output, job_start, cutoff=cutoff)
            for build in builds.values():
                build.verify()
            report["state"] = "complete"
        else:
            report["errors"].append("Both exact sources must support the shared harness; no adaptation or retry")
    except Exception as error:
        report["errors"].append(f"{type(error).__name__}: {error}")
        perf_compare.failure(output, error, job_start, 1800)
        # An unexpected sampler/hash failure invalidates any earlier timing claim.
        report["performance"] = []
    return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(argv)
    job_start = float(os.environ["RELEASE_PERF_JOB_START"])
    if not math.isfinite(job_start) or not 0 <= time.monotonic() - job_start < 1800:
        parser.error("RELEASE_PERF_JOB_START must be a recent monotonic clock reading")
    if os.environ.get("CARGO_BUILD_JOBS") != "2" or any(
            os.environ.get(f"CARGO_PROFILE_RELEASE_{name}") != value for name, value in RELEASE_SETTINGS.items()):
        parser.error("use the workflow's pinned shared release settings and two build jobs")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    request = json.loads(args.request.read_text(encoding="utf-8"))
    if (request.get("run"), request.get("attempt")) != (
            int(os.environ["GITHUB_RUN_ID"]), int(os.environ["GITHUB_RUN_ATTEMPT"])):
        parser.error("request must be pinned for this run attempt; rerun all jobs")
    report = collect(request, output, job_start)
    perf_compare.write_json(output / "report.json", report)
    markdown = render(report)
    (output / "report.md").write_text(markdown, encoding="utf-8", newline="\n")
    # Bind all raw attempts, copied binaries, logs and partial outputs to both pins.
    perf_compare.write_json(output / "evidence.json", {"request": request, "files": {
        p.relative_to(output).as_posix(): digest(p) for p in sorted(output.rglob("*")) if p.is_file()}})
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as summary:
            summary.write(markdown)


if __name__ == "__main__":
    main()
