"""Run the frozen localhost transport study on a disposable Linux machine.

Raw captures and machine metadata are written only to an external output root.
The unmodified local_capture flow and transport gates remain authoritative.
"""

import argparse
from contextlib import redirect_stdout
from datetime import datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/fidelity"))
import transport_parity as tp


def write(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def compare(report):
    buffer = io.StringIO()
    with redirect_stdout(buffer):
        code = tp.compare(json.loads(tp.DEFAULT_REFERENCE.read_text()), report,
                          json.loads(tp.DEFAULT_TOLERANCES.read_text()))
    return dict(exitCode=code, table=buffer.getvalue())


def event(out, plan, **fields):
    row = dict(schemaVersion=1, studyId=plan["studyId"],
               planSha256=hashlib.sha256((Path(__file__).parent / "plan.json").read_bytes()).hexdigest(),
               time=datetime.now(timezone.utc).isoformat(), **fields)
    with (out / "attempts.jsonl").open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(row) + "\n")
        stream.flush()
        os.fsync(stream.fileno())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(exist_ok=False, parents=True)
    study = Path(__file__).resolve().parent
    plan = json.loads((study / "plan.json").read_text())
    pins = json.loads((study / "source-pins.json").read_text())
    if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() != pins["baseCommit"]:
        raise RuntimeError("source revision differs from frozen pin")
    for name, digest in pins["files"].items():
        if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != digest:
            raise RuntimeError(f"source pin differs: {name}")
    if (study / "plan.sha256").read_text().strip() != hashlib.sha256((study / "plan.json").read_bytes()).hexdigest():
        raise RuntimeError("frozen plan changed")
    # Select separate physical cores rather than SMT siblings.
    topology = subprocess.check_output(["lscpu", "-p=CPU,CORE,SOCKET"], text=True)
    cores = {}
    for line in topology.splitlines():
        if not line.startswith("#"):
            cpu, core, socket = map(int, line.split(","))
            if cpu in os.sched_getaffinity(0):
                cores.setdefault((socket, core), cpu)
    cpus = list(cores.values())[:6]
    if len(cpus) != 6:
        raise RuntimeError("six separate physical cores required")
    write(out / "machine.json", dict(topology=topology, cpus=cpus,
          uname=subprocess.check_output(["uname", "-a"], text=True)))
    os.sched_setaffinity(0, {cpus[0]})
    shim = out / "bin"
    shim.mkdir()
    real_node = subprocess.check_output(["which", "node"], text=True).strip()
    # local_capture calls node only after host readiness. Pin host workers before
    # the first collector packet; do not alter collection, config or gate code.
    wrapper = f'''#!/usr/bin/python3
import json, os
from pathlib import Path
cpus = {cpus!r}
pins = {{"slither-simulat": cpus[1], "slither-sockets": cpus[2],
        "slither-bots-0": cpus[3], "slither-bots-1": cpus[4]}}
found = {{}}
for task in Path("/proc").glob("[0-9]*/task/[0-9]*"):
    try:
        name = (task / "comm").read_text().strip()
        if name in pins:
            os.sched_setaffinity(int(task.name), {{pins[name]}})
            found[name] = sorted(os.sched_getaffinity(int(task.name)))
    except (FileNotFoundError, ProcessLookupError):
        pass
required = {{"slither-simulat", "slither-sockets"}}
if os.environ["STUDY_BOTS"] == "500":
    required.update(["slither-bots-0", "slither-bots-1"])
if set(found) != required:
    raise RuntimeError(str(found))
Path(os.environ["STUDY_PIN_LOG"]).write_text(json.dumps(found) + "\\n")
os.sched_setaffinity(0, {{cpus[5]}})
os.execv({real_node!r}, [{real_node!r}, *os.sys.argv[1:]])
'''
    (shim / "node").write_text(wrapper)
    (shim / "node").chmod(0o755)
    environment = {**os.environ, "PATH": str(shim) + ":" + os.environ["PATH"],
                   "UV_THREADPOOL_SIZE": "1", "CARGO_BUILD_JOBS": "2"}
    paths = {0: [], 500: []}
    records = []
    # No choices or fitting depend on development outcomes. Seal those outputs
    # before collecting either held-out capture.
    for repetition in range(1, 4):
        for bots in (0, 500):
            name = f"bots-{bots}-r{repetition}"
            config = out / f"{name}.json"
            write(config, dict(bind="127.0.0.1:8766", seed=20261025 + repetition,
                  players=dict(max=4, latency={}), bots=dict(count=bots, threads=2, latency={}),
                  simulation=dict(delivery="official")))
            environment.update(STUDY_BOTS=str(bots), STUDY_PIN_LOG=str(out / f"{name}-pins.json"))
            command = [sys.executable, "tools/fidelity/local_capture.py", "--config", str(config),
                       "--seconds", "300", "--out", str(out / name)]
            event(out, plan, event="started", attemptId=name, command=command)
            started = time.time()
            with (out / f"{name}.log").open("w") as log:
                code = subprocess.call(command,
                       cwd=ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT)
            row = dict(attempt=name, split="heldout" if repetition == 3 else "development",
                       startedUnix=started, finishedUnix=time.time(), exitCode=code)
            records.append(row)
            write(out / "attempts.json", records)
            event(out, plan, event="finished", attemptId=name,
                  status="completed" if code == 0 else "failed", exitCode=code,
                  captures=[f"raw/{name}.jsonl.gz"] if (out / name / "raw/bot0.jsonl.gz").exists() else [],
                  error="transport gate failure" if code == 1 else "acquisition error" if code else None)
            if code not in (0, 1):
                raise RuntimeError(f"acquisition failed: {name}; retained without replacement")
            paths[bots].append(out / name / "raw/bot0.jsonl.gz")
        if repetition == 2:
            seal = {}
            for bots, captures in paths.items():
                report = tp.measure(captures, summary_only=True, relative_to=out)
                dest = out / f"development-{bots}.json"
                write(dest, report)
                write(out / f"development-{bots}-comparison.json", compare(report))
                seal[dest.name] = hashlib.sha256(dest.read_bytes()).hexdigest()
            write(out / "development-seal.json", seal)
    for bots, captures in paths.items():
        report = tp.measure(captures, summary_only=True, relative_to=out)
        write(out / f"pooled-{bots}.json", report)
        write(out / f"pooled-{bots}-comparison.json", compare(report))
    write(out / "complete.json", dict(planSha256=hashlib.sha256((study / "plan.json").read_bytes()).hexdigest(),
          attempts=records))


if __name__ == "__main__":
    main()
