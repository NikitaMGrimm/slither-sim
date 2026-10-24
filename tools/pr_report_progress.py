"""Job-local checkpoint handshake; uploads run only while the sampler is paused."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from pr_report import JOB_TIMEOUT_SECONDS, RESERVE_SECONDS
from perf_build import ProcessTree


def control_directory(output):
    return output.with_suffix(".control")


def supervise(output, command, deadline):
    """Own the collector tree until it is reaped and its output is closed."""
    control = control_directory(output)
    stopped, code = False, None
    try:
        with output.with_suffix(".collector.log").open("xb") as stream:
            tree = ProcessTree(command, Path(__file__).resolve().parents[1], os.environ.copy(), stream)
            try:
                (control / "started").touch(exist_ok=False)
                while tree.process.poll() is None:
                    if (control / "stop").exists() or time.monotonic() >= deadline:
                        stopped = True
                        break
                    time.sleep(0.1)
                code = tree.process.poll()
            finally:
                # Includes descendants left behind by a normally exited leader.
                # Query, termination and timeout errors withhold acknowledgment.
                tree.stop(verify=True)
        with (control / "result.json").open("x", encoding="utf-8") as result:
            json.dump({"returncode": code, "stopped": stopped}, result)
        (control / "quiesced").touch(exist_ok=False)
        return 0
    except BaseException as error:
        with (control / "cleanup-failure.json").open("x", encoding="utf-8") as failure:
            json.dump({"type": type(error).__name__, "error": str(error)}, failure)
        return 1


def cleanup(output, timeout=65):
    """No acknowledgment means no upload; never signal a persisted/reused PID."""
    control = control_directory(output)
    if not control.is_dir():
        return False
    if not (control / "quiesced").is_file():
        (control / "stop").touch()
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if (control / "quiesced").is_file():
            return True
        if (control / "cleanup-failure.json").is_file():
            return False
        time.sleep(0.1)
    return False


def wait(output, sequence, deadline):
    if sequence > 1:
        previous = output / "progress" / str(sequence - 1)
        if (previous / "ready").is_file():
            (previous / "resume").touch()
    directory = output / "progress" / str(sequence)
    while time.monotonic() < deadline:
        if (directory / "ready").is_file():
            return True
        control = control_directory(output)
        if ((output / "done").is_file() or (control / "quiesced").is_file()
                or (control / "cleanup-failure.json").is_file()):
            return False
        time.sleep(0.25)
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("start", "supervise", "wait", "finish", "cleanup"))
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--head")
    parser.add_argument("--sequence", type=int, choices=range(1, 4))
    args = parser.parse_args()
    output = args.output.resolve()
    deadline = float(os.environ["PR_REPORT_JOB_START"]) + JOB_TIMEOUT_SECONDS - RESERVE_SECONDS
    if args.command == "start":
        if output.exists():
            parser.error("output must be new")
        control_directory(output).mkdir(exist_ok=False)
        environment = {**os.environ, "PR_REPORT_CHECKPOINT_UPLOAD": "1"}
        process = subprocess.Popen([sys.executable, "-u", str(Path(__file__)), "supervise",
                                    "--output", str(output), "--head", args.head],
                                   env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL,
                                   creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        output.with_suffix(".pid").write_text(str(process.pid), encoding="ascii")
    elif args.command == "supervise":
        command = [sys.executable, "-u", str(Path(__file__).with_name("pr_report.py")),
                   "--output", str(output), "--head", args.head]
        raise SystemExit(supervise(output, command, deadline + 30))
    elif args.command == "cleanup":
        safe = cleanup(output)
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as stream:
            stream.write(f"safe_to_upload={str(safe).lower()}\n")
        if not safe:
            raise RuntimeError("Collector quiescence was not verified; diagnostic upload is disabled")
    elif args.command == "wait":
        if args.sequence is None:
            parser.error("wait requires sequence")
        ready = wait(output, args.sequence, deadline)
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as stream:
            stream.write(f"ready={str(ready).lower()}\n")
    else:
        # All three upload slots have finished. Resume the final slot, if used.
        directory = output / "progress" / "3"
        if (directory / "ready").is_file():
            (directory / "resume").touch()
        control = control_directory(output)
        while not (control / "quiesced").is_file() and not (control / "cleanup-failure.json").is_file() and time.monotonic() < deadline + 30:
            time.sleep(0.25)
        if not (control / "quiesced").is_file():
            raise RuntimeError("Collector did not quiesce; cleanup must run before upload")
        log = output.with_suffix(".collector.log")
        if log.is_file():
            print(log.read_text(encoding="utf-8", errors="replace"))
        if not (output / "report.json").is_file() or (output / "failure.json").exists():
            raise RuntimeError("Collector did not produce a terminal report; retained logs identify the failure")
        result = json.loads((control / "result.json").read_text(encoding="utf-8"))
        if result["stopped"] or result["returncode"] != 0:
            raise RuntimeError("Collector failed or was stopped; clean process cleanup is not a successful comparison")
        report = json.loads((output / "report.json").read_text(encoding="utf-8"))
        if report.get("snapshot", {}).get("state") != "complete":
            raise ValueError("Terminal snapshot is missing")


if __name__ == "__main__":
    main()
