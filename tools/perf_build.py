"""Build source snapshots for performance comparisons; standard library only.

Sole owner of snapshot identity (Git commit and tree), detached worktrees, the
measurement-input overlay, Cargo target policy, copied executables and identity
records. It never samples performance. See docs/performance.md.
"""

from contextlib import ExitStack, contextmanager
from dataclasses import dataclass, field, replace
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

from perf_baseline import digest


TESTS = ("movement_turns", "physics")
# The candidate's measurement code and evidence, copied into the baseline so both
# runtimes run identical harness sources. The harness must compile against the base API.
HARNESS = ("build.rs", "examples/support/state.rs", "examples/support/runtime.rs",
           "examples/support/catalog_worlds.rs", "examples/support/catalog_backend.rs",
           "examples/support/catalog_spawn.bin", "examples/perf_baseline.rs",
           "examples/perf_observation.rs", "examples/perf_client_retained.rs", "examples/perf_session.rs", "examples/perf_radius.rs",
           "examples/fidelity_metrics.rs", "examples/host_load.rs", "tests/support/mod.rs", *(f"tests/{name}.rs" for name in TESTS))
FIXTURES = "tests/fixtures"
GIT_TIMEOUT_SECONDS = 120
SUFFIX = ".exe" if os.name == "nt" else ""
# Fixed metadata makes the snapshot commit a pure function of its tree and parent.
SNAPSHOT_IDENTITY = {name: value for role in ("AUTHOR", "COMMITTER") for name, value in (
    (f"GIT_{role}_NAME", "slither-perf"), (f"GIT_{role}_EMAIL", "slither-perf@localhost"),
    (f"GIT_{role}_DATE", "2000-01-01T00:00:00Z"))}


def remaining(timeout, deadline):
    if deadline is not None:
        timeout = min(timeout, deadline - time.perf_counter())
        if timeout <= 0:
            raise TimeoutError("performance preparation deadline exceeded before starting a command")
    return timeout


def output(args, cwd, environment=None, timeout=GIT_TIMEOUT_SECONDS, *, deadline=None):
    if deadline is not None:
        remaining(timeout, deadline)
        # Preparation (notably worktree checkout) can itself spawn descendants.
        # The shared cutoff must stop/reap those before finalization too.
        with tempfile.TemporaryFile() as stream, tempfile.TemporaryFile() as errors:
            tree = ProcessTree(args, cwd, environment, stream, stderr=errors)
            try:
                try:
                    code = tree.wait(remaining(timeout, deadline))
                finally:
                    tree.stop(verify=True)
            except (subprocess.TimeoutExpired, TimeoutError) as error:
                stream.seek(0)
                errors.seek(0)
                raise TimeoutError(f"preparation command timed out: {error}: "
                                   f"{stream.read().decode('utf-8', errors='replace').strip()}\n"
                                   f"{errors.read().decode('utf-8', errors='replace').strip()}") from error
            stream.seek(0)
            text = stream.read().decode("utf-8", errors="replace").strip()
            errors.seek(0)
            detail = errors.read().decode("utf-8", errors="replace").strip()
        if code:
            raise RuntimeError(f"{' '.join(map(str, args))} exited {code}: {text}\n{detail}")
        return text
    result = subprocess.run([str(arg) for arg in args], cwd=cwd, env=environment, capture_output=True,
                            encoding="utf-8", errors="replace", timeout=remaining(timeout, deadline), check=False)
    if result.returncode:
        raise RuntimeError(f"{' '.join(map(str, args))} exited {result.returncode}: {result.stderr.strip()}")
    return result.stdout.strip()


# Windows children start through this shim, which spawns the real command only after
# the parent has assigned it to the job object; nothing can escape the job before that.
# EOF (a parent that died before assigning) or any other input must not start it.
RELEASE = b"\n"
SHIM = ("import subprocess, sys\n"
        f"if sys.stdin.buffer.read(1) != {RELEASE!r}:\n"
        "    sys.exit('not released into the job object; command not started')\n"
        "sys.exit(subprocess.call(sys.argv[1:], stdin=subprocess.DEVNULL))\n")


class Job:
    """Windows job object that terminates every member, including orphaned descendants."""

    def __init__(self):
        import ctypes
        from ctypes import wintypes

        class Basic(ctypes.Structure):
            _fields_ = [("PerProcessUserTimeLimit", ctypes.c_int64), ("PerJobUserTimeLimit", ctypes.c_int64),
                        ("LimitFlags", wintypes.DWORD), ("MinimumWorkingSetSize", ctypes.c_size_t),
                        ("MaximumWorkingSetSize", ctypes.c_size_t), ("ActiveProcessLimit", wintypes.DWORD),
                        ("Affinity", ctypes.c_size_t), ("PriorityClass", wintypes.DWORD),
                        ("SchedulingClass", wintypes.DWORD)]

        class Extended(ctypes.Structure):
            _fields_ = [("BasicLimitInformation", Basic), ("IoInfo", ctypes.c_uint64 * 6),
                        ("ProcessMemoryLimit", ctypes.c_size_t), ("JobMemoryLimit", ctypes.c_size_t),
                        ("PeakProcessMemoryUsed", ctypes.c_size_t), ("PeakJobMemoryUsed", ctypes.c_size_t)]

        class Accounting(ctypes.Structure):
            _fields_ = [("Times", ctypes.c_int64 * 4), ("TotalPageFaultCount", wintypes.DWORD),
                        ("TotalProcesses", wintypes.DWORD), ("ActiveProcesses", wintypes.DWORD),
                        ("TotalTerminatedProcesses", wintypes.DWORD)]

        self.ctypes, self.accounting = ctypes, Accounting
        kernel = self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateJobObjectW.restype = wintypes.HANDLE
        kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
        for name in ("SetInformationJobObject", "QueryInformationJobObject"):
            getattr(kernel, name).argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD] + (
                [ctypes.c_void_p] if name.startswith("Query") else [])
        kernel.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
        kernel.TerminateJobObject.argtypes = [wintypes.HANDLE, ctypes.c_uint]
        kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        kernel.OpenProcess.restype = wintypes.HANDLE
        kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
        kernel.WaitForSingleObject.restype = wintypes.DWORD
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        self.handle = kernel.CreateJobObjectW(None, None)
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())
        limits = Extended()
        limits.BasicLimitInformation.LimitFlags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        self.check(kernel.SetInformationJobObject(self.handle, 9, ctypes.byref(limits), ctypes.sizeof(limits)))

    def check(self, ok):
        if not ok:
            error = self.ctypes.WinError(self.ctypes.get_last_error())
            self.close()
            raise error

    def assign(self, process):
        self.check(self.kernel.AssignProcessToJobObject(self.handle, int(process._handle)))

    def active(self, *, verify=False):
        info = self.accounting()
        ok = self.kernel.QueryInformationJobObject(self.handle, 1, self.ctypes.byref(info),
                                                   self.ctypes.sizeof(info), None)
        if not ok and verify:
            raise self.ctypes.WinError(self.ctypes.get_last_error())
        return info.ActiveProcesses if ok else 0

    def process_handles(self):
        """Hold termination witnesses before terminating members removes them from accounting."""
        ctypes = self.ctypes
        handles, capacity = [], 16
        try:
            while True:
                class ProcessIds(ctypes.Structure):
                    _fields_ = [("assigned", ctypes.c_uint32), ("count", ctypes.c_uint32),
                                ("pids", ctypes.c_size_t * capacity)]

                info = ProcessIds()
                if self.kernel.QueryInformationJobObject(self.handle, 3, ctypes.byref(info),
                                                         ctypes.sizeof(info), None):
                    break
                error = ctypes.get_last_error()
                if error != 234:  # ERROR_MORE_DATA: members were added during the query.
                    raise ctypes.WinError(error)
                capacity = max(capacity * 2, info.assigned)
            for pid in info.pids[:info.count]:
                handle = self.kernel.OpenProcess(0x100000, False, pid)  # SYNCHRONIZE
                if handle:
                    handles.append(handle)
                elif (error := ctypes.get_last_error()) != 87:  # ERROR_INVALID_PARAMETER: already gone.
                    raise ctypes.WinError(error)
            return handles
        except BaseException:
            for handle in handles:
                self.kernel.CloseHandle(handle)
            raise

    def terminate(self, wait_seconds=30, *, verify=False):
        deadline = time.perf_counter() + wait_seconds
        try:
            handles = self.process_handles()
        except OSError:
            # Best-effort stops must still terminate; the active() loop below
            # covers members whose handles could not be held.
            if verify:
                raise
            handles = []
        try:
            ok = self.kernel.TerminateJobObject(self.handle, 1)
            if not ok and verify:
                raise self.ctypes.WinError(self.ctypes.get_last_error())
            # ActiveProcesses can reach zero before members finish releasing file/cwd
            # handles. Wait for actual termination, including nested venv launchers.
            for handle in handles:
                status = self.kernel.WaitForSingleObject(handle, max(0, int((deadline - time.perf_counter()) * 1000)))
                if status == 0xFFFFFFFF and verify:  # WAIT_FAILED
                    raise self.ctypes.WinError(self.ctypes.get_last_error())
                if status != 0 and verify:
                    raise TimeoutError("Windows job member did not terminate")
            while self.active(verify=verify):
                if time.perf_counter() >= deadline:
                    if verify:
                        raise TimeoutError("Windows process job still has active descendants")
                    break
                time.sleep(0.01)
        finally:
            for handle in handles:
                self.kernel.CloseHandle(handle)

    def close(self):
        if self.handle:
            self.kernel.CloseHandle(self.handle)
            self.handle = None


class ProcessTree:
    """A child whose descendants never outlive it, whether it exits, times out or is interrupted.

    subprocess.run's timeout kills only the immediate child, and the child may also exit
    before cleanup; cargo's rustc and linker descendants would then keep writing into a
    target whose lock is about to be released.
    """

    def __init__(self, args, cwd, environment, stream, *, stderr=subprocess.STDOUT):
        args = [str(arg) for arg in args]
        if os.name == "nt":
            executable = shutil.which(args[0], path=(environment or os.environ).get("PATH"))
            if executable is None:
                raise FileNotFoundError(f"{args[0]} not found")
            self.job = Job()
            try:
                # A venv redirector can spawn its interpreter before assignment;
                # that existing child would escape this job and retain log/cwd handles.
                self.process = subprocess.Popen([sys._base_executable, "-I", "-c", SHIM, executable, *args[1:]],
                                                cwd=cwd, env=environment, stdin=subprocess.PIPE,
                                                stdout=stream, stderr=stderr)
            except BaseException:
                self.job.close()
                raise
            try:
                self.job.assign(self.process)
                self.process.stdin.write(RELEASE)
                self.process.stdin.close()
            except BaseException:
                self.process.kill()
                self.process.wait()
                self.job.close()
                raise
        else:
            self.job = None
            self.process = subprocess.Popen(args, cwd=cwd, env=environment, stdout=stream,
                                            stderr=stderr, start_new_session=True)
            self.group = self.process.pid

    def wait(self, timeout):
        return self.process.wait(timeout=timeout)

    def stop(self, *, verify=False):
        """Kill every remaining member regardless of the leader's state, and reap the leader."""
        if self.job:
            try:
                # Diagnostic uploads need proof of quiescence, not best-effort
                # cleanup: failed query/termination/timeout must prevent upload.
                self.job.terminate(verify=verify)
            finally:
                self.job.close()
                self.process.wait(timeout=30)
            return
        import signal
        try:
            os.killpg(self.group, signal.SIGKILL)
        except ProcessLookupError:
            pass
        # An unreaped leader stays a zombie member, keeping the group observable.
        self.process.wait(timeout=30)
        deadline = time.perf_counter() + 30
        while time.perf_counter() < deadline:
            try:
                os.killpg(self.group, 0)
            except (ProcessLookupError, PermissionError):
                break
            time.sleep(0.01)
        else:
            if verify:
                raise TimeoutError("Process group still has descendants after cleanup")


def command(args, cwd, log, environment=None, timeout=600, *, deadline=None):
    """Run a logged child; return its exit code, or -1 if it could not run or timed out.

    No descendant outlives the call on any path.
    An optional shared deadline forbids starting after expiry, bounds the wait
    after spawn, and requires verified process-tree cleanup. Pre-start expiry
    raises TimeoutError without creating a log or child.
    """
    remaining(timeout, deadline)
    with log.open("w", encoding="utf-8", newline="\n") as stream:
        try:
            tree = ProcessTree(args, cwd, environment, stream)
        except OSError as error:
            stream.write(f"\n{type(error).__name__}: {error}\n")
            return -1
        try:
            try:
                return tree.wait(remaining(timeout, deadline))
            finally:
                if deadline is None:
                    tree.stop()
                else:
                    tree.stop(verify=True)
        except subprocess.TimeoutExpired as error:
            stream.write(f"\n{type(error).__name__}: {error}\n")
            return -1


@dataclass(frozen=True)
class Snapshot:
    label: str
    checkout: Path
    commit: str
    tree: str
    origin: str
    inputs: dict = field(default_factory=dict)
    suite_changes: tuple = ()

    def record(self):
        return {"label": self.label, "checkout": str(self.checkout), "commit": self.commit,
                "tree": self.tree, "origin": self.origin, "overlay_inputs": self.inputs,
                "suite_changes": list(self.suite_changes)}


@dataclass(frozen=True)
class Build:
    label: str
    snapshot: Snapshot
    binaries: dict
    hashes: dict
    record: dict

    def verify(self):
        current = {path.name: digest(path) for path in self.binaries.values()}
        if current != self.hashes:
            raise RuntimeError(f"{self.label} executables changed after the build record")


def resolve(root, ref, *, deadline=None):
    budget = {"deadline": deadline} if deadline is not None else {}
    commit = output(["git", "rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}"], root, **budget)
    return commit, output(["git", "rev-parse", "--verify", f"{commit}^{{tree}}"], root, **budget)


def checkout_snapshot(root, label):
    """An existing checkout measured in place (CI's clean merge checkout)."""
    commit, tree = resolve(root, "HEAD")
    return Snapshot(label, Path(root), commit, tree, "checkout")


def freeze(root):
    """Return (commit, tree) for the working tree: tracked edits, deletions and untracked
    non-ignored files. A temporary index leaves the user's index, HEAD, stash and files alone."""
    head = output(["git", "rev-parse", "--verify", "HEAD^{commit}"], root)
    with tempfile.TemporaryDirectory(prefix="slither-perf-index-") as directory:
        environment = {**os.environ, "GIT_INDEX_FILE": str(Path(directory) / "index")}
        output(["git", "read-tree", head], root, environment)
        output(["git", "add", "-A"], root, environment)
        tree = output(["git", "write-tree"], root, environment)
    commit = output(["git", "commit-tree", tree, "-p", head, "-m", "slither-perf working-tree snapshot"],
                    root, {**os.environ, **SNAPSHOT_IDENTITY})
    return commit, tree


def require_outside_snapshot(root, path):
    """`git add -A` would capture run outputs placed in a non-ignored part of the checkout."""
    try:
        relative = Path(path).resolve().relative_to(Path(root).resolve())
    except ValueError:
        return
    result = subprocess.run(["git", "check-ignore", "-q", "--", relative.as_posix()], cwd=root,
                            capture_output=True, timeout=GIT_TIMEOUT_SECONDS, check=False)
    if result.returncode:
        raise ValueError(f"output {path} is inside the checkout but not ignored by Git; "
                         "use outputs/... or a directory outside the checkout")


class Session:
    """Detached worktrees under `work`, removed on exit. Logs and binaries live elsewhere."""

    def __init__(self, root, work, *, deadline=None):
        self.root = Path(root)
        self.work = Path(work)
        self.paths = []
        self.budget = {"deadline": deadline} if deadline is not None else {}

    def __enter__(self):
        self.work.mkdir(parents=True, exist_ok=False)
        return self

    def __exit__(self, *exception):
        for path in reversed(self.paths):
            try:
                # Cleanup may use the finalization reserve, but never three
                # independent 120-second Git waits after a preparation cutoff.
                output(["git", "worktree", "remove", "--force", path], self.root,
                       **({"timeout": 5} if self.budget else {}))
            except (RuntimeError, OSError, subprocess.TimeoutExpired) as error:
                print(f"warning: worktree {path} was not removed: {error}", file=sys.stderr)
        try:
            self.work.rmdir()
        except OSError:
            pass
        return False

    def add(self, label, commit):
        path = self.work / label
        self.paths.append(path)
        output(["git", "worktree", "add", "--detach", path, commit], self.root, **self.budget)
        return path

    def worktree(self, label, ref):
        commit, tree = resolve(self.root, ref, **self.budget)
        return Snapshot(label, self.add(label, commit), commit, tree, f"ref {ref}")

    def freeze(self, label):
        commit, tree = freeze(self.root)
        return Snapshot(label, self.add(label, commit), commit, tree, "working tree")


def overlay(source, destination, *, deadline=None):
    """Copy the source snapshot's measurement inputs into the destination checkout."""
    remaining(GIT_TIMEOUT_SECONDS, deadline)
    files = [*(source.checkout / name for name in HARNESS),
             *sorted((source.checkout / FIXTURES).rglob("*"))]
    files = [path for path in files if path.is_file()]
    inputs = {path.relative_to(source.checkout).as_posix(): digest(path) for path in files}
    changes = tuple(name for name, sha in inputs.items()
                    if not (destination.checkout / name).is_file() or digest(destination.checkout / name) != sha)
    for path in files:
        remaining(GIT_TIMEOUT_SECONDS, deadline)
        target = destination.checkout / path.relative_to(source.checkout)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
    return replace(destination, inputs=inputs, suite_changes=changes)


@contextmanager
def exclusive(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
    except FileExistsError:
        raise RuntimeError(f"{path} exists: another performance build is using this target. "
                           "Delete it only after confirming that no run is active.") from None
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            stream.write(json.dumps({"pid": os.getpid(), "utc": utc()}) + "\n")
        yield
    finally:
        path.unlink(missing_ok=True)


class LocalTargets:
    """Persistent per-label targets reuse dependency builds across local runs.

    Fresh worktree files are newer than earlier build outputs, so Cargo rebuilds the
    package; stale executables are deleted before every build in any case.
    """

    policy = "local per-label persistent target"

    def __init__(self, root):
        base = Path(os.environ.get("CARGO_TARGET_DIR") or Path(root) / "target")
        self.base = (base if base.is_absolute() else Path(root) / base) / "perf-suite"

    def path(self, label):
        return self.base / label

    def environment(self, label):
        return {**os.environ, "CARGO_TARGET_DIR": str(self.path(label)), "CARGO_TERM_COLOR": "never"}

    @contextmanager
    def hold(self, labels):
        with ExitStack() as locks:
            for label in labels:
                locks.enter_context(exclusive(self.base / f"{label}.lock"))
            yield

    def before(self, snapshots, index, logs, *, deadline=None):
        return None


class SharedTarget:
    """CI: one target, possibly restored from a dependency cache. Dependencies are
    reused; the package is cleaned before each label builds."""

    policy = "shared target, package cleaned before each label"

    def __init__(self, path):
        self.target = Path(path)

    def path(self, label):
        return self.target

    def environment(self, label):
        # CARGO_BUILD_JOBS comes from the workflow, which knows the runner's cores.
        return {**os.environ, "CARGO_TARGET_DIR": str(self.target),
                "CARGO_TERM_COLOR": "never", "RUST_TEST_THREADS": "1"}

    def before(self, snapshots, index, logs, *, deadline=None):
        label = snapshots[index].label
        environment = self.environment(label)
        budget = {"deadline": deadline} if deadline is not None else {}
        # Package-only clean defaults to dev; explicitly clear release too. A restored
        # cache holds no package outputs, but the first label must not trust that.
        # Earlier executables have already been copied out of this target.
        if any(command(["cargo", "clean", "--locked", "--package", "slither-sim", "--profile", profile],
                       snapshots[max(index - 1, 0)].checkout, logs / f"{label}-clean-{profile}.log", environment, **budget)
               for profile in ("release", "dev")):
            return f"{label} package cleanup failed; {label} measurements unavailable"
        return None


def build_all(snapshots, targets, examples, *, logs, bins, hook=None, timeout=600, cargo_args=(), deadline=None):
    """Build each snapshot's release examples and copy them out. Returns (builds, errors).

    `hook(snapshot, build_or_none, environment)` runs after each build attempt that got
    past target preparation, before the next label can reuse the target. Callers hold
    any target locks.
    """
    builds, errors = {}, []
    budget = {"deadline": deadline} if deadline is not None else {}
    for index, snapshot in enumerate(snapshots):
        remaining(timeout, deadline)
        label = snapshot.label
        error = targets.before(snapshots, index, logs, **budget)
        if error:
            errors.append(error)
            continue
        target = targets.path(label)
        environment = targets.environment(label)
        built_names = [f"{name}{SUFFIX}" for name in examples]
        for name in built_names:
            (target / "release/examples" / name).unlink(missing_ok=True)
        arguments = ["cargo", "build", "--release", "--locked", *cargo_args,
                     *(part for name in examples for part in ("--example", name))]
        print(f"[{label}] Building release measurement examples", flush=True)
        log = logs / f"{label}-build.log"
        code = command(arguments, snapshot.checkout, log, environment, timeout, **budget)
        build = None
        if code:
            errors.append(f"{label} release build exited {code}")
        else:
            directory = Path(bins) / f"bin-{label}"
            directory.mkdir(parents=True)
            for name in built_names:
                remaining(timeout, deadline)
                shutil.copy2(target / "release/examples" / name, directory / name)
            binaries = {name: directory / f"{name}{SUFFIX}" for name in examples}
            hashes = {path.name: digest(path) for path in binaries.values()}
            record = {"schema": "slither-perf-build-v1", "utc": utc(), **snapshot.record(),
                      "runtime_adapter": "private-state" if (snapshot.checkout / "src/simulator/initialization.rs").is_file() else "public-state",
                      "state_adapter": ("checkpoint-v2" if 'slither-sim-checkpoint-v2' in (snapshot.checkout / "src/simulator/checkpoint.rs").read_text(encoding="utf-8") else "checkpoint-v1") if (snapshot.checkout / "src/simulator/checkpoint.rs").is_file() else "legacy-serde",
                      "radius_adapter": "runtime-state" if (snapshot.checkout / "src/simulator.rs").is_file() and "\n    radius: f64," in (snapshot.checkout / "src/simulator.rs").read_text(encoding="utf-8").split("pub struct Simulator {", 1)[-1].split("}", 1)[0] else "config-live",
                      "state_adapter_sources": {name: digest(snapshot.checkout / name) for name in ("build.rs", "examples/support/state.rs", "examples/support/runtime.rs") if (snapshot.checkout / name).is_file()},
                      "runtime_sources": {path.relative_to(snapshot.checkout).as_posix(): digest(path) for path in sorted((snapshot.checkout / "src").rglob("*.rs"))},
                      "rustc": output(["rustc", "-Vv"], snapshot.checkout, **budget),
                      "cargo": output(["cargo", "-V"], snapshot.checkout, **budget),
                      "cargo_lock_sha256": digest(snapshot.checkout / "Cargo.lock"),
                      "command": arguments, "target": str(target), "target_policy": targets.policy,
                      "log": str(log), "log_sha256": digest(log),
                      "binaries": {path.name: {"path": str(path), "sha256": hashes[path.name]}
                                   for path in binaries.values()}}
            (logs / f"{label}-identity.json").write_text(json.dumps(record, indent=2) + "\n",
                                                         encoding="utf-8", newline="\n")
            build = Build(label, snapshot, binaries, hashes, record)
            builds[label] = build
        if hook:
            remaining(timeout, deadline)
            hook(snapshot, build, environment)
    return builds, errors


def prepare_local(root, ref, out, *, examples=("perf_baseline",), timeout=1800):
    """Build `ref` (with the candidate's measurement inputs) and the current working tree."""
    targets = LocalTargets(root)
    with targets.hold(("baseline", "candidate")), Session(root, Path(out) / "work") as session:
        candidate = session.freeze("candidate")
        baseline = overlay(candidate, session.worktree("baseline", ref))
        builds, errors = build_all([baseline, candidate], targets, examples,
                                   logs=Path(out), bins=Path(out), timeout=timeout)
    if errors:
        raise RuntimeError("; ".join(errors) + f"; see build logs in {out}")
    return builds


def utc():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
