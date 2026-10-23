"""Compare audited evidence batches; a drift signal is not a physics verdict.

Local use: python compare_evidence.py submission.json --output fresh-report-dir
Each bundle is one acquisition batch. Reconnects, bots and packets never add n.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import statistics
import sys
import tarfile
import zlib

import capture_quality as cq
import evidence
import packet_census as pc
from evidence_statistics import holm_adjust, permutation_test

METRICS = {"minimapServerGapMs", "leaderboardServerGapMs", "preyServerGapMs",
           "preyUpdatesPerKnownSecond", "pingRttMs"}
ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}\Z")
MAX_SUBMISSION_BYTES = 128 * 1024


def _keys(value, keys):
    if not isinstance(value, dict) or set(value) != set(keys.split()):
        raise ValueError("unexpected or missing submission fields")


def _integer(value, low, high):
    return type(value) is int and low <= value <= high


def _number(value, low, high=math.inf):
    return type(value) in (int, float) and math.isfinite(value) and low <= value <= high


def validate_submission(value):
    _keys(value, "schemaVersion comparisonId independence context alpha minUnits resamples seed metrics baseline candidate")
    if value["schemaVersion"] != 1 or type(value["schemaVersion"]) is not int:
        raise ValueError("unsupported comparison schema")
    if not isinstance(value["comparisonId"], str) or not ID.fullmatch(value["comparisonId"]):
        raise ValueError("invalid comparisonId")
    for key in ("independence", "context"):
        if not isinstance(value[key], str) or not 1 <= len(value[key].strip()) <= 4096:
            raise ValueError(f"invalid {key} rationale")
    if not _number(value["alpha"], 0, .1) or value["alpha"] == 0:
        raise ValueError("alpha must be in (0, 0.1]")
    for key, low, high in (("minUnits", 4, 32), ("resamples", 1, 65535), ("seed", 0, 2**32 - 1)):
        if not _integer(value[key], low, high):
            raise ValueError(f"invalid {key}")
    metrics = value["metrics"]
    if not isinstance(metrics, list) or not 1 <= len(metrics) <= 16:
        raise ValueError("expected 1..16 predeclared metrics")
    names, ids = set(), set()
    for metric in metrics:
        _keys(metric, "id name minEffect minSamples")
        if not isinstance(metric["id"], str) or not ID.fullmatch(metric["id"]):
            raise ValueError("invalid metric id")
        if metric["name"] not in METRICS or metric["name"] in names or metric["id"] in ids:
            raise ValueError("unknown or repeated metric")
        if not _number(metric["minEffect"], 0) or not _integer(metric["minSamples"], 1, 1_000_000):
            raise ValueError("invalid metric exposure/effect gate")
        names.add(metric["name"])
        ids.add(metric["id"])
    sources, digests = set(), set()
    for cohort in ("baseline", "candidate"):
        items = value[cohort]
        if not isinstance(items, list) or not 1 <= len(items) <= 32:
            raise ValueError("expected 1..32 bundles per cohort")
        for item in items:
            _keys(item, "source sha256")
            source = item["source"]
            if not isinstance(source, str) or not 1 <= len(source) <= 4096:
                raise ValueError("invalid bundle source")
            if not cq.sha256_string(item["sha256"]):
                raise ValueError("invalid bundle digest")
            if source in sources or item["sha256"] in digests:
                raise ValueError("duplicate bundle across or within cohorts")
            sources.add(source)
            digests.add(item["sha256"])
    return value


def _metric(report, audit, name, minimum):
    if name == "pingRttMs":
        item = audit["network"]["gamePingRttMs"]
        return item["mean"] if item["count"] >= minimum else None
    values = []
    for epoch in report["connections"]:
        if epoch["wireVersion"] != 19:
            continue
        if name == "preyServerGapMs":
            gaps = epoch["prey"]["updateGapsMs"]["server"]
            if gaps["n"] >= minimum:
                values.append(gaps["median"])
        elif name == "preyUpdatesPerKnownSecond":
            known = epoch["prey"]["knownEpisodes"]
            rate = known["updatesPerKnownPreySecond"]["server"]
            if known["knownUpdates"] >= minimum and rate is not None:
                values.append(rate)
        elif name == "minimapServerGapMs":
            gaps = epoch["minimapUpdateGapsMs"]["server"]
            if gaps["n"] >= minimum:
                values.append(gaps["median"])
        else:
            for item in epoch["commands"]:
                gaps = item["gapsMs"]["server"]
                if item["command"] == "l" and gaps["n"] >= minimum:
                    values.append(gaps["median"])
    # Epochs and simultaneous clients stay nested inside the batch statistic.
    return statistics.mean(values) if values else None


def summarize_bundle(path, expected_sha, metrics):
    try:
        return _summarize_bundle(path, expected_sha, metrics)
    except (tarfile.TarError, zlib.error, EOFError, RecursionError) as error:
        raise ValueError(f"invalid evidence bundle: {type(error).__name__}") from None


def _summarize_bundle(path, expected_sha, metrics):
    before = pc.sha256(path)
    if before != expected_sha:
        raise ValueError("bundle SHA-256 mismatch")
    with evidence.verified_bundle(path) as (root, manifest):
        if manifest["audit"]["status"] != "verified":
            raise ValueError("bundle audit is not verified")
        plan = evidence.read_plan(root)
        if "capture" not in plan or plan["sampling"]["attempts"] != 1:
            raise ValueError("comparison needs one acquisition batch per bundle")
        raw = sorted(root / item["path"] for item in manifest["files"]
                     if item["path"].startswith("raw/") and item["path"].endswith(".jsonl.gz"))
        audited = cq.audit_captures(raw)
        if len(audited["batches"]) != 1 or any(c["status"] != "verified" for c in audited["captures"]):
            raise ValueError("expected one complete audited capture batch")
        batch = audited["batches"][0]
        if not all(batch[k] for k in ("completeMembership", "matchedProvenance", "sharedAcquisitionInterval")):
            raise ValueError("incomplete or unmatched acquisition batch")
        values = {metric["id"]: [] for metric in metrics}
        identities, profiles, intervals = [], [], []
        for capture in audited["captures"]:
            provenance = capture["provenance"]
            report = pc.census(Path(capture["path"]))
            for metric in metrics:
                value = _metric(report, capture, metric["name"], metric["minSamples"])
                values[metric["id"]].append(value)
            identities.append({key: capture[key] for key in ("archiveSha256", "payloadSha256")})
            identities[-1].update({key: provenance[key] for key in ("captureId", "captureBatchId")})
            profiles.append(dict(collector=provenance["collectorSha256"],
                                 # Endpoint/nickname identify acquisition, while
                                 # origin, size targets and controls can change it.
                                 options={k: v for k, v in provenance["options"].items()
                                          if k not in {"server", "nick"}},
                                 setup=capture["setupSignatures"],
                                 framing=sorted({e["framing"] for e in report["connections"] if e["wireVersion"] == 19})))
            intervals.append(dict(server=capture["header"]["server"], start=capture["wallStartMs"], end=capture["wallEndMs"]))
        capture_plan = dict(plan["capture"])
        capture_plan.pop("server")
        profile = dict(capture=capture_plan, dataOrigin=plan["dataOrigin"],
                       kind=plan["kind"], acquisition=profiles[0])
        if any(p != profiles[0] for p in profiles):
            raise ValueError("mixed collector/setup/framing within batch")
        result = dict(bundleSha256=before, studyId=manifest["studyId"], planSha256=manifest["planSha256"],
                      profile=profile, identities=identities, intervals=intervals,
                      metrics={key: statistics.mean(items) if all(v is not None for v in items) else None
                               for key, items in values.items()})
    if pc.sha256(path) != before:
        raise ValueError("bundle changed during comparison")
    return result


def compare_submission(submission, resolved, *, submission_sha256):
    validate_submission(submission)
    if not cq.sha256_string(submission_sha256):
        raise ValueError("invalid submission digest")
    groups = {cohort: [summarize_bundle(Path(resolved[item["source"]]), item["sha256"], submission["metrics"])
                       for item in submission[cohort]] for cohort in ("baseline", "candidate")}
    units = groups["baseline"] + groups["candidate"]
    seen = {key: set() for key in ("archiveSha256", "payloadSha256", "captureId", "captureBatchId")}
    reasons = []
    for unit in units:
        for key in seen:
            identities = {row[key] for row in unit["identities"]}
            if seen[key] & identities:
                reasons.append("reused_" + key)
            seen[key].update(identities)
    if any(unit["profile"] != units[0]["profile"] for unit in units):
        reasons.append("collector_setup_scenario_origin_or_framing_changed")
    for i, unit in enumerate(units):
        for other in units[:i]:
            if any(a["server"] == b["server"] and a["start"] < b["end"] and b["start"] < a["end"]
                   for a in unit["intervals"] for b in other["intervals"]):
                reasons.append("overlapping_same_server_acquisitions")
    results = []
    for metric in submission["metrics"]:
        samples = {cohort: [u["metrics"][metric["id"]] for u in groups[cohort]] for cohort in groups}
        row = dict(metric, samples=samples, status="inconclusive")
        # Never drop missing batches, failed exposures or unfavorable strata to
        # rescue a test. The entire predeclared metric remains inconclusive.
        if reasons:
            row["reason"] = "incomparable acquisition cohorts"
        elif any(len(items) < submission["minUnits"] or any(x is None for x in items) for items in samples.values()):
            row["reason"] = "insufficient independent batches or metric exposure"
        else:
            row["test"] = permutation_test(samples["baseline"], samples["candidate"],
                                           resamples=submission["resamples"], seed=submission["seed"])
        results.append(row)
    # Missing endpoints remain in the predeclared Holm family at p=1.
    adjusted = holm_adjust([row.get("test", {}).get("pValue", 1.) for row in results])
    for row, p in zip(results, adjusted):
        if "test" in row:
            row["adjustedPValue"] = p
            row["status"] = ("drift_detected" if p <= submission["alpha"] and
                             abs(row["test"]["difference"]) >= row["minEffect"] else "no_drift_detected")
    status = "incomparable" if reasons else "inconclusive"
    if not reasons and any(row["status"] == "drift_detected" for row in results):
        status = "drift_detected"
    elif not reasons and all(row["status"] == "no_drift_detected" for row in results):
        status = "no_drift_detected"
    return dict(schemaVersion=1, comparisonId=submission["comparisonId"], submissionSha256=submission_sha256,
                status=status, scientificStatus="unreviewed", reasons=sorted(set(reasons)),
                declaration=submission,
                independence=submission["independence"], context=submission["context"],
                alpha=submission["alpha"], minUnits=submission["minUnits"], seed=submission["seed"],
                resamples=submission["resamples"], cohorts=groups, metrics=results,
                tools={name: pc.sha256(Path(module.__file__)) for name, module in
                       (("comparison", sys.modules[__name__]), ("census", pc), ("decoder", pc.sp),
                        ("audit", cq), ("bundle", evidence), ("statistics", sys.modules[permutation_test.__module__]))},
                limits=["One complete acquisition batch is one declared unit; provenance cannot prove independence or exchangeability.",
                        "Matched collection metadata cannot remove unobserved size, movement, population or network confounding.",
                        "Tests detect shifts in batch means of selected summaries, not every distribution or physics change.",
                        "No drift detected does not establish equivalence. Never auto-replace historical fixtures or defaults.",
                        "Per-client means of eligible epoch summaries are averaged within each batch, then batches get equal weight.",
                        "Reporting cadence and RTT are recipient observables; a drift flag requires scientific review."])


def write_report(output, report):
    serialized = json.dumps(report, indent=2, allow_nan=False) + "\n"
    lines = ["# Evidence comparison", "", "Status: `" + report["status"] + "`. Scientific status: unreviewed.", "",
             "A signal is not a physics verdict; absence of a signal is not equivalence.", "",
             "| Metric | Result | Adjusted p | Candidate minus baseline |", "| --- | --- | --- | --- |"]
    for row in report["metrics"]:
        test = row.get("test", {})
        lines.append(f'| {row["id"]} | {row["status"]} | {row.get("adjustedPValue", "—")} | {test.get("difference", "—")} |')
    lines.extend(["", "See report.json for source/tool hashes, every batch, exposure and limitations.", ""])
    output = Path(output)
    output.mkdir(parents=True, exist_ok=False)
    (output / "report.json").write_text(serialized, encoding="utf-8", newline="\n")
    (output / "REPORT.md").write_text("\n".join(lines), encoding="utf-8", newline="\n")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("submission", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        if os.path.lexists(args.output) or args.submission.stat().st_size > MAX_SUBMISSION_BYTES:
            raise ValueError("existing output or oversized submission")
        with args.submission.open("rb") as stream:
            raw = stream.read(MAX_SUBMISSION_BYTES + 1)
        if len(raw) > MAX_SUBMISSION_BYTES:
            raise ValueError("oversized submission")
        # Parse and hash one snapshot, sharing the bundle reader's strict JSON
        # key/number rules instead of reopening a mutable declaration.
        value = validate_submission(json.loads(raw.decode("utf-8"), object_pairs_hook=evidence._pairs,
                                               parse_constant=evidence._constant))
        resolved = {}
        for item in value["baseline"] + value["candidate"]:
            name = evidence.portable_path(item["source"])
            resolved[item["source"]] = args.submission.parent / name
        report = compare_submission(value, resolved, submission_sha256=hashlib.sha256(raw).hexdigest())
        write_report(args.output, report)
    except (ValueError, TypeError, KeyError, IndexError, OSError, EOFError, OverflowError, RecursionError) as error:
        print("evidence comparison failed: " + json.dumps(str(error)[:1000]), file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
