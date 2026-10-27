"""Linux-only wrapper around the retained AWS capacity harness; stdlib only.

Install beside run.sh under ~/slither-bench with a repository snapshot in
harness/ and unmodified git archives in archives/FULL_SHA.tar. No network fetches.
"""
import argparse
import datetime as dt
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import time

ROOT = Path(__file__).resolve().parent
PERF = ROOT / "harness/research/performance"
BASE = "4c2471ba9afff3609faae964cdaad49e3209da68"
INDEX = "65ff713731cbd436bf5c19a25705fa77b0ebedcc"
MAIN = "9960f4f84d1b831d4468486b1e0524710b7b9c45"


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def replace(path, old, new):
    text = path.read_text()
    require(text.count(old) == 1, f"overlay anchor mismatch in {path.name}: {old!r}")
    path.write_text(text.replace(old, new), newline="\n")


def commit(value):
    aliases = {"v0.7.0": BASE, "index": INDEX, "main": MAIN}
    value = aliases.get(value, value)
    matches = [p.stem for p in (ROOT / "archives").glob(value + "*.tar")]
    require(len(matches) == 1 and re.fullmatch(r"[0-9a-f]{40}", matches[0]),
            "commit must identify exactly one installed full-SHA archive")
    return matches[0]


def build(revision):
    out = ROOT / "bin" / revision
    require(not out.exists(), "binary directory exists; preserve it or choose a new commit")
    source = ROOT / "sources" / revision
    target = ROOT / "targets" / revision
    require(not source.exists() and not target.exists(), "build path already exists; inspect failed build")
    require(not any((ROOT / "targets").glob("*/release")), "another target exists; inspect it before building")
    require(shutil.disk_usage(ROOT).free > 3 * 1024**3, "less than 3 GiB free")
    source.mkdir(parents=True)
    with tarfile.open(ROOT / "archives" / (revision + ".tar")) as archive:
        archive.extractall(source, filter="data")
    overlay = module("input_overlay", PERF / "2026-10-27-host-tick-handoff/instrument.py")
    overlay.instrument(source, PERF / "2026-10-26-host-capacity",
                       PERF / "2026-10-26-epyc-host/harness.patch")
    worker = source / "scripts/run_trial.py"
    replace(worker, "'bind': '0.0.0.0:8766'", "'bind': '127.0.0.1:8766'")
    replace(worker, "str(i + (1 if args.watcher else 0))", "str(1 + i % 3)")
    replace(worker, "        log = open(out / (name + '.log'), 'w')",
            "        if name == 'host': command = ['taskset', '-c', '0', *command]\n"
            "        log = open(out / (name + '.log'), 'w')")
    replace(worker, "'cpus':sorted(os.sched_getaffinity(int(tid.name))), **value}",
            "'cpus':sorted(os.sched_getaffinity(int(tid.name))), "
            "'lastCpu':int((tid/'stat').read_text().rsplit(')',1)[1].split()[36]), **value}")
    # The generic AWS reducer counts only non-loopback NICs; retain lo instead.
    replace(worker, "        if name.strip() == 'lo':\n            continue",
            "        if name.strip() != 'lo':\n            continue")
    out.mkdir(parents=True)
    env = dict(os.environ, CARGO_BUILD_JOBS="1", CARGO_TARGET_DIR=str(target),
               CARGO_PROFILE_RELEASE_DEBUG="1")
    require(not env.get("RUSTFLAGS") and not env.get("CARGO_ENCODED_RUSTFLAGS"), "unexpected Rust flags")
    command = ["cargo", "build", "--release", "--locked", "--bin", "slither-sim"]
    if revision == BASE:
        command += ["--example", "host_capacity_load"]
    with (out / "build.log").open("w") as log:
        subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    shutil.copy2(target / "release/slither-sim", out / "slither-sim")
    shutil.copy2(worker, out / "run_trial.py")
    if revision == BASE:
        shutil.copy2(target / "release/examples/host_capacity_load", ROOT / "bin/host_capacity_load")
    pins = {"commit": revision, "archiveSha256": sha(ROOT / "archives" / (revision + ".tar")),
            "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
            "cargo": subprocess.check_output(["cargo", "--version"], text=True).strip(),
            "command": command, "profile": {"lto": "thin", "codegenUnits": 1, "debug": 1,
                                             "jobs": 1, "rustflags": ""},
            "binarySha256": sha(out / "slither-sim"), "workerSha256": sha(out / "run_trial.py"),
            "instrumentedSourceSha256": {str(p.relative_to(source)): sha(p) for p in [
                source / "src/main.rs", source / "src/client/server.rs",
                source / "src/client/server/study_input.rs", source / "src/client/simulation/connection.rs",
                source / "examples/host_capacity_load.rs"]}}
    if revision == BASE:
        pins["loadBinarySha256"] = sha(ROOT / "bin/host_capacity_load")
    save(out / "pins.json", pins)
    # Only delete the exact target created above; retain source/binaries and failed builds.
    require(target.resolve().parent == (ROOT / "targets").resolve(), "target escaped bench root")
    shutil.rmtree(target)
    print("BUILT", revision, pins["binarySha256"], flush=True)


def affinity(path, expected):
    data = json.loads(path.read_text())
    samples = 0
    observed = set()
    last = set()
    violations = 0
    for row in data["rows"]:
        # taskset's pre-exec process can be observed before applying its mask.
        # Check the actual workload from the shared epoch through shutdown.
        if row["unix"] < data["epoch"]:
            continue
        for group in row["threads"].values():
            for thread in group.values():
                samples += 1
                observed.update(thread["cpus"])
                last.add(thread["lastCpu"])
                violations += (not set(thread["cpus"]).issubset(expected)
                               or thread["lastCpu"] not in expected)
    return {"threadSamples": samples, "allowedCpuUnion": sorted(observed),
            "lastCpuUnion": sorted(last), "violations": violations}


def listening():
    return any(fields[1].endswith(f":{8766:04X}") and fields[3] == "0A"
               for fields in (line.split() for line in Path("/proc/net/tcp").read_text().splitlines()[1:]))


def reduce_trial(out, count):
    analysis = module("tick_analysis", PERF / "2026-10-27-epyc-host-recheck/analyze.py")
    result = analysis.reduce(out / "host", out / "load", count)
    inputs = module("input_analysis", PERF / "2026-10-27-host-tick-handoff/analyze_inputs.py")
    result["sampledInput"] = inputs.reduce(out / "host", out / "load")
    result["hostAffinity"] = affinity(out / "host/telemetry.json", {0})
    result["generatorAffinity"] = affinity(out / "load/telemetry.json", {1, 2, 3})
    telemetry = json.loads((out / "load/telemetry.json").read_text())
    a = next(r for r in telemetry["rows"] if r.get("boundary") == "start")
    b = next(r for r in telemetry["rows"] if r.get("boundary") == "end")
    per_core = {str(i): 0. for i in [1, 2, 3]}
    for pid, value in result["generatorResources"]["perProcessCpuPercentOneCore"].items():
        threads = a["threads"][pid]
        require(len(threads) == 1, "generator unexpectedly multi-threaded")
        cpu = str(next(iter(threads.values()))["cpus"][0])
        per_core[cpu] += value
    result["generatorCpuPercentByPinnedCore"] = per_core
    result["resourceBoundarySkewSeconds"] = {
        "generatorStart": a["unix"] - telemetry["epoch"] - telemetry["warmup"],
        "generatorEnd": b["unix"] - telemetry["epoch"] - telemetry["warmup"] - telemetry["hold"]}
    # Allow kernel-local identities only in retained raw telemetry, never selected results.
    for key in ["hostResources", "generatorResources"]:
        resources = result[key]
        resources["processCpuPercentOneCore"] = list(resources.pop("perProcessCpuPercentOneCore").values())
        resources["loopback"] = resources.pop("nic")
    result.pop("hostNicBytesPerPlayingClientSecond", None)
    result["validTraffic"] = not result.get("errors") and result["excessMs"]["count"] > 0
    result["validAffinity"] = all(result[k]["threadSamples"] > 0 and result[k]["violations"] == 0
                                  for k in ["hostAffinity", "generatorAffinity"])
    result["generatorHasHeadroom"] = max(per_core.values()) < 90
    result["rawFiles"] = {str(p.relative_to(out)): {"sha256": sha(p), "bytes": p.stat().st_size}
                          for p in sorted(out.rglob("*")) if p.is_file() and p.name != "summary.json"}
    return result


def trial(revision, preset, count, hold=120, warmup=15, label="qualification"):
    require(1 <= count <= 1000, "count must be 1..1000")
    require(not listening(), "bench port already in use; leave existing process untouched")
    bins = ROOT / "bin" / revision
    pins = json.loads((bins / "pins.json").read_text())
    require(sha(bins / "slither-sim") == pins["binarySha256"], "host binary changed")
    base_pins = json.loads((ROOT / "bin" / BASE / "pins.json").read_text())
    require(sha(ROOT / "bin/host_capacity_load") == base_pins["loadBinarySha256"], "load binary changed")
    require(sha(bins / "run_trial.py") == pins["workerSha256"], "worker changed")
    require(shutil.disk_usage(ROOT).free > 1024**3, "less than 1 GiB free")
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    out = ROOT / "raw" / f"{stamp}-{label}-{revision[:7]}-{preset}-{count}"
    out.mkdir(parents=True)
    epoch = time.time() + 8
    save(out / "attempt.json", {"commit": revision, "preset": preset, "count": count,
                               "hold": hold, "warmup": warmup, "label": label, "epoch": epoch})
    # Each per-trial bin dir uses the same frozen generator and the selected host.
    trial_bin = out / "bin"
    trial_bin.mkdir()
    for name, path in [("slither-sim", bins / "slither-sim"),
                       ("host_capacity_load", ROOT / "bin/host_capacity_load")]:
        (trial_bin / name).symlink_to(path)
    children, logs = [], []
    try:
        for role in ["host", "load"]:
            command = [sys.executable, str(bins / "run_trial.py"), role, str(out / role),
                       "--bin", str(trial_bin), "--count", str(count), "--preset", preset,
                       "--shards", "8", "--warmup", str(warmup), "--hold", str(hold), "--epoch", str(epoch)]
            log = (out / (role + "-worker.log")).open("w")
            logs.append(log)
            children.append(subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT))
            if role == "host":
                while not listening():
                    require(children[-1].poll() is None and time.time() < epoch - 1,
                            "host failed to listen before shared epoch; attempt retained")
                    time.sleep(.01)
        for child in children:
            require(child.wait(timeout=max(1, epoch + warmup + hold + 60 - time.time())) == 0,
                    "worker failed; raw attempt retained")
        result = reduce_trial(out, count)
        result.update(commit=revision, preset=preset, label=label, attempt=out.name)
        save(out / "summary.json", result)
        require(result["validTraffic"] and result["validAffinity"] and result["generatorHasHeadroom"],
                "traffic/affinity/generator validity failed; stop and inspect retained attempt")
        print("RESULT", revision[:7], preset, count, "cpu_ms/tick", round(result["cpuMsPerExecutedTick"], 4),
              "excess_p99", result["excessMs"]["p99"], "deferred", result["deferredTicks"], flush=True)
        return result
    finally:
        for child in children:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=15)
        for log in logs:
            log.close()


def suite():
    for revision in [BASE, INDEX, MAIN]:
        trial(revision, "official", 8, 8, 3, "smoke")
    for preset, count, order in [("official", 500, [BASE, INDEX, MAIN]),
                                  ("official", 375, [MAIN, INDEX, BASE]),
                                  ("battledome", 80, [BASE, INDEX, MAIN])]:
        for revision in order:
            trial(revision, preset, count)


def select():
    attempts = []
    for path in sorted((ROOT / "raw").glob("*/attempt.json")):
        summary = path.with_name("summary.json")
        if summary.exists():
            result = json.loads(summary.read_text())
            clients = result.pop("perClient")
            result["perClientExcessP99MsSpread"] = {
                "clients": len(clients),
                **module("ranks", PERF / "2026-10-26-host-capacity/analyze.py").stats(
                    c["excessMs"]["p99"] for c in clients.values() if c["excessMs"]["count"])}
            result["rawSummarySha256"] = sha(summary)
            attempts.append(result)
        else:
            attempts.append({**json.loads(path.read_text()), "attempt": path.parent.name, "incomplete": True})
    harness_files = [ROOT / "bench.py", ROOT / "run.sh", ROOT / "verify.py", *(
        PERF / "2026-10-27-native-zen5-bench").glob("PLAN.md")]
    for directory, names in [
        ("2026-10-26-host-capacity", ["host_capacity_load.rs", "run_trial.py", "analyze.py"]),
        ("2026-10-26-epyc-host", ["harness.patch", "analyze.py"]),
        ("2026-10-27-epyc-host-recheck", ["instrument.py", "analyze.py"]),
        ("2026-10-27-host-tick-handoff", ["instrument.py", "analyze_inputs.py"])]:
        harness_files.extend(PERF / directory / name for name in names)
    save(ROOT / "selected-results.json", {
        "machine": "a 4-vCPU Zen 5 EPYC KVM guest (Debian 13)",
        "mainAtSetup": MAIN, "loadBuiltFrom": BASE,
        "dataOrigin": "local-simulator", "rawAccess": "local-only; no public download",
        "seed": 7, "bots": 0, "shards": 8, "warmupSeconds": 15, "holdSeconds": 120,
        "grade": "excess p99 <=10 ms and zero newly deferred ticks, subject to traffic and occupancy",
        "builds": [json.loads((ROOT / "bin" / rev / "pins.json").read_text()) for rev in [BASE, INDEX, MAIN]],
        "harnessSha256": {str(p.relative_to(ROOT)): sha(p) for p in harness_files},
        "retainedWrapperSha256": {p.name: sha(p) for p in sorted((ROOT / "retained").glob("*.py"))},
        "verification": json.loads((ROOT / "verification.json").read_text())
                        if (ROOT / "verification.json").exists() else None,
        "attempts": attempts})


if __name__ == "__main__":
    lock = (ROOT / ".bench.lock").open("w")
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    os.sched_setaffinity(0, {1, 2, 3})
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    p = sub.add_parser("build")
    p.add_argument("commit")
    p = sub.add_parser("trial")
    p.add_argument("commit")
    p.add_argument("--preset", choices=["official", "battledome"], default="official")
    p.add_argument("--count", type=int, default=500)
    sub.add_parser("suite")
    sub.add_parser("select")
    args = parser.parse_args()
    if args.action == "build":
        build(commit(args.commit))
    elif args.action == "trial":
        trial(commit(args.commit), args.preset, args.count, label="rerun")
    elif args.action == "suite":
        suite()
    else:
        select()
