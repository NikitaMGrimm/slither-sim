"""Run one declared headless capture attempt, preserving failures and partial files."""

import argparse
import ipaddress
import json
from pathlib import Path
import shutil
import subprocess
import uuid

import evidence


def capture_command(study, output, node="node"):
    study = Path(study).resolve()
    plan = evidence.read_plan(study)
    if "capture" not in plan:
        raise ValueError("socket capture requires a plan with capture settings")
    capture = plan["capture"]
    address = ipaddress.ip_address(capture["server"]["ip"])
    if address.version != 4:
        raise ValueError("this collector requires an explicit IPv4 server")
    if capture["scenario"] in ("preyprobe", "boundaryprobe", "visibilityprobe", "speedprobe") and capture["bots"] != 1:
        raise ValueError("this scenario requires exactly one client")
    executable = shutil.which(node)
    if executable is None:
        raise ValueError(f"Node executable not found: {node}")
    paired = capture["scenario"] == "paired-prey"
    driver = "slither-paired-prey.mjs" if paired else "slither-headless.mjs"
    command = [
        executable, str(evidence.ROOT / "tools/fidelity" / driver),
        "--out", str(output), "--plan", str(study / "plan.json"),
        "--server", f"{address}:{capture['server']['port']}",
        "--seconds", str(capture["seconds"]),
    ]
    if paired:
        command.extend(["--mode", capture["mode"]])
        if "direction" in capture:
            command.extend(["--direction", capture["direction"]])
    else:
        command.extend(["--bots", str(capture["bots"]), "--scenario", capture["scenario"]])
    if "origin" in capture:
        command.extend(["--origin", capture["origin"]])
    return command


def _stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def collect(study, node="node"):
    study = Path(study).resolve()
    with evidence.study_lock(study):
        plan = evidence.read_plan(study)
        starts, _ = evidence.read_attempts(study)
        if len(starts) >= plan["sampling"]["attempts"]:
            raise ValueError("all planned attempts are already reserved; no automatic replacements")
        attempt = str(uuid.uuid4())
        output = study / "raw" / attempt
        command = capture_command(study, output, node)
        base = {"schemaVersion": 1, "attemptId": attempt, "studyId": plan["studyId"],
                "planSha256": evidence.sha256((study / "plan.json").read_bytes())}
        evidence.append_event(study, {**base, "event": "started", "time": evidence.utc_now(), "command": command})
        logs = study / "logs"
        logs.mkdir(exist_ok=True)
        status, code, error = "failed", None, None
        process = None
        try:
            with (logs / f"{attempt}.log").open("xb") as log:
                process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
                code = process.wait(timeout=plan["capture"]["seconds"] + 30)
                status = "completed" if code == 0 else "failed"
        except KeyboardInterrupt:
            status, error = "interrupted", "controller interrupted"
        except (OSError, subprocess.TimeoutExpired) as failure:
            error = str(failure)[:2000]
        finally:
            if process is not None:
                _stop(process)
                code = process.returncode
            captures = sorted(path.relative_to(study).as_posix() for path in output.glob("*.jsonl.gz"))
            # Audit errors must not erase the process outcome or free its attempt.
            evidence.append_event(study, {**base, "event": "finished", "time": evidence.utc_now(),
                "status": status, "exitCode": code, "captures": captures, "error": error})
        return {"attemptId": attempt, "status": status, "exitCode": code,
                "captures": captures, "audit": evidence.audit_study(study)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("study", type=Path)
    parser.add_argument("--node", default="node")
    parser.add_argument("--dry-run", action="store_true", help="print the command without creating an attempt or connecting")
    args = parser.parse_args()
    try:
        if args.dry_run:
            result = {"dryRun": True, "command": capture_command(args.study, Path(args.study).resolve() / "raw/DRY-RUN", args.node)}
        else:
            result = collect(args.study, args.node)
        print(json.dumps(result, indent=2, allow_nan=False))
        return 0 if args.dry_run or result["audit"]["status"] == "verified" else 2
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
