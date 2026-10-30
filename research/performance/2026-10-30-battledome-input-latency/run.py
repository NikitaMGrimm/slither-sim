"""Frozen remote-only collection driver; JOB contains private IPs, never publish it.

python3 run.py ROOT JOB_JSON
Native binaries are ROOT/bin/{clients,probe}; target runs server_agent.py.
"""
from pathlib import Path
import hashlib
import json
import os
import signal
import subprocess
import sys
import threading
import time
import urllib.request
from checkpoint import checkpoint

root = Path(sys.argv[1]).resolve()
os.sched_setaffinity(0, {7})
job = json.loads(Path(sys.argv[2]).read_text())
server = job["serverPrivate"]
endpoint = f"http://{server}:18769"
rows = root / "rows"
rows.mkdir(exist_ok=True)
started = time.monotonic()


def atomic_text(path, value):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(value)
    temporary.replace(path)


def status(value):
    value["unix"] = time.time()
    temporary = root / "progress.tmp"
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(root / "progress.json")


def api(action, **values):
    request = urllib.request.Request(endpoint, json.dumps(dict(action=action, **values)).encode())
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def wait(seconds, stage):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        assert time.monotonic() - started < 90 * 60, "Planned collection deadline"
        assert api("status")["alive"], "Host exited"
        status({"stage": stage, "remaining": round(until - time.monotonic())})
        time.sleep(min(3, max(0, until - time.monotonic())))


class Capture:
    def __init__(self, directory):
        self.directory = directory
        self.stop = threading.Event()
        self.children = []
        self.error = None
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()

    def run(self):
        seen = set()
        try:
            while not self.stop.is_set():
                for path in self.directory.glob("connection-*.json"):
                    if path.name in seen:
                        continue
                    note = json.loads(path.read_text())
                    life = note["life"]
                    self.close_children()
                    output = self.directory / f"capture-{life}.pcap"
                    log = self.directory / f"capture-{life}.log"
                    stream = log.open("w")
                    process = subprocess.Popen(["taskset", "-c", "8", "sudo", "-n", "tcpdump", "--immediate-mode", "-i", "any", "-s", "0",
                                                "--time-stamp-precision=nano", "-U", "-Z", "ubuntu",
                                                "-w", str(output), "tcp", "and", "host", server,
                                                "and", "port", "8766", "and", "port", str(note["localPort"])],
                                               stdout=subprocess.DEVNULL, stderr=stream,
                                               start_new_session=True)
                    self.children.append((process, stream))
                    deadline = time.monotonic() + 3
                    while "listening on" not in log.read_text():
                        assert process.poll() is None and time.monotonic() < deadline, "Capture failed"
                        time.sleep(.02)
                    (self.directory / f"capture-ready-{life}").write_text("ready\n")
                    seen.add(path.name)
                time.sleep(.02)
        except Exception as error:
            self.error = str(error)

    def finish(self):
        self.stop.set()
        self.thread.join(timeout=5)
        assert not self.thread.is_alive()
        self.close_children()
        assert self.error is None, self.error

    def close_children(self):
        if self.children:
            time.sleep(.25)
        for process, stream in self.children:
            subprocess.run(["sudo", "-n", "kill", "-INT", "--", f"-{process.pid}"], check=True)
            process.wait(timeout=10)
            stream.close()
            assert process.returncode == 0, "Capture did not close cleanly"
        self.children.clear()


def start_probe(label):
    directory = rows / label
    directory.mkdir()
    capture = Capture(directory)
    log = (directory / "probe.log").open("w")
    seed = {"smoke": 700, "01-solo": 701, "02-80": 702, "03-160": 703, "04-320": 704}[label]
    process = subprocess.Popen(["taskset", "-c", "6", str(root / "bin/probe"), server, "8766",
                                str(directory), "1500", str(seed)],
                               stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    return directory, capture, process, log


def measure(label, seats, probe):
    directory, capture, process, log = probe
    api("mark", label=label + ":start")
    start = time.time()
    (directory / "START").write_text("start\n")
    atomic_text(root / "load/RESET", label)
    wait(360, label + ":count")
    end = time.time()
    api("mark", label=label + ":end")
    (directory / "STOP").write_text("stop\n")
    process.wait(timeout=30)
    log.close()
    capture.finish()
    assert process.returncode == 0 and (directory / "COMPLETE").exists(), "Probe failed"
    (directory / "window.json").write_text(json.dumps({"label": label, "seats": seats,
                                                      "startUnix": start, "endUnix": end,
                                                      "selectionRole": "primary" if job.get("primaryOnly", seats) == seats else "calibration"}) + "\n")
    checkpoint(root, directory, api, endpoint)
    status({"stage": label + ":complete", "probeBytes": (directory / "probe.jsonl").stat().st_size})


def main():
    controls = {name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
                for name in ["run.py", "checkpoint.py", "server_agent.py"]}
    (root / "control-manifest.json").write_text(json.dumps(controls, sort_keys=True, indent=2) + "\n")
    (root / "load").mkdir()
    ping_log = (root / "icmp.log").open("w")
    ping = subprocess.Popen(["stdbuf", "-oL", "ping", "-n", "-D", "-i", "0.2", server], stdout=ping_log,
                            stderr=subprocess.STDOUT, start_new_session=True)
    load = None
    load_log = None
    try:
        if job.get("primaryOnly") != 320:
            api("start", label="solo", bind=f"{server}:8766")
            probe = start_probe("01-solo")
            wait(30, "solo:settle")
            measure("01-solo", 1, probe)
            api("stop")
        api("start", label="loaded", bind=f"{server}:8766")
        directory = root / "load"
        atomic_text(directory / "target", "79\n")
        load_log = (root / "clients.jsonl").open("w")
        load = subprocess.Popen([str(root / "bin/clients"), str(directory), "8766", "79", server],
                                stdout=subprocess.DEVNULL, stderr=load_log, start_new_session=True)
        for label, seats, settle in [("02-80", 80, 300), ("03-160", 160, 60), ("04-320", 320, 60)]:
            atomic_text(directory / "target", str(seats - 1) + "\n")
            probe = start_probe(label)
            wait(settle, label + ":settle")
            assert load.poll() is None, "Generator exited"
            measure(label, seats, probe)
            assert load.poll() is None, "Generator exited"
    finally:
        (root / "load/STOP").touch()
        if load is not None:
            load.wait(timeout=30)
            load_log.close()
        os.killpg(ping.pid, signal.SIGINT)
        ping.wait(timeout=5)
        ping_log.close()
        api("stop")
        if load is not None:
            assert load.returncode == 0, "Load worker failed; preserve complete row artifacts for qualification"
    meta = api("archive")
    assert meta["complete"] and meta["allMembersVerified"]
    urllib.request.urlretrieve(endpoint + "/server.tar.gz", root / "server.tar.gz")
    assert (root / "server.tar.gz").stat().st_size == meta["bytes"]
    with (root / "server.tar.gz").open("rb") as stream:
        assert hashlib.file_digest(stream, "sha256").hexdigest() == meta["sha256"]
    (root / "server-retrieval.json").write_text(json.dumps(meta) + "\n")
    status({"stage": "collection-complete", "serverArchiveVerified": True})


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        status({"stage": "failed", "error": str(error)})
        raise
