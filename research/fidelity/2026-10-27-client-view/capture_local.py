"""Bounded loopback-only collection; always tears down the child host.

Build separately with -j 1. No outside address, credentials or persistent service.
"""
import argparse
import ipaddress
import json
from pathlib import Path
import socket
import subprocess
import time
from replay import dump, limit_cpus, sha


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--host", type=Path, required=True)
    p.add_argument("--repo", type=Path, required=True)
    p.add_argument("--config", type=Path, default=Path(__file__).with_name("host-config.json"))
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--seconds", type=int, default=120)
    p.add_argument("--warmup", type=int, default=30)
    a = p.parse_args()
    if not 1 <= a.seconds <= 600 or not 0 <= a.warmup <= 120:
        p.error("seconds must be 1..600 and warmup 0..120")
    config = json.loads(a.config.read_text())
    address, port = config["bind"].split(":")
    if not ipaddress.IPv4Address(address).is_loopback or not 1 <= int(port) <= 65535:
        p.error("config must bind numeric IPv4 loopback")
    limit_cpus()
    with socket.socket() as probe:
        probe.bind((address, int(port)))
    a.output.mkdir(parents=True, exist_ok=False)
    dump(a.output / "config.json", config)
    collector = a.repo / "tools/fidelity/slither-headless.mjs"
    manifest = dict(config_sha256=sha(a.config), host_sha256=sha(a.host),
                    collector_sha256=sha(collector), warmup_seconds=a.warmup,
                    seconds=a.seconds, data_origin="local-simulator",
                    git_head=subprocess.check_output(["git", "-C", str(a.repo), "rev-parse", "HEAD"], text=True).strip())
    dump(a.output / "manifest.json", manifest)
    process = None
    try:
        with (a.output / "host.log").open("w") as log:
            process = subprocess.Popen([str(a.host.resolve()), "host", "--config",
                                       str((a.output / "config.json").resolve())], stdout=log, stderr=log)
            deadline = time.monotonic() + 30
            while True:
                if process.poll() is not None:
                    raise RuntimeError("host exited; inspect retained host.log")
                try:
                    with socket.create_connection((address, int(port)), timeout=0.2):
                        break
                except OSError:
                    if time.monotonic() > deadline:
                        raise TimeoutError("host did not bind in 30 seconds")
                    time.sleep(0.05)
            print("loopback host ready; warming up", flush=True)
            # This background collector is bounded; the parent remains free to report progress.
            time.sleep(a.warmup)
            with (a.output / "collector.log").open("w") as log:
                result = subprocess.run(["node", str(collector.resolve()), "--server", config["bind"],
                    "--out", str((a.output / "raw").resolve()), "--seconds", str(a.seconds)],
                    stdout=log, stderr=log, timeout=a.seconds + 30, check=False)
            dump(a.output / "completion.json", dict(collector_exit_code=result.returncode))
            if result.returncode:
                raise RuntimeError("collector failed; inspect retained collector.log")
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
    print("capture complete; host stopped", flush=True)


if __name__ == "__main__":
    main()
