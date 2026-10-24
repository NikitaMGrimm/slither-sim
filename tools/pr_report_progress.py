"""Job-local checkpoint handshake; uploads run only while the sampler is paused."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

from pr_report import JOB_TIMEOUT_SECONDS, RESERVE_SECONDS


def wait(output, sequence, deadline):
    if sequence > 1:
        previous = output / "progress" / str(sequence - 1)
        if (previous / "ready").is_file():
            (previous / "resume").touch()
    directory = output / "progress" / str(sequence)
    while time.monotonic() < deadline:
        if (directory / "ready").is_file():
            return True
        if (output / "done").is_file():
            return False
        time.sleep(0.25)
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("start", "wait", "finish"))
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--head")
    parser.add_argument("--sequence", type=int, choices=range(1, 4))
    args = parser.parse_args()
    output = args.output.resolve()
    deadline = float(os.environ["PR_REPORT_JOB_START"]) + JOB_TIMEOUT_SECONDS - RESERVE_SECONDS
    if args.command == "start":
        if output.exists():
            parser.error("output must be new")
        log = output.with_suffix(".collector.log")
        environment = {**os.environ, "PR_REPORT_CHECKPOINT_UPLOAD": "1"}
        with log.open("xb") as stream:
            process = subprocess.Popen([sys.executable, "-u", str(Path(__file__).with_name("pr_report.py")),
                                        "--output", str(output), "--head", args.head],
                                       env=environment, stdin=subprocess.DEVNULL, stdout=stream, stderr=stream,
                                       creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        output.with_suffix(".pid").write_text(str(process.pid), encoding="ascii")
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
        while not (output / "done").is_file() and time.monotonic() < deadline + 30:
            time.sleep(0.25)
        log = output.with_suffix(".collector.log")
        if log.is_file():
            print(log.read_text(encoding="utf-8", errors="replace"))
        if not (output / "report.json").is_file() or (output / "failure.json").exists():
            raise RuntimeError("Collector did not produce a terminal report; retained logs identify the failure")
        report = json.loads((output / "report.json").read_text(encoding="utf-8"))
        if report.get("snapshot", {}).get("state") != "complete":
            raise ValueError("Terminal snapshot is missing")


if __name__ == "__main__":
    main()
