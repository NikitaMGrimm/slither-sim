"""Run a localhost-only host capture, measure it and compare with the reference.

Creates a new output directory; always stops the host process tree. Exit 1 is
a parity gate failure, exit 2 an acquisition/tool error. No outside game servers.
"""

import argparse
from contextlib import redirect_stdout
import io
import ipaddress
import json
import math
import os
from pathlib import Path
import socket
import subprocess
import sys
import time

import transport_parity as tp

sys.path.insert(0, str(tp.ROOT / "tools"))
from perf_build import ProcessTree, output


def loopback_endpoint(bind):
    """Use a numeric IPv4 loopback address, also accepted by the headless CLI."""
    try:
        host, port_text = bind.split(":")
        address, port = ipaddress.IPv4Address(host), int(port_text)
        if not address.is_loopback or not 1 <= port <= 65535:
            raise ValueError()
        return str(address), port
    except (ValueError, AttributeError):
        raise ValueError("host bind must be a numeric IPv4 loopback address and port in 1..65535") from None


def run(config, out, seconds, reference, tolerances):
    config, out = config.resolve(), out.resolve()
    settings = json.loads(config.read_text(encoding="utf-8"))
    host, port = loopback_endpoint(settings.get("bind", "127.0.0.1:8766"))
    if not math.isfinite(seconds) or seconds <= 0:
        raise ValueError("seconds must be positive and finite")
    reference_report = json.loads(reference.read_text(encoding="utf-8"))
    tolerance_report = json.loads(tolerances.read_text(encoding="utf-8"))
    # Fail before starting Cargo or the collector if another process owns this port.
    with socket.socket() as probe:
        probe.bind((host, port))
    out.mkdir(parents=True, exist_ok=False)
    frozen_config = out / "host-config.json"
    frozen_config.write_text(json.dumps(settings, indent=2) + "\n", encoding="utf-8", newline="\n")
    environment = {**os.environ, "CARGO_BUILD_JOBS": "2", "RAYON_NUM_THREADS": "2"}
    environment.setdefault("CARGO_TARGET_DIR", str(tp.ROOT / "target" / "transport-parity"))
    command = ["cargo", "run", "--release", "--locked", "-j", "2", "--", "host",
               "--config", str(frozen_config), "--bind", f"{host}:{port}"]
    manifest = dict(hostCommand=command, gitHead=output(["git", "rev-parse", "HEAD"], tp.ROOT),
        configSha256=tp.pc.sha256(frozen_config), collectorSha256=tp.pc.sha256(
            tp.ROOT / "tools/fidelity/slither-headless.mjs"), seconds=seconds,
        targetDir=environment["CARGO_TARGET_DIR"])
    (out / "run.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
    with (out / "host.log").open("w", encoding="utf-8", newline="\n") as log:
        tree = ProcessTree(command, tp.ROOT, environment, log)
        try:
            deadline = time.monotonic() + 600
            while True:
                if tree.process.poll() is not None:
                    raise RuntimeError(f"host exited; see {out / 'host.log'}")
                try:
                    with socket.create_connection((host, port), timeout=.2):
                        break
                except OSError:
                    if time.monotonic() >= deadline:
                        raise TimeoutError(f"host not ready within 600s; see {out / 'host.log'}")
                    time.sleep(.1)
            with (out / "collector.log").open("w", encoding="utf-8", newline="\n") as collector_log:
                collector = ProcessTree(["node", "tools/fidelity/slither-headless.mjs", "--server",
                                         f"{host}:{port}", "--seconds", str(seconds), "--out", str(out / "raw")],
                                        tp.ROOT, environment, collector_log)
                try:
                    if collector.wait(timeout=seconds + 30):
                        raise RuntimeError(f"collector failed; see {out / 'collector.log'}")
                finally:
                    collector.stop()
        finally:
            tree.stop()
    capture = out / "raw/bot0.jsonl.gz"
    print(f"Capture: {capture}")
    report = tp.measure([capture])
    with (out / "measurement.json").open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(report, stream, indent=2, allow_nan=False)
        stream.write("\n")
    buffer = io.StringIO()
    with redirect_stdout(buffer):
        result = tp.compare(reference_report, report, tolerance_report)
    table = buffer.getvalue()
    (out / "comparison.txt").write_text(table, encoding="utf-8", newline="\n")
    print(table, end="")
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--seconds", type=float, default=60)
    parser.add_argument("--reference", type=Path, default=tp.DEFAULT_REFERENCE)
    parser.add_argument("--tolerances", type=Path, default=tp.DEFAULT_TOLERANCES)
    args = parser.parse_args(argv)
    try:
        return run(args.config, args.out, args.seconds, args.reference, args.tolerances)
    except (*tp.MEASUREMENT_ERRORS, RuntimeError, subprocess.SubprocessError) as error:
        print(f"local transport capture failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
