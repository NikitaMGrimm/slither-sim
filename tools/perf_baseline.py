"""Bounded release benchmarks; standard library only, no builds or remote servers.

Pass copied, immutable executables, not a shared Cargo target. See docs/performance.md.
"""

import argparse
from contextlib import ExitStack
import ctypes
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
CASES = ("step-default", "step-food", "step-body", "observe", "portable", "protocol")
COMMAND_TIMEOUT_SECONDS = 30


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def command_output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True, timeout=COMMAND_TIMEOUT_SECONDS).strip()


def process_metrics(process):
    """Read the exited process handle before Popen closes it (Windows only)."""
    if os.name != "nt":
        return {"cpu_seconds": None, "peak_working_set_bytes": None,
                "unavailable": "per-process counters implemented only for Windows"}
    from ctypes import wintypes

    class MemoryCounters(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
            (name, ctypes.c_size_t) for name in (
                "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage",
                "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage", "QuotaNonPagedPoolUsage",
                "PagefileUsage", "PeakPagefileUsage")]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(MemoryCounters), wintypes.DWORD]
    times = [wintypes.FILETIME() for _ in range(4)]
    memory = MemoryCounters()
    memory.cb = ctypes.sizeof(memory)
    if not kernel.GetProcessTimes(int(process._handle), *(ctypes.byref(t) for t in times)):
        raise ctypes.WinError(ctypes.get_last_error())
    if not psapi.GetProcessMemoryInfo(int(process._handle), ctypes.byref(memory), memory.cb):
        raise ctypes.WinError(ctypes.get_last_error())
    seconds = lambda t: ((t.dwHighDateTime << 32) | t.dwLowDateTime) / 10_000_000
    return {"cpu_seconds": seconds(times[2]) + seconds(times[3]),
            "peak_working_set_bytes": memory.PeakWorkingSetSize}


class Lines:
    """Deadline-aware pipe reads without helper threads; includes polling overhead."""

    def __init__(self, pipe):
        self.pipe = pipe
        self.pending = b""

    def read(self, deadline):
        while b"\n" not in self.pending:
            if time.perf_counter() >= deadline:
                raise TimeoutError("child response deadline exceeded")
            if os.name == "nt":
                import msvcrt
                from ctypes import wintypes
                available = wintypes.DWORD()
                peek = ctypes.WinDLL("kernel32", use_last_error=True).PeekNamedPipe
                peek.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD,
                                 ctypes.c_void_p, ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
                ok = peek(msvcrt.get_osfhandle(self.pipe.fileno()), None, 0,
                          None, ctypes.byref(available), None)
                if not ok:
                    raise EOFError("child pipe closed")
                count = available.value
            else:
                import select
                count = 65536 if select.select([self.pipe], [], [], 0)[0] else 0
            if count:
                chunk = os.read(self.pipe.fileno(), min(count, 65536))
                if not chunk:
                    raise EOFError("child pipe closed")
                self.pending += chunk
            else:
                time.sleep(0.0005)
        line, self.pending = self.pending.split(b"\n", 1)
        return line

    def drain(self, output):
        """Preserve read-ahead bytes, then drain the pipe after the child stops."""
        output.write(self.pending)
        self.pending = b""
        output.write(self.pipe.read())


def stop(process):
    if process.poll() is None:
        process.kill()
        process.wait(timeout=10)


def close_child(process, reader=None, errors=None):
    if process is None:
        return
    with ExitStack() as pipes:
        for pipe in (process.stdin, process.stdout, process.stderr):
            if pipe is not None:
                pipes.callback(pipe.close)
        try:
            stop(process)
        finally:
            if reader is not None:
                if process.poll() is not None:
                    reader.drain(errors)
                else:
                    # A failed stop must not turn diagnostic draining into an unbounded read.
                    errors.write(reader.pending)


def percentiles(values):
    values = sorted(values)
    return {name: values[min(len(values) - 1, int((len(values) - 1) * q))]
            for name, q in (("p50", .5), ("p95", .95), ("p99", .99), ("max", 1))}


def native(executable, case, iterations, stderr_path, profile=False):
    args = [str(executable), case, str(iterations)] + (["--profile"] if profile else [])
    started = time.perf_counter()
    with ExitStack() as cleanup:
        errors = cleanup.enter_context(stderr_path.open("wb"))
        process = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=errors)
        cleanup.callback(close_child, process)
        line = Lines(process.stdout).read(started + 120)
        process.wait(timeout=max(.1, started + 120 - time.perf_counter()))
        wall = time.perf_counter() - started
        metrics = process_metrics(process)
        if process.returncode:
            raise RuntimeError(f"{case} exited {process.returncode}; see {stderr_path}")
        return {"case": case, "command": args, "process_wall_seconds": wall,
                **metrics, "exit_code": process.returncode, "stderr": str(stderr_path),
                "workload": json.loads(line)}


def capture_trace(executable, case, iterations, directory):
    stderr_path = directory / f"{case}-trace-stderr.txt"
    with ExitStack() as cleanup:
        errors = cleanup.enter_context(stderr_path.open("wb"))
        process = subprocess.Popen([str(executable), case, str(iterations), "--trace"],
                                   stdout=subprocess.PIPE, stderr=errors)
        cleanup.callback(close_child, process)
        reader = Lines(process.stdout)
        sha = hashlib.sha256()
        lines = 0
        size = 0
        deadline = time.perf_counter() + 120
        with (directory / f"{case}.jsonl.gz").open("wb") as raw:
            # Parity hashes uncompressed bytes; favor faster capture over archive size.
            with gzip.GzipFile(fileobj=raw, mode="wb", compresslevel=1, mtime=0) as output:
                while True:
                    try:
                        line = reader.read(deadline) + b"\n"
                    except EOFError:
                        break
                    output.write(line)
                    sha.update(line)
                    lines += 1
                    size += len(line)
        process.wait(timeout=max(.1, deadline - time.perf_counter()))
        if process.returncode or not lines or reader.pending:
            raise RuntimeError(f"invalid/incomplete {case} trace; see {stderr_path}")
        return {"case": case, "iterations": iterations, "lines": lines,
                "uncompressed_bytes": size, "sha256": sha.hexdigest()}


def e2e(executable, steps, hosted, directory):
    """Exercise the shipped JSON CLI, optionally through a loopback-only host."""
    host = None
    process = None
    host_error = None
    host_reader = None
    client_error = None
    started = time.perf_counter()
    deadline = started + 60
    trace = hashlib.sha256()
    try:
        if hosted:
            host_error = (directory / "host-stderr.txt").open("wb")
            config = directory / "host-config.json"
            config.write_text(json.dumps({"bind": "127.0.0.1:0", "seed": 7,
                                          "maxClients": 1, "bots": 4}), encoding="utf-8")
            host = subprocess.Popen([str(executable), "host", "--config", str(config)],
                                    stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            host_reader = Lines(host.stderr)
            announcement = host_reader.read(deadline).decode().strip()
            host_error.write((announcement + "\n").encode())
            if not announcement.startswith("Listening on ws://127.0.0.1:"):
                raise RuntimeError(announcement)
            request = {"op": "network", "config": {"url": announcement.split(" on ", 1)[1]}}
        else:
            request = {"op": "simulation", "seed": 7, "config": {"agents": 8}}
        client_error = (directory / "client-stderr.txt").open("wb")
        process = subprocess.Popen([str(executable), "client"], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=client_error, bufsize=0)
        reader = Lines(process.stdout)

        def exchange(message):
            data = json.dumps(message, separators=(",", ":")).encode() + b"\n"
            process.stdin.write(data)
            response = reader.read(deadline)
            trace.update(response + b"\n")
            value = json.loads(response)
            if "error" in value:
                raise RuntimeError(value["error"])
            if value["protocol"] != "slither-client-json-v1":
                raise RuntimeError("unexpected CLI protocol")
            return value["result"]

        exchange(request)
        latencies = []
        playing = 0
        previous = None
        for i in range(steps):
            tick = time.perf_counter()
            # Fixed commands and explicit processing time avoid bot wall-clock feedback.
            message = {"op": "step", "processingSeconds": 0.0}
            if previous and previous["status"] == "playing":
                head = previous["observation"]["self"]["head"]
                message["action"] = {"heading": math.atan2(head["y"], head["x"]) + math.pi / 2,
                                     "boost": i % 40 < 10}
            result = exchange(message)
            latencies.append(time.perf_counter() - tick)
            playing += result["frame"]["status"] == "playing"
            previous = result["frame"]
        process.stdin.close()
        process.wait(timeout=max(.1, deadline - time.perf_counter()))
        if process.returncode:
            raise RuntimeError(f"client exited {process.returncode}; see {directory / 'client-stderr.txt'}")
        result = {"case": "e2e-host" if hosted else "e2e-client", "steps": steps,
                  "playing_frames": playing, "wall_seconds": time.perf_counter() - started,
                  "response_latency_seconds": percentiles(latencies),
                  "response_sha256": trace.hexdigest(),
                  "client": {**process_metrics(process), "exit_code": process.returncode}}
        if host:
            if host.poll() is not None:
                raise RuntimeError(f"host exited prematurely with {host.returncode}; see {directory / 'host-stderr.txt'}")
            stop(host)
            result["host"] = {**process_metrics(host), "exit_code": host.returncode,
                              "shutdown": "terminated and waited by runner"}
            result["response_sha256"] = None  # Real-time receipts are intentionally nondeterministic.
        return result
    finally:
        try:
            close_child(process)
        finally:
            try:
                close_child(host, host_reader, host_error)
            finally:
                try:
                    if host_error:
                        host_error.close()
                finally:
                    if client_error:
                        client_error.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--harness", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="new owned output directory")
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--iterations", type=int, default=1000)
    parser.add_argument("--e2e-steps", type=int, default=100)
    parser.add_argument("--trace-iterations", type=int, default=32,
                        help="separate untimed full-state traces; 0 skips")
    parser.add_argument("--cases", nargs="+", choices=(*CASES, "e2e-client", "e2e-host"),
                        default=[*CASES, "e2e-client", "e2e-host"])
    args = parser.parse_args()
    if not (1 <= args.repeats <= 20 and 1 <= args.iterations <= 100000
            and 1 <= args.e2e_steps <= 1000 and 0 <= args.trace_iterations <= 1000):
        parser.error("repeats 1..20, iterations 1..100000, e2e-steps 1..1000, trace-iterations 0..1000 required")
    args.harness = args.harness.resolve(strict=True)
    args.cli = args.cli.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    try:
        run(args)
    except BaseException as error:
        (args.output / "failure.json").write_text(json.dumps({
            "type": type(error).__name__, "error": str(error),
            "diagnostics": "Child stderr and partial results remain in this directory.",
        }, indent=2) + "\n", encoding="utf-8")
        raise


def run(args):
    manifest = {
        "schema": "slither-perf-v1", "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source_commit": command_output("git", "rev-parse", "HEAD"),
        "source_status": command_output("git", "status", "--short"),
        "source_diff_sha256": hashlib.sha256(subprocess.check_output(
            ["git", "diff", "HEAD"], cwd=ROOT, timeout=COMMAND_TIMEOUT_SECONDS)).hexdigest(),
        "rustc": command_output("rustc", "-Vv"), "cargo": command_output("cargo", "-V"),
        "python": sys.version, "platform": platform.platform(), "cpu": platform.processor(),
        "logical_cpus": os.cpu_count(), "cargo_lock_sha256": digest(ROOT / "Cargo.lock"),
        "runner_sha256": digest(__file__), "harness_source_sha256": digest(ROOT / "examples/perf_baseline.rs"),
        "binaries": {str(p): digest(p) for p in (args.harness, args.cli)},
        "arguments": {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()},
        "metric_notes": ["Native workload wall/latency exclude setup; process CPU/wall/peak include setup, teardown and report serialization.",
                         "Windows CPU is kernel+user GetProcessTimes, quantized; peak memory is PeakWorkingSetSize, not private bytes.",
                         "E2E wall and response latency include Python JSON, pipe polling (0.5 ms sleeps), process startup and OS scheduling as applicable.",
                         "Host is real-time, one client plus four bots, stopped after each bounded loopback run; hosted hashes unavailable.",
                         "No CPU affinity or platform tuning. Quiet-window agreement and background load must be recorded separately."]}
    (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    if args.trace_iterations:
        traces = [capture_trace(args.harness, case, args.trace_iterations, args.output)
                  for case in args.cases if case in CASES]
        (args.output / "traces.json").write_text(json.dumps(traces, indent=2) + "\n", encoding="utf-8")
    profiles = [native(args.harness, case, args.iterations,
                       args.output / f"{case}-profile-stderr.txt", profile=True)
                for case in args.cases if case.startswith("step-")]
    (args.output / "profiles.json").write_text(json.dumps(profiles, indent=2) + "\n", encoding="utf-8")
    with (args.output / "results.jsonl").open("w", encoding="utf-8", newline="\n") as output:
        # One untallied warmup per case. Every repetition runs a fresh process.
        for repetition in range(-1, args.repeats):
            for case in args.cases:
                if case.startswith("e2e-"):
                    directory = args.output / f"{case}-{repetition}"
                    directory.mkdir()
                    row = e2e(args.cli, args.e2e_steps, case == "e2e-host", directory)
                else:
                    row = native(args.harness, case, args.iterations,
                                 args.output / f"{case}-{repetition}-stderr.txt")
                row["repetition"] = repetition
                output.write(json.dumps(row, separators=(",", ":")) + "\n")
                output.flush()
                print(json.dumps({"case": case, "repetition": repetition,
                                  "wall": row.get("process_wall_seconds", row.get("wall_seconds"))}), flush=True)
    if manifest["binaries"] != {str(p): digest(p) for p in (args.harness, args.cli)}:
        raise RuntimeError("benchmark executable changed during run")


if __name__ == "__main__":
    main()
