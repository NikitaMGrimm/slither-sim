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
HARNESS = ("examples/perf_baseline.rs", "examples/fidelity_metrics.rs", *(f"tests/{name}.rs" for name in TESTS))
FIXTURES = "tests/fixtures"
GIT_TIMEOUT_SECONDS = 120
SUFFIX = ".exe" if os.name == "nt" else ""
# Fixed metadata makes the snapshot commit a pure function of its tree and parent.
SNAPSHOT_IDENTITY = {name: value for role in ("AUTHOR", "COMMITTER") for name, value in (
    (f"GIT_{role}_NAME", "slither-perf"), (f"GIT_{role}_EMAIL", "slither-perf@localhost"),
    (f"GIT_{role}_DATE", "2000-01-01T00:00:00Z"))}


def output(args, cwd, environment=None, timeout=GIT_TIMEOUT_SECONDS):
    result = subprocess.run([str(arg) for arg in args], cwd=cwd, env=environment, capture_output=True,
                            encoding="utf-8", errors="replace", timeout=timeout, check=False)
    if result.returncode:
        raise RuntimeError(f"{' '.join(map(str, args))} exited {result.returncode}: {result.stderr.strip()}")
    return result.stdout.strip()


def stop_tree(process):
    """Kill the child and its descendants, then reap the child.

    subprocess.run's timeout kills only the immediate child; cargo's rustc and linker
    descendants would keep writing into a target whose lock is about to be released.
    """
    if process.poll() is None:
        if os.name == "nt":
            subprocess.run(["taskkill", "/F", "/T", "/PID", str(process.pid)],
                           capture_output=True, timeout=30, check=False)
        else:
            import signal
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    if process.poll() is None:
        process.kill()
    process.wait(timeout=30)


def command(args, cwd, log, environment=None, timeout=600):
    """Run a logged child; return its exit code, or -1 if it could not run or timed out."""
    with log.open("w", encoding="utf-8", newline="\n") as stream:
        try:
            # A separate POSIX session lets stop_tree signal every descendant at once.
            process = subprocess.Popen(args, cwd=cwd, env=environment, stdout=stream, stderr=subprocess.STDOUT,
                                       text=True, start_new_session=os.name != "nt")
        except OSError as error:
            stream.write(f"\n{type(error).__name__}: {error}\n")
            return -1
        try:
            return process.wait(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            stop_tree(process)
            stream.write(f"\n{type(error).__name__}: {error}\n")
            return -1
        except BaseException:
            stop_tree(process)
            raise


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


def resolve(root, ref):
    commit = output(["git", "rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}"], root)
    return commit, output(["git", "rev-parse", "--verify", f"{commit}^{{tree}}"], root)


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

    def __init__(self, root, work):
        self.root = Path(root)
        self.work = Path(work)
        self.paths = []

    def __enter__(self):
        self.work.mkdir(parents=True, exist_ok=False)
        return self

    def __exit__(self, *exception):
        for path in reversed(self.paths):
            try:
                output(["git", "worktree", "remove", "--force", path], self.root)
            except (RuntimeError, OSError, subprocess.TimeoutExpired) as error:
                print(f"warning: worktree {path} was not removed: {error}", file=sys.stderr)
        try:
            self.work.rmdir()
        except OSError:
            pass
        return False

    def add(self, label, commit):
        path = self.work / label
        output(["git", "worktree", "add", "--detach", path, commit], self.root)
        self.paths.append(path)
        return path

    def worktree(self, label, ref):
        commit, tree = resolve(self.root, ref)
        return Snapshot(label, self.add(label, commit), commit, tree, f"ref {ref}")

    def freeze(self, label):
        commit, tree = freeze(self.root)
        return Snapshot(label, self.add(label, commit), commit, tree, "working tree")


def overlay(source, destination):
    """Copy the source snapshot's measurement inputs into the destination checkout."""
    files = [*(source.checkout / name for name in HARNESS),
             *sorted((source.checkout / FIXTURES).rglob("*"))]
    files = [path for path in files if path.is_file()]
    inputs = {path.relative_to(source.checkout).as_posix(): digest(path) for path in files}
    changes = tuple(name for name, sha in inputs.items()
                    if not (destination.checkout / name).is_file() or digest(destination.checkout / name) != sha)
    for path in files:
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

    def before(self, snapshots, index, logs):
        return None


class SharedTarget:
    """CI: one run-owned target; dependencies are reused and the package is cleaned between labels."""

    policy = "shared run-owned target, package cleaned between labels"

    def __init__(self, path):
        self.target = Path(path)

    def path(self, label):
        return self.target

    def environment(self, label):
        return {**os.environ, "CARGO_TARGET_DIR": str(self.target), "CARGO_BUILD_JOBS": "2",
                "CARGO_TERM_COLOR": "never", "RUST_TEST_THREADS": "1"}

    def before(self, snapshots, index, logs):
        if index == 0:
            return None
        label = snapshots[index].label
        environment = self.environment(label)
        # Package-only clean defaults to dev; explicitly clear release too.
        # Earlier executables have already been copied out of this target.
        if any(command(["cargo", "clean", "--locked", "--package", "slither-sim", "--profile", profile],
                       snapshots[index - 1].checkout, logs / f"{label}-clean-{profile}.log", environment)
               for profile in ("release", "dev")):
            return f"{label} package cleanup failed; {label} measurements unavailable"
        return None


def build_all(snapshots, targets, examples, *, logs, bins, hook=None, timeout=600):
    """Build each snapshot's release examples and copy them out. Returns (builds, errors).

    `hook(snapshot, build_or_none, environment)` runs after each build attempt that got
    past target preparation, before the next label can reuse the target. Callers hold
    any target locks.
    """
    builds, errors = {}, []
    for index, snapshot in enumerate(snapshots):
        label = snapshot.label
        error = targets.before(snapshots, index, logs)
        if error:
            errors.append(error)
            continue
        target = targets.path(label)
        environment = targets.environment(label)
        built_names = [f"{name}{SUFFIX}" for name in examples]
        for name in built_names:
            (target / "release/examples" / name).unlink(missing_ok=True)
        arguments = ["cargo", "build", "--release", "--locked",
                     *(part for name in examples for part in ("--example", name))]
        print(f"[{label}] Building release measurement examples", flush=True)
        log = logs / f"{label}-build.log"
        code = command(arguments, snapshot.checkout, log, environment, timeout)
        build = None
        if code:
            errors.append(f"{label} release build exited {code}")
        else:
            directory = Path(bins) / f"bin-{label}"
            directory.mkdir(parents=True)
            for name in built_names:
                shutil.copy2(target / "release/examples" / name, directory / name)
            binaries = {name: directory / f"{name}{SUFFIX}" for name in examples}
            hashes = {path.name: digest(path) for path in binaries.values()}
            record = {"schema": "slither-perf-build-v1", "utc": utc(), **snapshot.record(),
                      "rustc": output(["rustc", "-Vv"], snapshot.checkout),
                      "cargo": output(["cargo", "-V"], snapshot.checkout),
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
