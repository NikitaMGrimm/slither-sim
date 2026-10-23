"""Collect an advisory, fixed-suite PR comparison; standard library only.

The worktrees, binaries, logs and failed attempts are retained in fresh owned
directories. Performance interpretation requires a quiet runner and matched inputs.
"""

import argparse
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import sys

from perf_baseline import CASES, capture_trace, digest


ROOT = Path(__file__).resolve().parents[1]
TESTS = ("movement_turns", "physics")
MARKER = "<!-- slither-report-preview -->"


def command(args, cwd, log, environment=None, timeout=600):
    with log.open("w", encoding="utf-8", newline="\n") as output:
        try:
            result = subprocess.run(args, cwd=cwd, env=environment, stdout=output,
                                    stderr=subprocess.STDOUT, text=True, timeout=timeout, check=False)
            return result.returncode
        except (OSError, subprocess.TimeoutExpired) as error:
            output.write(f"\n{type(error).__name__}: {error}\n")
            return -1


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


def performance_rows(directory):
    summary = json.loads((directory / "summary.json").read_text(encoding="utf-8"))
    raw = [json.loads(line) for line in (directory / "results.jsonl").read_text(encoding="utf-8").splitlines()]
    result = []
    for case, data in summary.items():
        ratios = [p["cpu_ratio_candidate_over_baseline"] for p in data["pairs"]]
        use_cpu = all(r is not None and math.isfinite(r) and r > 0 for r in ratios)
        if not use_cpu:
            ratios = [p["wall_ratio_candidate_over_baseline"] for p in data["pairs"]]
        if not ratios or not all(math.isfinite(r) and r > 0 for r in ratios):
            raise ValueError(f"invalid performance ratios for {case}")
        ratio = statistics.median(ratios)
        # A display heuristic for exploratory hosted timings, not a significance test.
        reading = ("Faster" if ratio < .95 and max(ratios) < 1
                   else "Slower" if ratio > 1.05 and min(ratios) > 1 else "Inconclusive")
        representative = sorted(zip(ratios, data["pairs"]), key=lambda pair: pair[0])[len(ratios) // 2][1]
        values = {}
        for label in ("baseline", "candidate"):
            sample = next(r for r in raw if r["case"] == case and r["label"] == label
                          and r["repetition"] == representative["repetition"])
            values[label] = (sample["cpu_seconds"] if use_cpu else sample["workload"]["timing"]["wallSeconds"]) * 1000 / data["iterations"]
        result.append({"case": case, "metric": "process CPU" if use_cpu else "operation wall",
                       **values, "ratio": ratio, "ratio_min": min(ratios), "ratio_max": max(ratios),
                       "pairs": len(ratios), "reading": reading})
    return result


def cell(value):
    return str(value).replace("|", "\\|").replace("\n", " ").replace("\r", " ").replace("`", "'")


def status(reading):
    icon = {"Faster": "🟢", "Lower error": "🟢", "Equal": "🟢",
            "Slower": "🔴", "Higher error": "🔴", "Inconclusive": "🟡",
            "Changed": "🟡", "Unavailable": "⚠️"}.get(reading, "⚪")
    return f"{icon} {reading}"


def render(report):
    performance = report.get("performance", [])
    metrics = {row["metric"] for row in performance}
    units = f"{next(iter(metrics)).capitalize()}, ms/op." if len(metrics) == 1 else "Time per operation, ms."
    lines = [MARKER, "## Comparison", "",
             f"Base `{report['base'][:8]}` → merge `{report['merge'][:8]}` · [Run and data]({report['run_url']})", "",
             "### Performance", "", units, "",
             "| Workload | Base | PR | Change | Result |",
             "| --- | ---: | ---: | ---: | --- |"]
    for row in performance:
        label = row["case"] + (f" ({row['metric']})" if len(metrics) > 1 else "")
        lines.append(f"| {cell(label)} | {row['baseline']:.5f} | {row['candidate']:.5f} | "
                     f"{(row['ratio'] - 1) * 100:+.1f}% | {status(row['reading'])} |")
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
              "- Three alternating pairs after warmup. Base/PR costs come from the median-ratio pair.",
              "- Faster/slower requires a ≥5% median shift with the same direction in every pair. This is a display heuristic, not statistical significance.",
              "- Process CPU includes setup/teardown; operation wall time excludes them.",
              "- Both runtimes use the same selected test sources and fixtures. Errors describe those fixtures, not overall accuracy.",
              "- Turn errors are duration weighted; speed ramps are grouped by source. Each state trace covers 32 iterations. Changed traces indicate changed behavior."]
    for row in performance:
        lines.append(f"- `{row['case']}` paired change range: {(row['ratio_min'] - 1) * 100:+.1f}% to {(row['ratio_max'] - 1) * 100:+.1f}%.")
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
    return "\n".join(lines)


def collect(output, head):
    merge = git("rev-parse", "HEAD")
    base = git("rev-parse", "HEAD^1")
    if git("rev-parse", "HEAD^2") != head:
        raise ValueError("checkout must be the PR merge commit for the expected head")
    report = {"schema": 1, "base": base, "head": head, "merge": merge,
              "repository": os.environ["GITHUB_REPOSITORY"], "pr": int(os.environ["PR_NUMBER"]),
              "run": int(os.environ["GITHUB_RUN_ID"]), "attempt": int(os.environ["GITHUB_RUN_ATTEMPT"]),
              "run_url": f"{os.environ['GITHUB_SERVER_URL']}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}",
              "platform": platform.platform(), "errors": [], "tests": {}, "traces": []}
    work = output.with_name(output.name + "-work")
    work.mkdir()
    baseline = work / "base"
    subprocess.run(["git", "worktree", "add", "--detach", str(baseline), base], cwd=ROOT, check=True, timeout=30)
    inputs = [ROOT / "examples/perf_baseline.rs", ROOT / "examples/fidelity_metrics.rs",
              *(ROOT / "tests" / f"{name}.rs" for name in TESTS), *sorted((ROOT / "tests/fixtures").rglob("*"))]
    inputs = [p for p in inputs if p.is_file()]
    report["inputs"] = {str(p.relative_to(ROOT)).replace("\\", "/"): digest(p) for p in inputs}
    report["suite_changes"] = [name for name, sha in report["inputs"].items()
                               if not (baseline / name).is_file() or digest(baseline / name) != sha]
    report["added_tests"], report["removed_tests"], report["expected_tests"] = [], [], []
    for name in TESTS:
        path = Path("tests") / f"{name}.rs"
        old, new = test_names((baseline / path).read_text()), test_names((ROOT / path).read_text())
        report["expected_tests"] += [f"{name}::{n}" for n in sorted(new)]
        report["added_tests"] += [f"{name}::{n}" for n in sorted(new - old)]
        report["removed_tests"] += [f"{name}::{n}" for n in sorted(old - new)]
    for path in inputs:
        destination = baseline / path.relative_to(ROOT)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
    report["rustc"] = subprocess.check_output(["rustc", "-Vv"], text=True, timeout=30)
    fidelity = {}
    binaries = {}
    for label, checkout in (("baseline", baseline), ("candidate", ROOT)):
        env = {**os.environ, "CARGO_TARGET_DIR": str(work / f"target-{label}"), "CARGO_BUILD_JOBS": "2",
               "CARGO_TERM_COLOR": "never", "RUST_TEST_THREADS": "1"}
        report.setdefault("cargo_locks", {})[label] = digest(checkout / "Cargo.lock")
        print(f"[{label}] Building release measurement examples", flush=True)
        build_log = output / f"{label}-build.log"
        built = command(["cargo", "build", "--release", "--locked", "--example", "perf_baseline", "--example", "fidelity_metrics"], checkout, build_log, env)
        test_log = output / f"{label}-tests.log"
        print(f"[{label}] Running the selected regression suite", flush=True)
        code = command(["cargo", "test", "--locked", "--no-fail-fast", "--test", "movement_turns", "--test", "physics", "--", "--test-threads=1"], checkout, test_log, env)
        report["tests"][label] = parse_tests(test_log.read_text(encoding="utf-8"))
        if code:
            report["errors"].append(f"{label} selected tests exited {code}")
        if built:
            report["errors"].append(f"{label} release build exited {built}")
            continue
        suffix = ".exe" if os.name == "nt" else ""
        bins = work / f"bin-{label}"
        bins.mkdir()
        for name in ("perf_baseline", "fidelity_metrics"):
            shutil.copy2(work / f"target-{label}/release/examples/{name}{suffix}", bins / f"{name}{suffix}")
        binaries[label] = bins / f"perf_baseline{suffix}"
        report.setdefault("binaries", {})[label] = {p.name: digest(p) for p in bins.iterdir()}
        metrics_log = output / f"{label}-fidelity.json"
        if command([str(bins / f"fidelity_metrics{suffix}"), str(ROOT / "tests/fixtures")], checkout, metrics_log):
            report["errors"].append(f"{label} fidelity metrics unavailable")
        else:
            fidelity[label] = json.loads(metrics_log.read_text(encoding="utf-8"))
    report["fidelity"] = metric_rows(fidelity.get("baseline"), fidelity.get("candidate"))
    for case in CASES:
        print(f"[parity] Capturing {case} traces", flush=True)
        hashes = {}
        for label, binary in binaries.items():
            directory = output / f"traces-{label}"
            directory.mkdir(exist_ok=True)
            try:
                hashes[label] = capture_trace(binary, case, 32, directory)
            except Exception as error:
                report["errors"].append(f"{label} {case} trace: {error}")
        report["traces"].append({"case": case, **hashes, "reading": "Unavailable" if len(hashes) != 2
                                 else "Equal" if hashes["baseline"]["sha256"] == hashes["candidate"]["sha256"] else "Changed"})
    if len(binaries) == 2:
        print("[performance] Running three alternating pairs for each workload", flush=True)
        perf = output / "performance"
        code = command([sys.executable, str(ROOT / "tools/perf_compare.py"), "--baseline", str(binaries["baseline"]),
                        "--candidate", str(binaries["candidate"]), "--output", str(perf), "--repeats", "3"],
                       ROOT, output / "performance.log")
        if code:
            report["errors"].append(f"paired performance runner exited {code}")
        else:
            report["performance"] = performance_rows(perf)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--head", required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.head):
        parser.error("head must be a full Git SHA")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    try:
        report = collect(output, args.head)
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
        markdown = render(report)
        (output / "report.md").write_text(markdown, encoding="utf-8", newline="\n")
        if os.environ.get("GITHUB_STEP_SUMMARY"):
            with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as summary:
                summary.write(markdown)
    except Exception as error:
        (output / "failure.json").write_text(json.dumps({"type": type(error).__name__, "error": str(error)}) + "\n", encoding="utf-8")
        raise


if __name__ == "__main__":
    main()
