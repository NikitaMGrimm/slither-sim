"""Frozen continuation after initial pre-acquisition port reuse failure.

Preserves the first capture and failed attempt. Uses five fresh loopback ports.
The original run.py, plan and pinned shared tools remain unchanged.
"""

from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

import run


def main():
    out = Path(sys.argv[1]).resolve()
    study = Path(__file__).resolve().parent
    plan = json.loads((study / "second-continuation-plan.json").read_text())
    plan_sha = hashlib.sha256((study / "second-continuation-plan.json").read_bytes()).hexdigest()
    pins = json.loads((study / "second-continuation-source-pins.json").read_text())
    for name, digest in pins["files"].items():
        assert hashlib.sha256((run.ROOT / name).read_bytes()).hexdigest() == digest, name
    assert not (out / "second-continuation-attempts.jsonl").exists()
    cpus = json.loads((out / "machine.json").read_text())["cpus"]
    os.sched_setaffinity(0, {cpus[0]})
    environment = {**os.environ, "PATH": str(out / "bin") + ":" + os.environ["PATH"],
                   "UV_THREADPOOL_SIZE": "1", "CARGO_BUILD_JOBS": "2"}
    paths = {0: [out / "bots-0-r1/raw/bot0.jsonl.gz"], 500: []}
    records = json.loads((out / "all-attempts.json").read_text())

    def event(**fields):
        value = dict(schemaVersion=1, studyId=plan["studyId"], planSha256=plan_sha,
                     time=datetime.now(timezone.utc).isoformat(), **fields)
        with (out / "second-continuation-attempts.jsonl").open("a") as stream:
            stream.write(json.dumps(value) + "\n")
            stream.flush()
            os.fsync(stream.fileno())

    for index, (bots, repetition) in enumerate(((500, 1), (0, 2), (500, 2), (0, 3), (500, 3))):
        name = f"bots-{bots}-r{repetition}" + ("-retry2" if index == 0 else "")
        config = out / f"{name}.json"
        run.write(config, dict(bind=f"127.0.0.1:{8772 + index}", seed=20261025 + repetition,
                  players=dict(max=4, latency={}), bots=dict(count=bots, threads=2, latency={}),
                  simulation=dict(delivery="official")))
        environment.update(STUDY_BOTS=str(bots), STUDY_PIN_LOG=str(out / f"{name}-pins.json"))
        command = [sys.executable, "tools/fidelity/local_capture.py", "--config", str(config),
                   "--seconds", "300", "--out", str(out / name)]
        event(event="started", attemptId=name, command=command)
        started = time.time()
        with (out / f"{name}.log").open("x") as log:
            code = subprocess.call(command, cwd=run.ROOT, env=environment,
                                   stdout=log, stderr=subprocess.STDOUT)
        records.append(dict(attempt=name, split="heldout" if repetition == 3 else "development",
                            startedUnix=started, finishedUnix=time.time(), exitCode=code))
        run.write(out / "all-attempts.json", records)
        event(event="finished", attemptId=name, status="completed" if code == 0 else "failed",
              exitCode=code, captures=[f"raw/{name}.jsonl.gz"] if (out / name / "raw/bot0.jsonl.gz").exists() else [],
              error="transport gate failure" if code == 1 else "acquisition error" if code else None)
        if code not in (0, 1):
            raise RuntimeError("continuation acquisition failed; no automatic replacement")
        paths[bots].append(out / name / "raw/bot0.jsonl.gz")
        if index == 2:
            seal = {}
            for arm, captures in paths.items():
                dest = out / f"development-{arm}.json"
                run.write(dest, run.tp.measure(captures, summary_only=True, relative_to=out))
                run.write(out / f"development-{arm}-comparison.json", run.compare(json.loads(dest.read_text())))
                seal[dest.name] = hashlib.sha256(dest.read_bytes()).hexdigest()
            run.write(out / "development-seal.json", seal)
    for bots, captures in paths.items():
        report = run.tp.measure(captures, summary_only=True, relative_to=out)
        run.write(out / f"pooled-{bots}.json", report)
        run.write(out / f"pooled-{bots}-comparison.json", run.compare(report))
    run.write(out / "complete.json", dict(planSha256=hashlib.sha256((study / "plan.json").read_bytes()).hexdigest(),
              continuationPlanSha256=plan_sha, attempts=records))


if __name__ == "__main__":
    main()
