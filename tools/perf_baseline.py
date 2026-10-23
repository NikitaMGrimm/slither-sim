"""Bounded release benchmarks; standard library only, no builds or remote servers.

Pass copied, immutable executables, not a shared Cargo target. See docs/performance.md.
Workloads, default iterations and trace windows come from `perf_baseline --list`.
"""

import argparse
from contextlib import ExitStack
import ctypes
from dataclasses import dataclass
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
E2E_CASES = ("e2e-client", "e2e-host")
CATALOG_FORMAT = "slither-perf-catalog-v1"
SUITES = ("ci", "full")
NAME = re.compile(r"[a-z0-9]+(-[a-z0-9]+)*")
COMMAND_TIMEOUT_SECONDS = 30
# Pin the historical compact workload independently of future gameplay defaults.
SANDBOX_WORLD = {"radius": 1000, "dynamicRadius": False, "foodCount": 350,
                 "ambientReplenishPerSecond": 5, "prey": {"count": 0}}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def canonical_json(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


@dataclass(frozen=True)
class Workload:
    name: str
    suite: str
    mode: str
    operation: str
    iterations: int
    trace_iterations: int
    profile: bool
    available: bool
    legacy: bool
    synthetic: str | None
    definition: str  # Canonical JSON text keeps the record immutable and hashable.
    fingerprint: str

    def plan(self):
        return {"name": self.name, "iterations": self.iterations,
                "traceIterations": self.trace_iterations}


@dataclass(frozen=True)
class Catalog:
    networking: bool
    seed: int
    workloads: tuple[Workload, ...]
    sha256: str
    source: str

    def select(self, cases=None, suite="full"):
        """Suite selections keep catalog order and skip unavailable rows; named cases keep
        request order and must exist and be available."""
        if cases is None:
            return tuple(w for w in self.workloads
                         if w.available and (suite == "full" or w.suite == suite))
        known = {w.name: w for w in self.workloads}
        unknown = [name for name in cases if name not in known]
        if unknown:
            raise ValueError(f"unknown workload(s) {', '.join(unknown)}; "
                             f"`{self.source} --list` prints the catalog")
        if len(set(cases)) != len(cases):
            raise ValueError("duplicate workload names requested")
        unavailable = [name for name in cases if not known[name].available]
        if unavailable:
            raise ValueError(f"workload(s) {', '.join(unavailable)} unavailable in this build "
                             "(networking feature disabled)")
        return tuple(known[name] for name in cases)

    def record(self):
        return {"format": CATALOG_FORMAT, "sha256": self.sha256, "seed": self.seed,
                "networking": self.networking,
                "workloads": {w.name: {"suite": w.suite, "mode": w.mode, "operation": w.operation,
                                       "iterations": w.iterations,
                                       "traceIterations": w.trace_iterations,
                                       "profile": w.profile, "available": w.available,
                                       "legacy": w.legacy, "synthetic": w.synthetic,
                                       "definitionSha256": w.fingerprint}
                              for w in self.workloads}}


def parse_catalog(line, source="perf_baseline"):
    """Validate the harness `--list` line at the boundary; later code trusts the records."""
    def fail(message):
        raise ValueError(f"invalid workload catalog from {source}: {message}")

    def integer(value, low, high):
        return isinstance(value, int) and not isinstance(value, bool) and low <= value <= high

    raw = (line if isinstance(line, bytes) else line.encode()).strip()
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"not JSON ({error})")
    if not isinstance(value, dict) or value.get("format") != CATALOG_FORMAT or value.get("kind") != "catalog":
        fail(f"expected kind catalog and format {CATALOG_FORMAT}")
    if not isinstance(value.get("networking"), bool) or not integer(value.get("seed"), 0, 2**32 - 1):
        fail("networking must be boolean and seed a u32")
    rows = value.get("workloads")
    if not isinstance(rows, list) or not rows:
        fail("workloads must be a non-empty list")
    workloads, names = [], set()
    for row in rows:
        if not isinstance(row, dict):
            fail("workload rows must be objects")
        name = row.get("name")
        if not isinstance(name, str) or not NAME.fullmatch(name) or name in names:
            fail(f"workload name {name!r} must be unique and lowercase-hyphenated")
        names.add(name)
        if row.get("suite") not in SUITES:
            fail(f"{name}: suite must be one of {', '.join(SUITES)}")
        if not all(isinstance(row.get(key), str) and row[key] for key in ("mode", "operation")):
            fail(f"{name}: mode and operation must be non-empty strings")
        if not integer(row.get("iterations"), 1, 100000) or not integer(row.get("traceIterations"), 1, 1000):
            fail(f"{name}: iterations 1..100000 and traceIterations 1..1000 required")
        if not all(isinstance(row.get(key), bool) for key in ("profile", "available", "legacy")):
            fail(f"{name}: profile, available and legacy must be boolean")
        synthetic = row.get("synthetic")
        if synthetic is not None and not (isinstance(synthetic, str) and synthetic):
            fail(f"{name}: synthetic must be null or a non-empty label")
        if not isinstance(row.get("definition"), dict):
            fail(f"{name}: definition must be an object")
        definition = canonical_json(row["definition"])
        workloads.append(Workload(
            name=name, suite=row["suite"], mode=row["mode"], operation=row["operation"],
            iterations=row["iterations"], trace_iterations=row["traceIterations"],
            profile=row["profile"], available=row["available"], legacy=row["legacy"],
            synthetic=synthetic, definition=definition,
            fingerprint=hashlib.sha256(definition.encode()).hexdigest()))
    return Catalog(networking=value["networking"], seed=value["seed"], workloads=tuple(workloads),
                   sha256=hashlib.sha256(raw).hexdigest(), source=source)


def load_catalog(executable, *, deadline=None):
    timeout = COMMAND_TIMEOUT_SECONDS
    if deadline is not None:
        timeout = min(timeout, deadline - time.perf_counter())
        if timeout <= 0:
            raise TimeoutError("performance run deadline exceeded before catalog command")
    # subprocess.run kills and reaps the child when the timeout expires.
    result = subprocess.run([str(executable), "--list"], stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=timeout, check=False)
    lines = result.stdout.splitlines()
    if result.returncode or len(lines) != 1:
        detail = result.stderr.decode(errors="replace").strip()[-500:]
        raise RuntimeError(f"`{executable} --list` exited {result.returncode} with {len(lines)} "
                           f"stdout lines; expected one catalog line. {detail}".rstrip())
    return parse_catalog(lines[0], str(executable))


def same_plan(selections):
    """Return the shared plan, or raise unless every label selects equal names, iterations and windows."""
    plans = {label: [w.plan() for w in selected] for label, selected in selections.items()}
    first, *others = plans
    for label in others:
        if plans[label] != plans[first]:
            raise ValueError(f"{first} and {label} workload plans differ: "
                             f"{plans[first]} != {plans[label]}")
    return plans[first]


def command_output(*args, deadline=None):
    timeout = COMMAND_TIMEOUT_SECONDS
    if deadline is not None:
        timeout = min(timeout, deadline - time.perf_counter())
        if timeout <= 0:
            raise TimeoutError("performance run deadline exceeded before metadata command")
    return subprocess.check_output(args, cwd=ROOT, text=True, timeout=timeout).strip()


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

    def __init__(self, pipe, spin_seconds=0.0):
        self.pipe = pipe
        self.pending = b""
        # Untimed bulk reads yield instead of sleeping for a short idle interval:
        # Windows pipes hold a few KiB, so a sleep per chunk made a 210 MB trace
        # take 27 s instead of 5 s. Timed callers keep the plain 0.5 ms sleep.
        self.spin_seconds = spin_seconds

    def available(self):
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
            return available.value
        import select
        return 65536 if select.select([self.pipe], [], [], 0)[0] else 0

    def read(self, deadline):
        # Join chunks once per line and scan only new bytes: growing one bytes object
        # is quadratic for the multi-megabyte lines of large scenario traces.
        chunks, size = [self.pending], len(self.pending)
        index = self.pending.find(b"\n")
        idle_since = None
        try:
            while index < 0:
                if time.perf_counter() >= deadline:
                    raise TimeoutError("child response deadline exceeded")
                count = self.available()
                if count:
                    chunk = os.read(self.pipe.fileno(), min(count, 65536))
                    if not chunk:
                        raise EOFError("child pipe closed")
                    found = chunk.find(b"\n")
                    if found >= 0:
                        index = size + found
                    chunks.append(chunk)
                    size += len(chunk)
                    idle_since = None
                else:
                    now = time.perf_counter()
                    idle_since = now if idle_since is None else idle_since
                    time.sleep(0 if now - idle_since < self.spin_seconds else 0.0005)
        finally:
            if len(chunks) > 1:
                self.pending = b"".join(chunks)
        line, self.pending = self.pending[:index], self.pending[index + 1:]
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


def native(executable, case, iterations, stderr_path, profile=False, *, deadline=None):
    args = [str(executable), case, str(iterations)] + (["--profile"] if profile else [])
    started = time.perf_counter()
    child_deadline = min(started + 120, deadline) if deadline is not None else started + 120
    if child_deadline <= started:
        raise TimeoutError("performance run deadline exceeded before native child")
    with ExitStack() as cleanup:
        errors = cleanup.enter_context(stderr_path.open("wb"))
        process = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=errors)
        cleanup.callback(close_child, process)
        line = Lines(process.stdout).read(child_deadline)
        process.wait(timeout=max(.1 if deadline is None else 0.0,
                                 child_deadline - time.perf_counter()))
        if deadline is not None and time.perf_counter() >= deadline:
            raise TimeoutError("performance run deadline exceeded during native child")
        wall = time.perf_counter() - started
        metrics = process_metrics(process)
        if process.returncode:
            raise RuntimeError(f"{case} exited {process.returncode}; see {stderr_path}")
        return {"case": case, "command": args, "process_wall_seconds": wall,
                **metrics, "exit_code": process.returncode, "stderr": str(stderr_path),
                "workload": json.loads(line)}


class TraceSequence:
    """Streaming check of a scenario trace's record order, one parsed line at a time.

    initial, warmup*, warmed, then operations numbered 0..N-1 (epoch records may
    appear between them), end (operations == N), and the trace-mode report last.
    Without this, two identically truncated streams could hash Equal.
    """

    OPERATIONS = ("step", "bot", "decision")

    def __init__(self, case, iterations):
        self.case = case
        self.iterations = iterations
        self.state = "initial"
        self.operations = 0
        self.records = 0
        self.end = None
        self.error = None

    def fail(self, message):
        self.error = f"record {self.records}: {message}"

    def feed(self, line):
        self.records += 1
        if self.error:
            return
        try:
            record = json.loads(line)
        except (UnicodeDecodeError, json.JSONDecodeError):
            return self.fail("not JSON")
        kind = record.get("kind") if isinstance(record, dict) else None
        if self.state == "initial":
            if kind != "initial":
                return self.fail(f"expected initial, found {kind!r}")
            self.state = "warmup"
        elif self.state == "warmup":
            if kind == "warmed":
                self.state = "operations"
            elif kind != "warmup":
                return self.fail(f"expected warmup or warmed, found {kind!r}")
        elif self.state == "operations":
            if kind in self.OPERATIONS:
                if record.get("iteration") != self.operations or self.operations >= self.iterations:
                    return self.fail(f"expected operation {self.operations} of {self.iterations}, "
                                     f"found iteration {record.get('iteration')!r}")
                self.operations += 1
            elif kind == "end":
                if (self.operations != self.iterations or record.get("case") != self.case
                        or record.get("operations") != self.iterations):
                    return self.fail(f"end after {self.operations} of {self.iterations} operations "
                                     f"(case {record.get('case')!r}, operations {record.get('operations')!r})")
                self.end = record
                self.state = "report"
            elif kind != "epoch":
                return self.fail(f"unexpected {kind!r} among operations")
        elif self.state == "report":
            if (kind != "report" or record.get("case") != self.case
                    or record.get("iterations") != self.iterations or record.get("mode") != "trace"):
                return self.fail("expected the trace-mode report for this case and window")
            self.state = "done"
        else:
            return self.fail("record after the report")

    def finish(self):
        if not self.error and self.state != "done":
            self.error = f"stream ended while expecting {self.state}"
        return self.error


def capture_trace(executable, case, iterations, directory, *, terminal=False, deadline=None):
    """Archive one trace. With `terminal`, the whole scenario record sequence must validate."""
    stderr_path = directory / f"{case}-trace-stderr.txt"
    started = time.perf_counter()
    child_deadline = min(started + 120, deadline) if deadline is not None else started + 120
    if child_deadline <= started:
        raise TimeoutError("performance run deadline exceeded before trace child")
    with ExitStack() as cleanup:
        errors = cleanup.enter_context(stderr_path.open("wb"))
        process = subprocess.Popen([str(executable), case, str(iterations), "--trace"],
                                   stdout=subprocess.PIPE, stderr=errors)
        cleanup.callback(close_child, process)
        reader = Lines(process.stdout, spin_seconds=0.005)
        sha = hashlib.sha256()
        lines = 0
        size = 0
        sequence = TraceSequence(case, iterations) if terminal else None
        with (directory / f"{case}.jsonl.gz").open("wb") as raw:
            # Parity hashes uncompressed bytes; favor faster capture over archive size.
            with gzip.GzipFile(fileobj=raw, mode="wb", compresslevel=1, mtime=0) as output:
                while True:
                    try:
                        line = reader.read(child_deadline)
                    except EOFError:
                        break
                    if sequence:
                        sequence.feed(line)
                    for part in (line, b"\n"):
                        output.write(part)
                        sha.update(part)
                    lines += 1
                    size += len(line) + 1
        process.wait(timeout=max(.1, child_deadline - time.perf_counter()))
        error = sequence.finish() if sequence else None
        if process.returncode or not lines or reader.pending or error:
            detail = f"; {error}" if error else ""
            raise RuntimeError(f"invalid/incomplete {case} trace{detail}; see {stderr_path}")
        record = {"case": case, "iterations": iterations, "lines": lines,
                  "uncompressed_bytes": size, "sha256": sha.hexdigest()}
        return {**record, "end": sequence.end} if terminal else record


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
                                          "maxClients": 1, "bots": 4,
                                          "simulation": {"world": SANDBOX_WORLD}}), encoding="utf-8")
            host = subprocess.Popen([str(executable), "host", "--config", str(config)],
                                    stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            host_reader = Lines(host.stderr)
            announcement = host_reader.read(deadline).decode().strip()
            host_error.write((announcement + "\n").encode())
            if not announcement.startswith("Listening on ws://127.0.0.1:"):
                raise RuntimeError(announcement)
            request = {"op": "network", "config": {"url": announcement.split(" on ", 1)[1]}}
        else:
            request = {"op": "simulation", "seed": 7,
                       "config": {"agents": 8, "world": SANDBOX_WORLD}}
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
    parser.add_argument("--iterations", type=int,
                        help="override every native workload's catalog sample size")
    parser.add_argument("--e2e-steps", type=int, default=100)
    parser.add_argument("--trace-iterations", type=int,
                        help="override every catalog trace window; 0 skips traces")
    parser.add_argument("--cases", nargs="+", metavar="CASE",
                        help=f"catalog names from `perf_baseline --list` and/or {', '.join(E2E_CASES)}; "
                             "default: every available catalog workload plus both E2E cases")
    args = parser.parse_args()
    if not (1 <= args.repeats <= 20 and 1 <= args.e2e_steps <= 1000
            and (args.iterations is None or 1 <= args.iterations <= 100000)
            and (args.trace_iterations is None or 0 <= args.trace_iterations <= 1000)):
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
    catalog = load_catalog(args.harness)
    if args.cases is None:
        workloads, e2e_cases = catalog.select(), list(E2E_CASES)
    else:
        e2e_cases = [case for case in args.cases if case in E2E_CASES]
        workloads = catalog.select([case for case in args.cases if case not in E2E_CASES])
    iterations = {w.name: args.iterations or w.iterations for w in workloads}
    windows = {w.name: w.trace_iterations if args.trace_iterations is None else args.trace_iterations
               for w in workloads}
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
        "catalog": catalog.record(),
        "plan": {"workloads": [{"name": w.name, "iterations": iterations[w.name],
                                "traceIterations": windows[w.name], "profile": w.profile}
                               for w in workloads], "e2e": e2e_cases},
        "metric_notes": ["Native workload wall/latency exclude setup; process CPU/wall/peak include setup, teardown and report serialization.",
                         "Windows CPU is kernel+user GetProcessTimes, quantized; peak memory is PeakWorkingSetSize, not private bytes.",
                         "E2E wall and response latency include Python JSON, pipe polling (0.5 ms sleeps), process startup and OS scheduling as applicable.",
                         "Host is real-time, one client plus four bots, stopped after each bounded loopback run; hosted hashes unavailable.",
                         "No CPU affinity or platform tuning. Quiet-window agreement and background load must be recorded separately."]}
    (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    traces = [capture_trace(args.harness, w.name, windows[w.name], args.output, terminal=not w.legacy)
              for w in workloads if windows[w.name]]
    if traces:
        (args.output / "traces.json").write_text(json.dumps(traces, indent=2) + "\n", encoding="utf-8")
    profiles = [native(args.harness, w.name, iterations[w.name],
                       args.output / f"{w.name}-profile-stderr.txt", profile=True)
                for w in workloads if w.profile]
    (args.output / "profiles.json").write_text(json.dumps(profiles, indent=2) + "\n", encoding="utf-8")
    cases = [w.name for w in workloads] + e2e_cases if args.cases is None else args.cases
    with (args.output / "results.jsonl").open("w", encoding="utf-8", newline="\n") as output:
        # One untallied warmup per case. Every repetition runs a fresh process.
        for repetition in range(-1, args.repeats):
            for case in cases:
                if case in E2E_CASES:
                    directory = args.output / f"{case}-{repetition}"
                    directory.mkdir()
                    row = e2e(args.cli, args.e2e_steps, case == "e2e-host", directory)
                else:
                    row = native(args.harness, case, iterations[case],
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
