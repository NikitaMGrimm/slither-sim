"""Validate complete retained runs and write bounded, portable selected results.

python3 select_results.py RAW_OUTPUT NEW_RESULTS_JSON
Fails on missing repeats, PMU scheduling failure, changed binaries, nonidentical
continuations or malformed operation windows. Keeps unsettled/depleted rows.
"""
import argparse
import filecmp
import hashlib
import json
import math
from pathlib import Path
import re
import statistics

CASES = ["500-common", "1000-common", "2000-common", "5000-capped", "5000-uncapped", "10000-capped"]
PHASES = ["movementAndBoost", "collisionBuild", "collisionQueryAndTests", "deathFood", "pickupAndFoodCache", "prey", "ambientRefillAndAdmission"]


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def load(path):
    return json.loads(path.read_text())


def publication_view(value, vm_root):
    """Label private locations after validation, preserving relative paths and hashes."""
    if isinstance(value, dict):
        result = {key: publication_view(item, vm_root) for key, item in value.items()}
        if "kernel" in result:
            match = re.match(r"\d+\.\d+(?:\.\d+)?", result["kernel"])
            require(match is not None, "unrecognized kernel version")
            result["kernel"] = match.group()
        return result
    if isinstance(value, list):
        return [publication_view(item, vm_root) for item in value]
    if isinstance(value, str):
        value = value.replace(str(vm_root), "<vm-root>")
        value = re.sub(r"/home/[^/\s]+/artifacts(?=/|$)", "<wsl-artifacts>", value)
        value = re.sub(r"/mnt/[a-z]/Users/[^/\s]+/(?:Source/slither-sim|\.t3/worktrees/slither-sim/[^/\s]+)(?=/|$)", "<repo>", value)
        return value.replace("Owner capped the uncapped 10k control", "The maintaining agent capped the uncapped 10k control").replace(
            "owner CPU-time budget", "maintaining agent CPU-time budget")
    return value


def validate_probe_source(path, expected):
    current = path.read_bytes()
    if hashlib.sha256(current).hexdigest() == expected:
        return
    # The retained probe predates the attribution correction. Reconstruct only
    # that exact text edit to check its original pin without rewriting evidence.
    corrected = b"maintaining agent CPU-time budget"
    require(path.name == "bounded_probe.rs" and current.count(corrected) == 1 and
            hashlib.sha256(current.replace(corrected, b"owner CPU-time budget")).hexdigest() == expected,
            "probe harness drift")


def log_json(path):
    return json.loads(path.read_text().splitlines()[-1])


def distribution(values):
    require(bool(values), "empty distribution")
    return {"median": statistics.median(values), "min": min(values), "max": max(values)}


def validate_rows(rows):
    require(len(rows) == 1000, "incomplete physics window")
    require(all(b["tick"] == a["tick"] + 1 for a, b in zip(rows, rows[1:])), "noncontiguous ticks")
    for row in rows:
        for metric in ["step", "spawn"]:
            count, enabled, running = row[metric]
            require(count > 0 and enabled > 0 and .99 <= running / enabled <= 1.000001, "invalid PMU interval")


def validate_reference_census(reference, measured):
    keys = ["tick", "attempts", "refusals", "births", "deaths", "alive"]
    require(len(reference) == len(measured) and all(all(a[key] == b[key] for key in keys)
            for a,b in zip(reference, measured)), "prepared admission changed the per-tick population/lifecycle census")


def admission_census(oracle):
    """Official pose probes draw exactly distance/angle/heading per candidate.

    Recover their iteration count from the retained xorshift endpoint, without
    adding hooks or running another world. This is the 50-pose oracle sample,
    not a census of candidates in the measured lifecycle window.
    """
    require(oracle["poses"] == 50, "unexpected admission sample size")
    state = 0x123456789abcdef1
    mask = (1 << 64) - 1
    for draws in range(1, 50 * 64 * 3 + 1):
        state = (state ^ (state << 13)) & mask
        state ^= state >> 7
        state = (state ^ (state << 17)) & mask
        if state == oracle["rng"]:
            require(draws >= 150 and draws % 3 == 0, "invalid admission draw count")
            return {"rngDraws": draws, "candidateIterations": draws // 3,
                    "meanCandidatesPerPose": draws / 150}
    raise RuntimeError("admission RNG endpoint is outside the frozen sample")


def death_record_count(path, chunk_size=1024 * 1024):
    # Canonical checkpoint keys cannot occur unescaped inside JSON strings.
    # Streaming this unique DeathRecord key avoids a multi-GB Python JSON tree.
    token = b'"victim":'
    count, tail = 0, b""
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(chunk_size), b""):
            joined = tail + block
            count += joined.count(token)
            tail = joined[-(len(token)-1):]
    return count


def continuation_census(warm, endpoint, end_stats):
    before, after = load(warm / "driver.json"), load(endpoint / "driver.json")
    if before["retry"] != after["retry"]:
        return {"status": "unavailable", "reason": "refusal retries changed during the cache prefix"}
    start_stats = load(warm / "summary.json")["end"]
    deaths = death_record_count(endpoint / "world.json") - death_record_count(warm / "world.json")
    births = deaths + end_stats["alive"] - start_stats["alive"]
    state, target = before["random"], after["random"]
    require(births > 0 and state != target, "empty continuation admission census")
    matches, mask = [], (1 << 64) - 1
    for draws in range(1, births * 64 * 3 + 1):
        state = (state ^ (state << 13)) & mask
        state ^= state >> 7
        state = (state ^ (state << 17)) & mask
        if state == target:
            matches.append(draws)
    require(len(matches) == 1 and matches[0] % 3 == 0, "continuation RNG endpoint mismatch")
    return {"status": "complete", "ticks": after["tick"]-before["tick"],
            "includesCachePrefix": True, "admissions": births, "deaths": deaths,
            "candidateIterations": matches[0]//3, "meanCandidatesPerAdmission": matches[0]/(3*births)}


def select(root):
    pins = load(root / "pins.json")
    study = Path(__file__).resolve().parent
    for name in ["physics.rs", "counter.rs", "bots.rs", "warm_bounds.rs"]:
        require(sha(study / name) == pins["harness"][name], "compiled harness drift")
    if pins.get("checkpointPreparation", {}).get("shapeSourceSha256"):
        require(sha(study / "checkpoint_shape.rs") == pins["checkpointPreparation"]["shapeSourceSha256"],
                "checkpoint preparation drift")
        checks = load(root / "checkpoint-checks.json")
        require(all(checks.get(arm) == {"validRoundTrip": True, "originalImporterCompared": True, "rejectedShapes": 8}
                    for arm in ["plain", "profile"]), "preparation fixture checks incomplete")
    for arm, info in pins["arms"].items():
        for binary, digest in info["binaries"].items():
            require(sha(root / arm / binary) == digest, "binary identity drift")
    result = {"format": "slither-population-scaling-v1", "pins": pins, "rawAccess": "local-only",
              "rawRoot": str(root), "environment": load(root / "environment.json"),
              "warmupOracles": load(root / "warmup-oracles.json"),
              "analysisSources": {p.name: sha(p) for p in study.glob("*.py")}, "cases": {}, "bots": {}}
    result["resourceAmendment"] = {
        name: load(root / name) for name in ["environment-one-cpu.json", "sequential-resource.json"]
        if (root / name).is_file()}
    if (root / "reused-inputs.json").is_file():
        reused = load(root / "reused-inputs.json")
        for case, files in reused["warmups"].items():
            for name, digest in files.items():
                require(sha(root / case / name) == digest, "reused warmup drift")
        result["reusedInputs"] = reused
    if (root / "checkpoint-checks.json").is_file():
        result["checkpointPreparationChecks"] = load(root / "checkpoint-checks.json")
    radius_check = load(root / "radius-plan-check.json")
    require(radius_check["validThroughTick"] == 241100 and radius_check["radius"] == 32000
            and radius_check["planSha256"] == sha(root / "capped-radius-plan.json"), "cap event validation incomplete")
    result["radiusPlanCheck"] = radius_check
    references = load(root / "original-references.json")
    require(len(references["origins"]) == 1, "original admission references need one binary cohort")
    for digest, origin in references["origins"].items():
        require(sha(root / origin["binary"]) == digest, "original admission reference binary drift")
        require(origin["pins"]["runtime"] == pins["runtime"] and
                origin["pins"]["archiveSha256"] == pins["archiveSha256"], "original reference runtime drift")
    result["originalAdmissionOrigins"] = references["origins"]
    for case in CASES:
        n = int(case.split("-")[0])
        warm = root / case
        summary = load(warm / "summary.json")
        blocks = load(warm / "warmup.json")
        require(3 <= len(blocks) <= 24 and summary["ticks"] == blocks[-1]["ticks"], "invalid warmup extent")
        require(summary["end"] == blocks[-1]["end"], "warmup endpoint mismatch")
        ratios = {key: max(b[key] for b in blocks[-3:]) / min(b[key] for b in blocks[-3:])
                  for key in ["meanSegments", "p90Mean"]}
        require(summary["settled"] == all(ratio <= 1.05 for ratio in ratios.values()), "warmup screen mismatch")
        require(summary["settled"] or len(blocks) == 24, "premature unsettled primary warmup")
        ratios["foodCountAtBlockEnd"] = max(b["end"]["food"] for b in blocks[-3:]) / min(b["end"]["food"] for b in blocks[-3:])
        oracle = load(root / f"{case}-oracle/result.json")
        require(oracle["poses"] == 50 and oracle["exact"], "admission oracle incomplete")
        oracle["candidateCensus"] = admission_census(oracle)
        runs = []
        for repeat in range(-1, 5):
            folder = root / f"{case}-count-{repeat}"
            rows = load(folder / "rows.json")
            validate_rows(rows)
            for name in ["world.json", "driver.json"]:
                require(filecmp.cmp(folder / name, root / f"{case}-count-0" / name, shallow=False), "counted continuation changed")
            runs.append({"repeat": repeat, "stepInstructionsPerTick": statistics.mean(r["step"][0] for r in rows),
                         "preparationSpawnInstructionsPerTick": statistics.mean(r["spawn"][0] for r in rows),
                         "minimumPmuScheduled": min(r[key][2] / r[key][1] for r in rows for key in ["step", "spawn"]),
                         "deaths": sum(r["deaths"] for r in rows), "births": sum(r["births"] for r in rows),
                         "attempts": sum(r["attempts"] for r in rows), "refusals": sum(r["refusals"] for r in rows),
                         "aliveMean": statistics.mean(r["alive"] for r in rows), "aliveMin": min(r["alive"] for r in rows),
                         "simulatedMean": statistics.mean(r["alive"] + r["deaths"] for r in rows),
                         "simulatedMin": min(r["alive"] + r["deaths"] for r in rows),
                         "stats": log_json(root / f"{case}-count-{repeat}.log"),
                         "sha256": {name: sha(folder / name) for name in ["rows.json", "world.json", "driver.json"]}})
        profile_folder = root / f"{case}-profile"
        for name in ["world.json", "driver.json"]:
            require(filecmp.cmp(profile_folder / name, root / f"{case}-count-0" / name, shallow=False), "profile continuation changed")
        profile_rows = load(profile_folder / "rows.json")
        validate_rows(profile_rows)
        profile = log_json(root / f"{case}-profile.log")["profile"]
        require(profile["calls"][:7] == [1000] * 7, "coarse diagnostic brackets did not execute")
        require(profile["calls"][7] == profile["calls"][8] and profile["calls"][7] > 0,
                "collision sampling brackets did not execute")
        phases = {name: profile["instructions"][i] / 1000 for i, name in enumerate(PHASES)}
        outer_profile = statistics.mean(r["step"][0] for r in profile_rows)
        phases["rest"] = outer_profile - sum(phases.values())
        require(phases["rest"] >= 0, "coarse phase budget exceeds outer step")
        # These estimates are nested in the coarse rows, not additive to them.
        phases["sampledCollisionQueryEstimate"] = profile["instructions"][7] * 64 / 1000
        phases["sampledCandidateTestsEstimate"] = profile["instructions"][8] * 64 / 1000
        phases["sampledBoostEmissionEstimate"] = profile["instructions"][19] * 64 / 1000
        selected = runs[1:]
        reference_runs = []
        counted_rows = load(root / f"{case}-count-0/rows.json")
        for reference in references["cases"][case]:
            folder = root / reference["folder"]
            require(reference["origin"] in references["origins"], "unknown admission reference origin")
            reference_rows = load(folder / "rows.json")
            validate_rows(reference_rows)
            validate_reference_census(reference_rows, counted_rows)
            for name in ["world.json", "driver.json"]:
                require(filecmp.cmp(folder / name, root / f"{case}-count-0" / name, shallow=False),
                        "prepared admission changed the original-search full continuation")
            reference_runs.append({**reference,
                "stepInstructionsPerTick": statistics.mean(r["step"][0] for r in reference_rows),
                "spawnInstructionsPerTick": statistics.mean(r["spawn"][0] for r in reference_rows),
                "totalInstructionsPerTick": statistics.mean(r["step"][0]+r["spawn"][0] for r in reference_rows),
                "minimumPmuScheduled": min(r[key][2]/r[key][1] for r in reference_rows for key in ["step","spawn"]),
                "sha256": {name: sha(folder/name) for name in ["world.json","driver.json","rows.json"]}})
        require(bool(reference_runs), "missing original admission reference")
        radius = summary["end"]["radius"]
        result["cases"][case] = {"requested": n, "warmup": summary, "warmupBlocks": blocks,
                                "lastThreeWarmupBlockRatios": ratios, "admissionOracle": oracle,
                                "continuationCandidateCensus": continuation_census(warm, root / f"{case}-count-0", selected[0]["stats"]["end"]),
                                "densityRelativeToHeuristic": (670 * math.sqrt(n) / radius) ** 2,
                                "stepInstructionsPerTick": distribution([r["stepInstructionsPerTick"] for r in selected]),
                                "spawnInstructionsPerTick": distribution([r["spawnInstructionsPerTick"] for r in reference_runs]),
                                "totalInstructionsPerTick": distribution([r["totalInstructionsPerTick"] for r in reference_runs]),
                                "referenceStepInstructionsPerTick": distribution([r["stepInstructionsPerTick"] for r in reference_runs]),
                                "preparedStepRelativeToOriginal": statistics.median(r["stepInstructionsPerTick"] for r in selected) /
                                    statistics.median(r["stepInstructionsPerTick"] for r in reference_runs),
                                "originalAdmissionReferenceCount": len(reference_runs), "originalAdmissionReferences": reference_runs,
                                "aliveMean": selected[0]["aliveMean"], "aliveMin": selected[0]["aliveMin"],
                                "simulatedMean": selected[0]["simulatedMean"], "simulatedMin": selected[0]["simulatedMin"],
                                "stepInstructionsPerSimulatedSnake": statistics.median(r["stepInstructionsPerTick"] / r["simulatedMean"] for r in selected),
                                "stepInstructionsPerLivingSnake": statistics.median(r["stepInstructionsPerTick"] / r["aliveMean"] for r in selected),
                                "deathsPerWorldSecond": selected[0]["deaths"] / 7,
                                "profileInstructionsPerTick": phases, "profileOuterInstructionsPerTick": outer_profile,
                                "profilePreparationSpawnInstructionsPerTick": statistics.mean(r["spawn"][0] for r in profile_rows),
                                "profileStepRelativeToPlain": outer_profile / statistics.median(r["stepInstructionsPerTick"] for r in selected),
                                "runsIncludingWarmup": runs, "profileRawSha256": sha(profile_folder / "rows.json")}
    result["cases"]["10000-uncapped"] = select_unsettled(root)
    public_oracle = root / "public-import-oracle"
    require(not pins.get("checkpointPreparation") or public_oracle.is_dir(), "missing original importer continuation oracle")
    if public_oracle.exists():
        for name in ["world.json", "driver.json"]:
            require(filecmp.cmp(public_oracle / name, root / "500-common-count-0" / name, shallow=False),
                    "bounded-memory preparation changed the original 1000-tick continuation")
        require(load(public_oracle / "source.json")["runtime"] == pins["runtime"], "importer oracle runtime drift")
        result["checkpointPreparationOracle"] = {"equal": True, "windowTicks": 1000,
            "source": load(public_oracle / "source.json"),
            "sha256": {name: sha(public_oracle / name) for name in ["world.json", "driver.json"]}}
    result["bots"] = select_bots(root)
    return result


def validate_censor(status):
    require(status["status"] == "unsettled", "censored control must remain unsettled")
    charged = status["priorChargedSeconds"] + status["isolatedDiagnosticCpuSeconds"]
    require(0 < charged <= status["cpuBudgetSeconds"] <= 3600, "control CPU budget exceeded")


def select_unsettled(root):
    status = load(root / "uncapped-status.json")
    validate_censor(status)
    probe = root / status["diagnostic"]
    pins = load(probe / "pins.json")
    require(pins["runtime"] == load(root / "pins.json")["runtime"], "probe runtime drift")
    for name, key in [("study_cliff", "binarySha256"), ("study_bounded", "boundedBinarySha256")]:
        require(sha(probe / name) == pins[key], "probe binary identity drift")
    study = Path(__file__).resolve().parent
    for name, key in [("cliff_probe.rs", "probeSourceSha256"), ("bounded_probe.rs", "boundedSourceSha256")]:
        validate_probe_source(study / name, pins[key])
    cpu = load(probe / "collection-cpu.json")
    require(abs(cpu["userSeconds"] + cpu["systemSeconds"] - status["isolatedDiagnosticCpuSeconds"]) < 1e-6,
            "probe CPU accounting mismatch")
    rows = [json.loads(line) for line in (probe / "bounded/ticks.jsonl").read_text().splitlines()]
    require(bool(rows) and [r["tick"] for r in rows] == list(range(1, len(rows)+1)), "incomplete reached-tick sequence")
    for row in rows:
        for key in ["step", "spawn"]:
            count, enabled, running = row[key]
            require(count > 0 and enabled > 0 and .99 <= running/enabled <= 1.000001, "invalid probe PMU interval")
        phases = row["phases"]
        require(phases["calls"][:7] == [1]*7, "probe phase brackets did not execute")
        require(sum(phases["instructions"][:7]) <= row["step"][0], "probe phase budget exceeds outer step")
        require(row["denseAccepted"] == (phases["instructions"][20] == 1), "probe dense census mismatch")
    cold = {}
    for path in sorted(probe.glob("[0-9]*.json")):
        value = load(path)
        require(value["kind"] == "cold-grid-cliff-diagnostic", "invalid cold diagnostic")
        require(len(value["steps"]) == value["ticks"] and value["ticks"] > 0, "incomplete cold window")
        for count, enabled, running in value["steps"]:
            require(count > 0 and enabled > 0 and .99 <= running/enabled <= 1.000001, "invalid cold PMU interval")
        require(value["profile"]["calls"][:7] == [value["ticks"]]*7, "cold phases did not execute")
        cold[path.stem] = {"data": value, "sha256": sha(path)}
    summary = load(probe / "bounded/summary.json") if (probe / "bounded/summary.json").exists() else None
    if summary is not None:
        require(summary["status"] == "unsettled" and summary["ticks"] == len(rows), "bounded endpoint mismatch")
        require(summary["end"] == rows[-1]["state"], "bounded state mismatch")
        require(abs(summary["cpuSeconds"] - rows[-1]["processCpuSeconds"]) < .1, "bounded clock mismatch")
    return {"requested": 10000, "status": "unsettled", "densityRelativeToHeuristic": 1,
            "censor": status, "probePins": pins, "probeCpu": cpu, "reachedTicks": rows,
            "publishedProbeSources": {name: sha(study / name) for name in ["cliff_probe.rs", "bounded_probe.rs"]},
            "boundedSummary": summary,
            "coldDiagnostics": cold, "reachedTicksSha256": sha(probe / "bounded/ticks.jsonl"),
            "note": "Cold and censored costs are not a steady-state estimate; prior interrupted warmup reached-tick count is unknown."}


def select_bots(root):
    pins = load(root / "bot-pins.json") if (root / "bot-pins.json").exists() else load(root / "pins.json")
    binaries = root / "bot-binaries" if (root / "bot-pins.json").exists() else root
    require(sha(Path(__file__).with_name("bots.rs")) == pins["harness"]["bots.rs"], "bot harness drift")
    require(sha(Path(__file__).with_name("counter.rs")) == pins["harness"]["counter.rs"], "bot counter drift")
    for arm, info in pins["arms"].items():
        require(sha(binaries / arm / "study_bots") == info["binaries"]["study_bots"], "bot binary identity drift")
    bot_runs = []
    plain_rows = load(root / "bots-plain/rows.json")
    require(len(plain_rows) == 200 and all(r["enabled"] > 0 and r["running"] / r["enabled"] >= .99 for r in plain_rows),
            "incomplete plain bot control")
    plain = {"instructionsPerTick": sum(r["instructions"] for r in plain_rows) / 1200,
             "minimumPmuScheduled": min(r["running"] / r["enabled"] for r in plain_rows),
             "rawSha256": sha(root / "bots-plain/rows.json")}
    for repeat in range(-1, 5):
        folder = root / f"bots-{repeat}"
        require(filecmp.cmp(folder / "world.json", root / "bots-plain/world.json", shallow=False), "bot diagnostic continuation changed")
        rows = load(folder / "rows.json")
        require(len(rows) == 200 and [r["decision"] for r in rows] == list(range(1000, 1200)), "incomplete bot window")
        require(all(r["enabled"] > 0 and r["running"] / r["enabled"] >= .99 for r in rows), "bot PMU starvation")
        sums = [sum(r["phases"]["instructions"][i] for r in rows) for i in range(24)]
        calls = [sum(r["phases"]["calls"][i] for r in rows) for i in range(24)]
        decisions = calls[13]
        require(decisions > 0 and calls[12] == decisions, "policy did not execute for all built frames")
        require(calls[9] == 1200 and calls[10] > 0 and calls[11] > 0, "client pipeline brackets did not execute")
        require(calls[14] == calls[15] == calls[16] == decisions, "missing bot observation census")
        bot_runs.append({"repeat": repeat, "liveBotDecisions": decisions, "aliveMean": statistics.mean(r["alive"] for r in rows),
                         "instructionsPerLiveBotDecision": {"broadcastAmortized": sums[9]/decisions,
                             "decodeAmortized": sums[10]/decisions, "encodeSubset": sums[11]/decisions,
                             "frameMaterialization": sums[12]/decisions, "policy": sums[13]/decisions},
                         "worldInstructionsPerTickIncludingClients": sum(r["instructions"] for r in rows)/1200,
                         "meanVisible": {"opponents": sums[14]/calls[14], "food": sums[15]/calls[15], "prey": sums[16]/calls[16]},
                         "backgroundReconnects": sums[17], "backgroundRefusals": sums[18],
                         "minimumPmuScheduled": min(r["running"] / r["enabled"] for r in rows),
                         "end": log_json(root / f"bots-{repeat}.log"),
                         "rawSha256": sha(folder / "rows.json"), "worldSha256": sha(folder / "world.json")})
    return {"buildPins": pins, "runsIncludingWarmup": bot_runs, "plainControl": plain,
            "diagnosticOuterInstructionsPerTick": distribution([r["worldInstructionsPerTickIncludingClients"] for r in bot_runs[1:]]),
            "perLiveDecision": {name: distribution([r["instructionsPerLiveBotDecision"][name] for r in bot_runs[1:]])
                                for name in bot_runs[0]["instructionsPerLiveBotDecision"]}}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--bots-only", action="store_true", help="Validate the independent bot-cost component only")
    args = parser.parse_args()
    root = args.raw.resolve()
    results = ({"format": "slither-population-bot-cost-v1", "pins": load(root / "pins.json"),
                "bots": select_bots(root), "selectorSha256": sha(Path(__file__))}
               if args.bots_only else select(root))
    results = publication_view(results, root.parent)
    with args.output.open("x") as stream:
        json.dump(results, stream, indent=2)
        stream.write("\n")
