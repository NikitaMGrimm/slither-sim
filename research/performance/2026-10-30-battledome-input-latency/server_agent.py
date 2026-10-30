"""Private-network controller around the unmodified host CLI, not a runtime hook.

Run only on the disposable study target: python3 server_agent.py ROOT PORT.
Endpoints/configs and complete logs stay in the private raw archive.
"""
from pathlib import Path
from http.server import BaseHTTPRequestHandler, HTTPServer
import hashlib
import json
import os
import signal
import subprocess
import sys
import tarfile
import threading
import time

root = Path(sys.argv[1]).resolve()
root.mkdir(exist_ok=True, parents=True)
host = None
reader = None
lock = threading.Lock()


def event(value):
    with lock, (root / "events.jsonl").open("a") as stream:
        stream.write(json.dumps(value) + "\n")


def read_output(process, label):
    with (root / f"{label}.jsonl").open("w") as stream:
        for line in process.stderr:
            stream.write(json.dumps({"unix": time.time(), "mono": time.monotonic(),
                                     "line": line.rstrip()}) + "\n")
            stream.flush()


def stop():
    global host, reader
    if host is not None:
        if host.poll() is None:
            os.killpg(host.pid, signal.SIGTERM)
        host.wait(timeout=10)
        reader.join(timeout=5)
        assert not reader.is_alive()
        host = None


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        global host, reader
        try:
            request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            action = request["action"]
            if action == "start":
                stop()
                label = request["label"]
                assert label in ["solo", "loaded"]
                config = {"bind": request["bind"], "seed": 7, "players": {"max": 641},
                          "bots": {"count": 0, "threads": 1},
                          "simulation": {"world": {"preset": "battledome"}}}
                path = root / f"{label}-config.json"
                path.write_text(json.dumps(config) + "\n")
                log = root / f"{label}.jsonl"
                if log.exists():
                    log.replace(root / f"{label}-prior-{time.time_ns()}.jsonl")
                log.touch()
                host = subprocess.Popen(["taskset", "-c", "0", str(root / "bin/host"),
                                         "host", "--config", str(path), "--stats", "0.1"],
                                        stderr=subprocess.PIPE, stdout=subprocess.DEVNULL,
                                        text=True, start_new_session=True)
                reader = threading.Thread(target=read_output, args=(host, label), daemon=True)
                reader.start()
                deadline = time.monotonic() + 5
                while True:
                    assert host.poll() is None, "Host exited before listening"
                    if log.exists() and "Listening on " in log.read_text():
                        break
                    assert time.monotonic() < deadline, "Host listen timeout"
                    time.sleep(.01)
                event({"type": "start", "label": label, "unix": time.time(), "pid": host.pid})
                result = {"started": True}
            elif action == "mark":
                result = {"type": "mark", "label": request["label"], "unix": time.time()}
                event(result)
            elif action == "status":
                result = {"alive": host is not None and host.poll() is None, "unix": time.time()}
            elif action == "stop":
                stop()
                result = {"stopped": True, "unix": time.time()}
                event(result)
            elif action == "archive":
                assert host is None
                files = [p for p in root.iterdir() if p.suffix in [".json", ".jsonl"]]
                manifest = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
                (root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
                temporary = root / "server.tar.gz.tmp"
                with tarfile.open(temporary, "w:gz") as archive:
                    for path in files + [root / "manifest.json"]:
                        archive.add(path, arcname=path.name)
                with tarfile.open(temporary) as archive:
                    assert all(hashlib.sha256(archive.extractfile(name).read()).hexdigest() == digest
                               for name, digest in manifest.items())
                temporary.replace(root / "server.tar.gz")
                data = (root / "server.tar.gz").read_bytes()
                result = {"complete": True, "allMembersVerified": True,
                          "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
                (root / "server-ready.json").write_text(json.dumps(result) + "\n")
            elif action == "snapshot":
                label = request["label"]
                assert label in ["01-solo", "02-80", "03-160", "04-320"]
                directory = root / "snapshots" / label
                directory.mkdir(parents=True)
                for source in root.iterdir():
                    if source.suffix == ".jsonl" or source.name.endswith("-config.json"):
                        data = source.read_bytes()
                        if source.suffix == ".jsonl":
                            data = data[:data.rfind(b"\n") + 1]
                        (directory / source.name).write_bytes(data)
                files = list(directory.iterdir())
                manifest = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
                (directory / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
                temporary = root / f"snapshot-{label}.tar.gz.tmp"
                with tarfile.open(temporary, "w:gz") as archive:
                    for path in files + [directory / "manifest.json"]:
                        archive.add(path, arcname=path.name)
                with tarfile.open(temporary) as archive:
                    assert all(hashlib.sha256(archive.extractfile(name).read()).hexdigest() == digest
                               for name, digest in manifest.items())
                filename = f"snapshot-{label}.tar.gz"
                temporary.replace(root / filename)
                data = (root / filename).read_bytes()
                result = {"complete": True, "allMembersVerified": True, "file": filename,
                          "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
            else:
                raise ValueError("Unknown action")
            body = json.dumps(result).encode()
            self.send_response(200)
        except Exception as error:
            body = json.dumps({"error": str(error)}).encode()
            self.send_response(500)
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        name = self.path.removeprefix("/")
        allowed = (name == "server.tar.gz" and (root / "server-ready.json").exists()) or (
            name in [f"snapshot-{label}.tar.gz" for label in ["01-solo", "02-80", "03-160", "04-320"]]
            and (root / name).is_file())
        if not allowed:
            self.send_error(404)
            return
        self.send_response(200)
        self.end_headers()
        with (root / name).open("rb") as stream:
            while block := stream.read(1 << 20):
                self.wfile.write(block)


HTTPServer(("0.0.0.0", int(sys.argv[2])), Handler).serve_forever()
