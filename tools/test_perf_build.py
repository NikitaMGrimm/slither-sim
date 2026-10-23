"""Snapshot identity, worktree, overlay and target-policy checks; temporary Git repositories, no Cargo."""

import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import perf_build


def git(root, *args, **kwargs):
    return subprocess.run(["git", *args], cwd=root, check=True, capture_output=True, text=True, **kwargs).stdout


class Repository(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "repo"
        self.root.mkdir()
        git(self.root, "init", "-q", "-b", "main")
        git(self.root, "config", "user.name", "Fixture")
        git(self.root, "config", "user.email", "fixture@localhost")
        git(self.root, "config", "core.autocrlf", "false")
        for name, text in {".gitignore": "/outputs/\nignored.txt\n", "kept.txt": "kept\n",
                           "edited.txt": "before\n", "deleted.txt": "gone\n"}.items():
            (self.root / name).write_text(text, newline="\n")
        git(self.root, "add", "-A")
        git(self.root, "commit", "-q", "-m", "fixture")
        self.head = git(self.root, "rev-parse", "HEAD").strip()

    def dirty(self):
        (self.root / "edited.txt").write_text("after\n", newline="\n")
        (self.root / "deleted.txt").unlink()
        (self.root / "untracked.txt").write_text("new\n", newline="\n")
        (self.root / "ignored.txt").write_text("secret\n", newline="\n")
        (self.root / "outputs").mkdir()
        (self.root / "outputs/result.json").write_text("{}\n")
        git(self.root, "add", "edited.txt")  # A staged entry that must survive untouched.

    def state(self):
        return {"index": (self.root / ".git/index").read_bytes(), "head": git(self.root, "rev-parse", "HEAD"),
                "status": git(self.root, "status", "--porcelain", "--ignored"),
                "stash": git(self.root, "stash", "list"),
                "files": {p.relative_to(self.root).as_posix(): p.read_bytes()
                          for p in self.root.rglob("*") if p.is_file() and ".git" not in p.parts}}

    def test_freeze_captures_the_working_tree_without_touching_user_state(self):
        self.dirty()
        before = self.state()
        commit, tree = perf_build.freeze(self.root)
        self.assertEqual(self.state(), before)
        files = git(self.root, "ls-tree", "-r", "--name-only", tree).split()
        self.assertEqual(sorted(files), [".gitignore", "edited.txt", "kept.txt", "untracked.txt"])
        self.assertEqual(git(self.root, "show", f"{tree}:edited.txt"), "after\n")
        self.assertEqual(git(self.root, "rev-parse", f"{commit}^{{tree}}").strip(), tree)
        self.assertEqual(git(self.root, "rev-parse", f"{commit}^").strip(), self.head)
        self.assertEqual(perf_build.freeze(self.root), (commit, tree))

    def test_session_worktrees_live_under_work_and_are_removed(self):
        self.dirty()
        work = self.root / "outputs/run/work"
        with perf_build.Session(self.root, work) as session:
            candidate = session.freeze("candidate")
            baseline = session.worktree("baseline", "main")
            self.assertEqual((candidate.checkout, baseline.checkout), (work / "candidate", work / "baseline"))
            self.assertEqual((candidate.checkout / "untracked.txt").read_text(), "new\n")
            self.assertFalse((candidate.checkout / "deleted.txt").exists())
            self.assertFalse((candidate.checkout / "ignored.txt").exists())
            self.assertEqual((baseline.checkout / "edited.txt").read_text(), "before\n")
            self.assertEqual((baseline.commit, baseline.origin), (self.head, "ref main"))
            self.assertEqual(candidate.tree, perf_build.freeze(self.root)[1])
        self.assertFalse(work.exists())
        self.assertNotIn("outputs/run/work", git(self.root, "worktree", "list").replace("\\", "/"))
        with self.assertRaisesRegex(RuntimeError, "rev-parse"):
            with perf_build.Session(self.root, work) as session:
                session.worktree("baseline", "no-such-ref")

    def test_outputs_must_be_outside_the_snapshot(self):
        perf_build.require_outside_snapshot(self.root, self.root / "outputs/perf/new")
        perf_build.require_outside_snapshot(self.root, Path(self.temporary.name) / "elsewhere")
        with self.assertRaisesRegex(ValueError, "not ignored"):
            perf_build.require_outside_snapshot(self.root, self.root / "perf-results")


class Overlay(unittest.TestCase):
    def test_measurement_inputs_are_copied_and_changes_recorded(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, destination = root / "candidate", root / "baseline"
            for checkout in (source, destination):
                for name in perf_build.HARNESS:
                    (checkout / name).parent.mkdir(parents=True, exist_ok=True)
                    (checkout / name).write_text("// same\n")
                (checkout / "tests/fixtures").mkdir(parents=True)
                (checkout / "src").mkdir()
                (checkout / "src/lib.rs").write_text(f"// {checkout.name} runtime\n")
            (source / "examples/perf_baseline.rs").write_text("// candidate harness\n")
            (source / "tests/fixtures/nested").mkdir()
            (source / "tests/fixtures/nested/new.json").write_text("{}\n")
            (destination / "tests/fixtures/base-only.json").write_text("{}\n")
            snapshot = lambda label, path: perf_build.Snapshot(label, path, "c" * 40, "t" * 40, "fixture")
            result = perf_build.overlay(snapshot("candidate", source), snapshot("baseline", destination))
            self.assertEqual(result.suite_changes, ("examples/perf_baseline.rs", "tests/fixtures/nested/new.json"))
            self.assertEqual(set(result.inputs), {*perf_build.HARNESS, "tests/fixtures/nested/new.json"})
            self.assertEqual((destination / "examples/perf_baseline.rs").read_text(), "// candidate harness\n")
            self.assertTrue((destination / "tests/fixtures/base-only.json").exists())
            self.assertEqual((destination / "src/lib.rs").read_text(), "// baseline runtime\n")
            self.assertEqual(result.record()["suite_changes"], list(result.suite_changes))


class Targets(unittest.TestCase):
    def test_local_targets_are_per_label_and_refuse_concurrent_use(self):
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {"CARGO_TARGET_DIR": ""}):
            targets = perf_build.LocalTargets(Path(temporary))
            self.assertEqual(targets.path("baseline"), Path(temporary) / "target/perf-suite/baseline")
            self.assertEqual(targets.environment("candidate")["CARGO_TARGET_DIR"],
                             str(Path(temporary) / "target/perf-suite/candidate"))
            with targets.hold(("baseline", "candidate")):
                lock = targets.base / "candidate.lock"
                self.assertEqual(json.loads(lock.read_text())["pid"], os.getpid())
                with self.assertRaisesRegex(RuntimeError, "candidate.lock exists"):
                    with targets.hold(("candidate",)):
                        pass
                self.assertTrue(lock.exists())
            self.assertEqual(list(targets.base.glob("*.lock")), [])
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": "shared"}):
                self.assertEqual(perf_build.LocalTargets(Path(temporary)).base, Path(temporary) / "shared/perf-suite")

    def test_failed_builds_never_reuse_stale_outputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            targets = perf_build.LocalTargets(root)
            snapshots = []
            for label in ("baseline", "candidate"):
                checkout = root / label
                checkout.mkdir()
                (checkout / "Cargo.lock").write_text("# lock\n")
                snapshots.append(perf_build.Snapshot(label, checkout, label[0] * 40, "t" * 40, "fixture"))
                stale = targets.path(label) / f"release/examples/perf_baseline{perf_build.SUFFIX}"
                stale.parent.mkdir(parents=True)
                stale.write_bytes(b"stale " + label.encode())
            hooks = []

            def command(args, cwd, log, environment=None, timeout=600):
                label = Path(cwd).name
                log.write_text("synthetic build\n")
                self.assertEqual(environment["CARGO_TARGET_DIR"], str(targets.path(label)))
                self.assertFalse((targets.path(label) / f"release/examples/perf_baseline{perf_build.SUFFIX}").exists())
                if label == "candidate":
                    return 101
                (targets.path(label) / f"release/examples/perf_baseline{perf_build.SUFFIX}").write_bytes(b"fresh")
                return 0

            with patch.object(perf_build, "command", side_effect=command), patch.object(
                    perf_build, "output", return_value="synthetic tool"), contextlib.redirect_stdout(io.StringIO()):
                builds, errors = perf_build.build_all(
                    snapshots, targets, ("perf_baseline",), logs=root, bins=root / "out",
                    hook=lambda snapshot, build, environment: hooks.append((snapshot.label, build is not None)))
            self.assertEqual(errors, ["candidate release build exited 101"])
            self.assertEqual(list(builds), ["baseline"])
            self.assertEqual(hooks, [("baseline", True), ("candidate", False)])
            self.assertFalse((root / "out/bin-candidate").exists())
            build = builds["baseline"]
            self.assertEqual(build.binaries["perf_baseline"].read_bytes(), b"fresh")
            record = json.loads((root / "baseline-identity.json").read_text())
            self.assertEqual((record["commit"], record["tree"], record["target_policy"]),
                             ("b" * 40, "t" * 40, targets.policy))
            self.assertEqual(record["binaries"][f"perf_baseline{perf_build.SUFFIX}"]["sha256"],
                             perf_build.digest(build.binaries["perf_baseline"]))
            build.verify()
            build.binaries["perf_baseline"].write_bytes(b"replaced")
            with self.assertRaisesRegex(RuntimeError, "changed after the build record"):
                build.verify()


class Commands(unittest.TestCase):
    GRANDCHILD = ("import os, sys, time\n"
                  "open(sys.argv[2], 'w').write(str(os.getpid()))\n"
                  "while True:\n"
                  "    open(sys.argv[1], 'a').write('.')\n"
                  "    time.sleep(0.02)\n")

    def run_tree(self, after_spawn, timeout, stop=None):
        """Child starts a heartbeating grandchild, waits until it runs, then runs `after_spawn`."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            beat, pid, release = root / "heartbeat", root / "grandchild.pid", root / "release"
            child = ("import os, subprocess, sys, time\n"
                     f"beat, pid, release = {str(beat)!r}, {str(pid)!r}, {str(release)!r}\n"
                     f"subprocess.Popen([sys.executable, '-c', {self.GRANDCHILD!r}, beat, pid])\n"
                     "while not os.path.exists(pid):\n"
                     "    time.sleep(0.01)\n" + after_spawn)
            log = root / "command.log"
            original = perf_build.ProcessTree.stop

            def delayed(tree):
                stop(tree, pid, release)
                original(tree)

            try:
                with patch.object(perf_build.ProcessTree, "stop", delayed) if stop else contextlib.nullcontext():
                    code = perf_build.command([sys.executable, "-c", child], root, log, timeout=timeout)
                if not pid.exists():
                    self.skipTest("grandchild did not start")
                size = beat.stat().st_size
                time.sleep(0.3)
                self.assertEqual(beat.stat().st_size, size, "grandchild outlived command()")
                return code, log.read_text()
            finally:
                if pid.exists():
                    kill(int(pid.read_text()))

    def test_timeout_kills_the_whole_process_tree(self):
        code, log = self.run_tree("time.sleep(60)\n", timeout=1.5)
        self.assertEqual(code, -1)
        self.assertIn("TimeoutExpired", log)

    def test_descendants_die_when_the_child_exits_between_timeout_and_cleanup(self):
        def child_exits_first(tree, pid, release):
            # The timeout has fired with the child alive; let it exit before cleanup runs.
            deadline = time.perf_counter() + 10
            while not pid.exists() and time.perf_counter() < deadline:
                time.sleep(0.01)
            release.write_text("")
            while tree.process.poll() is None and time.perf_counter() < deadline:
                time.sleep(0.01)
            self.assertIsNotNone(tree.process.poll(), "child did not exit before cleanup")

        code, log = self.run_tree("while not os.path.exists(release):\n    time.sleep(0.01)\n",
                                  timeout=0.5, stop=child_exits_first)
        self.assertEqual(code, -1)
        self.assertIn("TimeoutExpired", log)

    def test_descendants_die_when_the_child_exits_normally(self):
        code, _ = self.run_tree("sys.exit(0)\n", timeout=30)
        self.assertEqual(code, 0)

    def test_missing_executable_is_logged(self):
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "command.log"
            self.assertEqual(perf_build.command(["slither-perf-missing-executable"], temporary, log), -1)
            self.assertIn("Error", log.read_text())


def kill(pid):
    if os.name == "nt":
        subprocess.run(["taskkill", "/F", "/PID", str(pid)], capture_output=True, check=False)
    else:
        import signal
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


if __name__ == "__main__":
    unittest.main()
